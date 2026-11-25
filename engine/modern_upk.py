"""Local UPK codec and same-depth ProductAsset rewire.

Format/rewiring research: c1yq/RL-Swapper (MIT), src/upk_engine.rs.
Does not alter serialized donor object payloads, gameplay or process memory.
"""
from pathlib import Path
import base64
import hashlib
import io
import json
import struct
import sys
import zlib
from cryptography.hazmat.primitives.ciphers import Cipher, algorithms, modes


def i32(data, offset):
    return struct.unpack_from('<i', data, offset)[0]


def i64(data, offset):
    return struct.unpack_from('<q', data, offset)[0]


def put32(data, offset, value):
    struct.pack_into('<i', data, offset, value)


def put64(data, offset, value):
    struct.pack_into('<q', data, offset, value)


def crypt(key, data, nonce=None, decrypt=False):
    mode = modes.CTR(nonce + bytes(4)) if nonce is not None else modes.ECB()
    operation = Cipher(algorithms.AES(key), mode).decryptor() if decrypt else Cipher(algorithms.AES(key), mode).encryptor()
    return operation.update(data) + operation.finalize()


class Package:
    def __init__(self, file, engine, raw=None):
        self.file, self.engine = Path(file), Path(engine)
        sys.path.insert(0, str(engine))
        import rl_upk_editor as upk
        self.upk = upk
        self.raw = self.file.read_bytes() if raw is None else raw
        stream = io.BytesIO(self.raw)
        self.summary = summary = upk.parse_file_summary(stream)
        self.meta_offset = stream.tell()
        self.meta = upk.parse_file_compression_metadata(stream)
        self.nonce = self.raw[self.meta_offset + 12:self.meta_offset + 24] if summary.package_flags & 0x800 else None
        self.stride = 36 if summary.licensee_version >= 33 or self.nonce is not None else 24
        self.enc_size = summary.total_header_size - self.meta.garbage_size - summary.name_offset
        self.enc_aligned = (self.enc_size + 15) & ~15
        if not 0 < self.enc_aligned <= len(self.raw) - summary.name_offset:
            raise ValueError('Cabecera cifrada fuera de rango.')
        encrypted = self.raw[summary.name_offset:summary.name_offset + self.enc_aligned]
        keys = [upk.DEFAULT_KEY]
        keys.extend(base64.b64decode(line.strip()) for line in (self.engine/'keys.txt').read_text().splitlines()
                    if line.strip() and not line.startswith('#'))
        mapped = json.loads((self.engine/'keys_map.json').read_text()) if (self.engine/'keys_map.json').exists() else {}
        stem = self.file.stem.casefold()
        preferred = mapped.get(stem, mapped.get(stem.removesuffix('_sf')))
        if isinstance(preferred, int) and 0 <= preferred + 1 < len(keys):
            keys.insert(0, keys[preferred + 1])
        elif isinstance(preferred, str):
            keys.insert(0, base64.b64decode(preferred))
        self.key = None
        for key in dict.fromkeys(keys):
            if len(key) != 32:
                continue
            plain = crypt(key, encrypted, self.nonce, decrypt=True)
            offset = self.meta.compressed_chunks_offset
            if 0 <= offset <= len(plain) - 12 and 1 <= i32(plain, offset) <= 512 and i64(plain, offset + 4) == summary.depends_offset:
                self.key, self.header = key, plain[:self.enc_size]
                break
        if self.key is None:
            raise ValueError('No se reconoce el cifrado de ' + self.file.name)
        self.chunks = []
        offset = self.meta.compressed_chunks_offset
        for index in range(i32(self.header, offset)):
            pos = offset + 4 + index * self.stride
            uoff, usize, coff, csize = struct.unpack_from('<qiqi', self.header, pos)
            nonce = self.header[pos + 24:pos + 36] if self.stride == 36 else None
            if min(uoff, usize, coff, csize) < 0 or coff + csize > len(self.raw) or uoff + usize > 512 * 1024 * 1024:
                raise ValueError('Chunk fuera de rango.')
            self.chunks.append((uoff, usize, coff, csize, nonce))
        self.names, self.name_flags, self.names_end = [], [], 0
        cursor = 0
        reader = upk.BinaryReader(io.BytesIO(self.header))
        for _ in range(summary.name_count):
            self.names.append(reader.read_fstring())
            self.name_flags.append(reader.read_u64())
        self.names_end = reader.tell()
        self.imports = [self.header[summary.import_offset - summary.name_offset + index*28:summary.import_offset - summary.name_offset + (index+1)*28]
                        for index in range(summary.import_count)]
        self.exports = []
        cursor = summary.export_offset - summary.name_offset
        for index in range(summary.export_count):
            net_count = i32(self.header, cursor + 48)
            length = 72 + net_count * 4
            if net_count < 0 or cursor + length > summary.depends_offset - summary.name_offset:
                raise ValueError('Tabla de exports no válida.')
            self.exports.append(bytearray(self.header[cursor:cursor+length]))
            cursor += length
        self.exports_end = cursor

    def fname(self, index, instance=0):
        name = self.names[index]
        return name + ('_' + str(instance-1) if instance > 0 else '')

    def export_name(self, export):
        return self.fname(i32(export, 12), i32(export, 16))

    def class_name(self,export):
        index=i32(export,0)
        if index<0:
            imported=self.imports[-index-1]
            return self.fname(i32(imported,20),i32(imported,24))
        return self.export_name(self.exports[index-1]) if index>0 else 'Class'

    def path(self, position):
        parts, seen = [], set()
        while position >= 0:
            if position in seen or position >= len(self.exports):
                raise ValueError('Cadena de exports cíclica o fuera de rango.')
            seen.add(position)
            export = self.exports[position]
            parts.append(self.export_name(export))
            outer = i32(export, 8)
            if outer <= 0:
                break
            position = outer - 1
        return '.'.join(reversed(parts))

    def find_asset(self, asset_path):
        matches = [index for index in range(len(self.exports)) if self.path(index).casefold() == asset_path.casefold()]
        if len(matches) != 1:
            raise ValueError('No se puede identificar de forma unívoca el ProductAsset: ' + asset_path)
        return matches[0]

    def flat(self):
        if hasattr(self,'_flat_cache'):return self._flat_cache
        summary = self.summary
        size = max(uoff + usize for uoff, usize, _, _, _ in self.chunks)
        flat = bytearray(size)
        flat[:summary.name_offset] = self.raw[:summary.name_offset]
        flat[summary.name_offset:summary.depends_offset] = self.header[:summary.depends_offset-summary.name_offset]
        for uoff, usize, coff, csize, nonce in self.chunks:
            payload = self.raw[coff:coff+csize]
            if self.nonce is not None:
                payload = crypt(self.key, payload, nonce, decrypt=True)
            if len(payload) < 16 or struct.unpack_from('<I', payload)[0] != 0x9E2A83C1:
                raise ValueError('No se reconoce la cabecera de compresión.')
            total = i32(payload, 12)
            cursor, amount, blocks = 16, 0, []
            while amount < total:
                comp, uncomp = struct.unpack_from('<ii', payload, cursor)
                if comp <= 0 or uncomp <= 0:
                    raise ValueError('Tamaño de bloque no válido.')
                blocks.append((comp, uncomp))
                amount += uncomp
                cursor += 8
            data = bytearray()
            for comp, uncomp in blocks:
                block = zlib.decompress(payload[cursor:cursor+comp])
                if len(block) != uncomp:
                    raise ValueError('El bloque descomprimido no coincide con su tamaño.')
                data.extend(block)
                cursor += comp
            if len(data) != usize:
                raise ValueError('El chunk descomprimido no coincide con su tamaño.')
            flat[uoff:uoff+usize] = data
        put32(flat, summary.compression_flags_offset, 0)
        struct.pack_into('<I', flat, summary.package_flags_flags_offset, summary.package_flags & ~8)
        self._flat_cache=bytes(flat)
        return self._flat_cache


