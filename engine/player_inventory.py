"""Read saved inventory/loadouts. No network access and no save-file writes."""
from dataclasses import dataclass
from pathlib import Path
import hashlib
import os
from collections import Counter
from save_reader import parse_savedata
from save_reader import crc32
import struct

PAINTS = {0: '', 1: 'Crimson', 2: 'Lime', 3: 'Black', 4: 'Sky Blue', 5: 'Cobalt',
          6: 'Burnt Sienna', 7: 'Forest Green', 8: 'Purple', 9: 'Pink', 10: 'Orange',
          11: 'Grey', 12: 'Titanium White', 13: 'Saffron', 14: 'Gold'}
SLOT_INDEX = {0: 'Body', 1: 'Decal', 2: 'Wheels', 3: 'Rocket Boost', 4: 'Topper',
              5: 'Antenna', 13: 'Engine Audio', 14: 'Trail', 15: 'Goal Explosion'}


def save_folders():
    documents = Path.home() / 'Documents'
    if os.name == 'nt':
        import ctypes
        buffer = ctypes.create_unicode_buffer(32768)
        if ctypes.windll.shell32.SHGetFolderPathW(None, 5, None, 0, buffer) == 0:
            documents = Path(buffer.value)
    return [documents / 'My Games/Rocket League/TAGame/SaveDataEpic',
            documents / 'My Games/Rocket League/TAGame/SaveData']


def save_candidates():
    files = [file for folder in save_folders() if folder.is_dir() for file in folder.rglob('*.save')]
    return sorted(files, key=lambda file: file.stat().st_mtime, reverse=True)


def instance_key(value):
    if not isinstance(value, dict):
        return (0, 0)
    return (int(value.get('UpperBits', 0)), int(value.get('LowerBits', 0)))


@dataclass
class Snapshot:
    inventory: list
    presets: list
    active_index: int
    account_equipped: list
    source: Path
    modified_ns: int
    crc_match: bool
    unknown_products: int

    @property
    def active(self):
        return self.presets[self.active_index] if 0 <= self.active_index < len(self.presets) else None


