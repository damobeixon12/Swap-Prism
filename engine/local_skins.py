"""Swap Prism (antes LocalSkins): API común del motor.

Rutas de la app, catálogo de objetos, detección del juego y ejecución de workers.
La interfaz es la app Rust (app/); este módulo ya no abre ninguna ventana.
El códec de paquetes (motor/rl_upk_editor.py) procede de RLUPKTools.
"""
import argparse
import hashlib
import json
import os
from pathlib import Path
import sys

FROZEN = getattr(sys, 'frozen', False)
SOURCE_DIR = Path(__file__).resolve().parent
# Carpeta de la app (contiene data/). La app Rust la pasa en SWAPPRISM_ROOT.
# Desde el código fuente se usa la copia de desarrollo outputs/LocalSkins del proyecto.
ROOT = Path(sys.executable).resolve().parent if FROZEN else SOURCE_DIR.parent / 'outputs' / 'LocalSkins'
ROOT = Path(os.environ.get('SWAPPRISM_ROOT', str(ROOT))).resolve()
RESOURCES = Path(getattr(sys, '_MEIPASS', ROOT))
DEFAULT_ENGINE = RESOURCES / 'motor' if FROZEN else SOURCE_DIR / 'motor'
SLOTS = ('Body', 'Decal', 'Wheels', 'Rocket Boost', 'Trail', 'Topper', 'Antenna', 'Goal Explosion', 'Player Banner', 'Avatar Border')
ENABLED_SLOTS = set(SLOTS)
STANDARD_CARS = {'Octane', 'Fennec', 'Dominus', 'Breakout', 'Merc', 'Backfire',
                 'Gizmo', 'Hotshot', 'Paladin', 'Road Hog', 'Venom', 'X-Devil',
                 'Takumi', 'Scarab', 'Zippy', 'Ripper', 'Grog'}


def read_database(engine):
    rows=json.loads((Path(engine) / 'items.json').read_text(encoding='utf-8-sig'))['Items']
    extra=Path(engine)/'catalog_enrichment.json'
    if extra.exists():
        data=json.loads(extra.read_text(encoding='utf-8'))
        for row in rows:
            row.update(data['metadata'].get(str(row['ID']),{}))
            row['Source']='https://github.com/c1yq/RL-Swapper/blob/main/items.json'
            if row['ID']==12968 and row.get('SecondaryName')!=row['Product']:
                row['NameConflict']=True
                row['Product']='Nombre por verificar'
        known={r['ID'] for r in rows}
        rows.extend(r for r in data['additional'] if r['ID'] not in known)
    return rows


def compatibility(item, car):
    """Conservative filters, not a claim of in-game or online compatibility."""
    if item.get('Slot') == 'Body':
        return bool(item.get('AssetPath') and item.get('AssetPackage')), 'Solo puente visual sobre los datos físicos originales; requiere validación de modelo.'
    if item.get('Slot') not in ENABLED_SLOTS:
        return False, 'Esta categoría solo está disponible para consulta.'
    if car not in STANDARD_CARS:
        return False, 'Este coche necesita reglas de compatibilidad verificadas.'
    attributes = {attribute.get('Key') for attribute in item.get('Attributes', [])}
    if attributes & {'IsLicensed', 'PreviewOnly', 'InclusivePlatforms'}:
        return False, 'Objeto con restricciones de licencia, plataforma o vista previa.'
    if item.get('Slot') == 'Decal' and ':' in item.get('Product', ''):
        required = item['Product'].split(':', 1)[0].strip()
        if required.casefold() != car.casefold():
            return False, 'Decal específico de ' + required + '.'
        return True, 'Decal identificado para ' + car + '; confirma que lo permite tu garaje.'
    return True, 'Confirma en el garaje que este objeto puede equiparse en ' + car + '.'


def digest(path):
    h = hashlib.sha256()
    with Path(path).open('rb') as stream:
        for chunk in iter(lambda: stream.read(1024 * 1024), b''):
            h.update(chunk)
    return h.hexdigest()


def detect_epic(manifests=None):
    folder = Path(manifests or Path(os.environ.get('PROGRAMDATA', 'C:/ProgramData')) / 'Epic/EpicGamesLauncher/Data/Manifests')
    for manifest in sorted(folder.glob('*.item')):
        try:
            data = json.loads(manifest.read_text(encoding='utf-8-sig'))
            if 'rocket league' in data.get('DisplayName', '').lower():
                install = Path(data['InstallLocation'])
                cooked = install / 'TAGame' / 'CookedPCConsole'
                if cooked.is_dir():
                    return install, data.get('AppVersionString', '')
        except (ValueError, OSError, KeyError):
            continue
    raise ValueError('No se ha encontrado Rocket League en los manifiestos de Epic.')


def safe_package(name):
    if not isinstance(name, str) or '/' in name or '\\' in name or ':' in name or Path(name).suffix.lower() != '.upk' or name in ('.', '..'):
        raise ValueError('Nombre de paquete no válido.')
    return name


def catalog(engine, game):
    rows = read_database(engine)
    result = []
    for row in rows:
        if row.get('Slot') not in SLOTS or not row.get('AssetPath'):
            continue
        name = safe_package(row.get('AssetPackage'))
        if (Path(game) / 'TAGame/CookedPCConsole' / name).is_file():
            result.append(row)
    return sorted(result, key=lambda item: (item['Slot'], item['Product'], item['ID']))


def separate_output(output, game, engine):
    output = Path(output).resolve()
    for protected in (Path(game).resolve(), Path(engine).resolve()):
        if output == protected or protected in output.parents or output in protected.parents:
            raise ValueError('La salida debe estar separada del juego y del motor.')
    return output


def generate(engine, game, output, target_id, donor_id, version='', log=print,
             car='', compatibility_confirmed=False, color='Original'):
    from cosmetic_backend import generate as generate_cosmetic
    return generate_cosmetic(sys.modules[__name__], engine, game, output, target_id, donor_id,
                             version, log, car, compatibility_confirmed, color)


if __name__ == '__main__':
    if len(sys.argv) >= 4 and sys.argv[1] == '--run-worker':
        import runpy
        script, log_path, params = sys.argv[2], sys.argv[3], sys.argv[4:]
        with open(log_path, 'w', encoding='utf-8') as worker_log:
            sys.stdout = sys.stderr = worker_log
            sys.path.insert(0, str(Path(script).resolve().parent))
            sys.argv = [script, *params]
            runpy.run_path(script, run_name='__main__')
        sys.exit(0)
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--engine', type=Path, default=DEFAULT_ENGINE)
    parser.add_argument('--output', type=Path, default=ROOT / 'paquetes')
    parser.add_argument('--target')
    parser.add_argument('--donor')
    parser.add_argument('--list', action='store_true')
    parser.add_argument('--car', default='')
    parser.add_argument('--confirm-garage', action='store_true')
    parser.add_argument('--color', default='Original')
    args = parser.parse_args()
    if args.list or args.target or args.donor:
        game, version = detect_epic()
        if args.list:
            print(json.dumps(catalog(args.engine, game), ensure_ascii=False, indent=2))
        elif args.target and args.donor:
            print(generate(args.engine, game, args.output, args.target, args.donor, version,
                           car=args.car, compatibility_confirmed=args.confirm_garage,color=args.color))
        else:
            parser.error('Se necesitan --target y --donor juntos.')
    else:
        parser.print_help()