def rewire(donor, target, donor_item, target_item):
    """Same-depth outer-chain rewire only, fail closed for complex packages."""
    donor_position = donor.find_asset(donor_item['AssetPath'])
    target_position = target.find_asset(target_item['AssetPath'])
    donor_class=donor.class_name(donor.exports[donor_position])
    target_class=target.class_name(target.exports[target_position])
    if donor_class!=target_class or not donor_class.startswith('ProductAsset_'):
        raise ValueError('Las clases del objeto cosmético no son compatibles.')
    changes = []
    dindex, tindex = donor_position, target_position
    while True:
        dexp, texp = donor.exports[dindex], target.exports[tindex]
        changes.append((dindex, target.names[i32(texp,12)], i32(texp,16)))
        dout, tout = i32(dexp,8), i32(texp,8)
        if dout <= 0 or tout <= 0:
            if dout != tout:
                raise ValueError('Profundidad de paquetes distinta; este par necesita edición adicional.')
            break
        dindex, tindex = dout-1, tout-1
    for index, export in enumerate(donor.exports):
        if i32(export,8) == 0 and donor.export_name(export).casefold() == donor.file.stem.casefold():
            target_roots = [exp for exp in target.exports if i32(exp,8) == 0 and target.export_name(exp).casefold() == target.file.stem.casefold()]
            if len(target_roots) == 1:
                changes.append((index, target.names[i32(target_roots[0],12)], i32(target_roots[0],16)))
    names, additions = list(donor.names), bytearray()
    exports = [bytearray(export) for export in donor.exports]
    flags = donor.name_flags[i32(donor.exports[donor_position],12)]
    for position, name, instance in changes:
        matches = [index for index, existing in enumerate(names) if existing.casefold() == name.casefold()]
        if matches:
            name_index = matches[0]
        else:
            encoded = name.encode('windows-1252') + b'\0'
            additions.extend(struct.pack('<i',len(encoded)) + encoded + struct.pack('<Q',flags))
            name_index = len(names)
            names.append(name)
        put32(exports[position],12,name_index)
        put32(exports[position],16,instance)
    delta = len(additions)
    summary = donor.summary
    nameoff = summary.name_offset
    importrel, exportrel, dependsrel = summary.import_offset-nameoff, summary.export_offset-nameoff, summary.depends_offset-nameoff
    for export in exports:
        if i64(export,36) > 0:
            put64(export,36,i64(export,36)+delta)
    header = bytearray(donor.header[:donor.names_end] + additions + donor.header[donor.names_end:exportrel] +
                       b''.join(exports) + donor.header[donor.exports_end:])
    chunkoffset = donor.meta.compressed_chunks_offset + delta
    for index in range(len(donor.chunks)):
        pos = chunkoffset + 4 + index * donor.stride
        put64(header,pos,i64(header,pos)+delta)
    newaligned = (len(header)+15)&~15
    growth = newaligned - donor.enc_aligned
    gap = summary.total_header_size - nameoff - donor.enc_aligned
    extra = (max(growth-gap, delta-donor.meta.garbage_size, 0)+255)&~15 if growth>gap or delta>donor.meta.garbage_size else 0
    for index in range(len(donor.chunks)):
        pos = chunkoffset + 4 + index * donor.stride
        put64(header,pos+12,i64(header,pos+12)+extra)
    header.extend(bytes(newaligned-len(header)))
    prefix = bytearray(donor.raw[:nameoff])
    offsets = donor.upk._find_summary_offsets(donor.raw)
    for key, value in [('total_header_size_offset',summary.total_header_size+extra), ('name_count_offset',len(names)),
                       ('import_offset_offset',summary.import_offset+delta), ('export_offset_offset',summary.export_offset+delta),
                       ('depends_offset_offset',summary.depends_offset+delta)]:
        put32(prefix,offsets[key],value)
    for key in ('import_export_guids_offset_offset','thumbnail_table_offset_offset'):
        if key in offsets and i32(prefix,offsets[key])>0:
            put32(prefix,offsets[key],i32(prefix,offsets[key])+delta)
    donor.upk._patch_generation_counts(prefix,offsets,summary.export_count,len(names))
    put32(prefix,donor.meta_offset,donor.meta.garbage_size+extra-delta)
    put32(prefix,donor.meta_offset+4,chunkoffset)
    guid_pos=offsets['import_export_guids_offset_offset']+16
    target_offsets=target.upk._find_summary_offsets(target.raw)
    target_guid_pos=target_offsets['import_export_guids_offset_offset']+16
    prefix[guid_pos:guid_pos+16]=target.raw[target_guid_pos:target_guid_pos+16]
    existing_gap=donor.raw[nameoff+donor.enc_aligned:summary.total_header_size]
    newgap=gap+extra-growth
    output=bytearray(prefix + crypt(target.key,bytes(header),donor.nonce) + existing_gap[:newgap] + bytes(max(0,newgap-len(existing_gap))) + donor.raw[summary.total_header_size:])
    if donor.nonce is not None and donor.key != target.key:
        for index in range(len(donor.chunks)):
            pos=chunkoffset+4+index*donor.stride
            coff,csize=i64(header,pos+12),i32(header,pos+20)
            nonce=header[pos+24:pos+36]
            output[coff:coff+csize]=crypt(target.key,crypt(donor.key,bytes(output[coff:coff+csize]),nonce,decrypt=True),nonce)
    checked=Package(target.file,donor.engine,raw=bytes(output))
    checked.find_asset(target_item['AssetPath'])
    before,after=donor.flat(),checked.flat()
    for old,new in zip(donor.exports,checked.exports):
        size=i32(old,32)
        if size!=i32(new,32) or before[i64(old,36):i64(old,36)+size]!=after[i64(new,36):i64(new,36)+size]:
            raise ValueError('Ha cambiado un contenido serializado del donante; operación rechazada.')
    return bytes(output), {'method':'productasset-export-rewire','exports':len(exports),
        'serialized_payloads_unchanged':True,'target_asset_path':target_item['AssetPath'],
        'donor_sha256':hashlib.sha256(donor.raw).hexdigest(),'generated_sha256':hashlib.sha256(output).hexdigest()}