def extract_snapshot(parsed, database, source, modified_ns):
    if not parsed.get('file_info', {}).get('crc_match'):
        raise ValueError('El guardado no supera la comprobación CRC; se conserva la última lectura válida.')
    objects = {entry['object_index']: obj for entry, obj in zip(parsed['object_types'], parsed['objects'])}
    products = {int(row['ID']): row for row in database}
    inventory, by_instance = [], {}
    unknown = 0

    def describe(product_id, instance=None, attribute_refs=(), source_kind='online'):
        nonlocal unknown
        row = products.get(product_id, {})
        if product_id and not row:
            unknown += 1
        attrs = [objects.get(ref, {}) for ref in attribute_refs if isinstance(ref, int)]
        paint = next((a.get('PaintID', 0) for a in attrs if a.get('__type') == 'TAGame.ProductAttribute_Painted_TA'), 0)
        certified = next((a.get('StatId') for a in attrs if a.get('__type') == 'TAGame.ProductAttribute_Certified_TA'), None)
        blueprint = any(a.get('__type') == 'TAGame.ProductAttribute_Blueprint_TA' for a in attrs)
        row = dict(row)
        row.update({'ID': product_id, 'Product': row.get('Product', f'Producto #{product_id}'),
                    'Slot': row.get('Slot', 'Sin identificar'), 'paint': PAINTS.get(paint, f'Paint #{paint}'),
                    'paint_id': paint, 'certified': certified, 'blueprint': blueprint,
                    'instance': instance_key(instance), 'source_kind': source_kind})
        title_id=next((a.get('TitleId') for a in attrs if a.get('__type')=='TAGame.ProductAttribute_TitleID_TA'),None)
        if title_id:row['title_id']=title_id
        return row

    for ref in parsed.get('properties', {}).get('OnlineProducts', []):
        obj = objects.get(ref)
        if not obj or obj.get('__type') != 'TAGame.OnlineProduct_TA' or '__parse_error' in obj:
            raise ValueError('No se pudo leer íntegramente el inventario del guardado.')
        item = describe(int(obj.get('ProductID', 0)), obj.get('InstanceID'), obj.get('Attributes', []))
        inventory.append(item)
        by_instance[item['instance']] = item
    profile = next((o for o in objects.values() if o.get('__type') == 'TAGame.ProfileLoadoutSave_TA'), None)
    if not profile or '__parse_error' in profile:
        raise ValueError('No se ha podido identificar el perfil de equipamiento.')
    presets = []
    active_index = -1
    for ref in profile.get('LoadoutSets', []):
        preset = objects.get(ref, {})
        if preset.get('__type') != 'TAGame.LoadoutSet_TA' or '__parse_error' in preset:
            raise ValueError('Hay un preset ilegible en el guardado.')
        teams = []
        for loadout_ref in preset.get('Loadouts', []):
            loadout = objects.get(loadout_ref, {})
            if loadout.get('__type') != 'TAGame.Loadout_TA' or '__parse_error' in loadout:
                raise ValueError('No se ha podido leer el equipamiento de un equipo.')
            equipped = []
            instances = loadout.get('OnlineProducts128', [])
            for slot_index, product_id in enumerate(loadout.get('Products', [])):
                if not isinstance(product_id, int) or product_id <= 0:
                    continue
                instance = instances[slot_index] if slot_index < len(instances) else {}
                item = by_instance.get(instance_key(instance))
                if item is None or item['ID'] != product_id:
                    item = describe(product_id, instance, source_kind='equipped-local')
                if item['Slot'] == 'Sin identificar' and slot_index in SLOT_INDEX:
                    item['Slot'] = SLOT_INDEX[slot_index]
                equipped.append({**item, 'slot_index': slot_index})
            teams.append({'items': equipped, 'paint': loadout.get('TeamPaint', {})})
        if ref == profile.get('EquippedLoadoutSet'):
            active_index = len(presets)
        presets.append({'name': preset.get('LoadoutSetName', 'Preset'), 'teams': teams})
    extras = []
    for key, slot in [('PlayerTitle', 'Player Title'), ('MusicStingerInstanceID', 'Player Anthem')]:
        if key == 'PlayerTitle' and profile.get(key):
            extras.append({'ID': 0, 'Product': profile[key], 'Slot': slot, 'paint': '', 'instance': (0, 0), 'blueprint': False})
        elif key == 'MusicStingerInstanceID':
            item = by_instance.get(instance_key(profile.get(key)))
            if item:
                extras.append(item)
    for typ in ('TAGame.PlayerBannerSave_TA', 'TAGame.PlayerAvatarBorderSave_TA'):
        obj = next((obj for obj in objects.values() if obj.get('__type') == typ), {})
        if obj.get('ProductID'):
            item = by_instance.get(instance_key(obj.get('InstanceID')))
            extras.append(item or describe(obj['ProductID'], obj.get('InstanceID'), source_kind='equipped-local'))
    # Only report entries present in the save; do not invent a full default inventory.
    known = {item['ID'] for item in inventory if not item['blueprint']}
    for preset in presets:
        for team in preset['teams']:
            for item in team['items']:
                if item['ID'] not in known:
                    inventory.append(item)
                    known.add(item['ID'])
    unknown = sum(item['ID'] not in products for item in inventory)
    return Snapshot(inventory, presets, active_index, extras, source, modified_ns, True, unknown)


def load_snapshot(database, file=None):
    candidates = [Path(file)] if file else save_candidates()
    if not candidates:
        raise FileNotFoundError('No se han encontrado guardados locales de Rocket League.')
    # Never silently switch to an older account or backup if the latest save is invalid.
    source = candidates[0]
    before = source.stat()
    if before.st_size > 64 * 1024 * 1024:
        raise ValueError('El guardado supera el tamaño admitido por el lector.')
    raw = source.read_bytes()
    if len(raw) < 8:
        raise ValueError('Guardado incompleto.')
    size, expected_crc = struct.unpack_from('<II', raw)
    if size < 16 or size % 16 or size > len(raw) - 8 or crc32(raw[8:8 + size]) != expected_crc:
        raise ValueError('Guardado incompleto o CRC incorrecto; espera al próximo guardado.')
    fingerprint = hashlib.sha256(raw).digest()
    parsed = parse_savedata(str(source))
    after = source.stat()
    if (before.st_mtime_ns, before.st_size) != (after.st_mtime_ns, after.st_size) or fingerprint != hashlib.sha256(source.read_bytes()).digest():
        raise ValueError('Rocket League está guardando. Se reintentará en la siguiente actualización.')
    return extract_snapshot(parsed, database, source, after.st_mtime_ns)
