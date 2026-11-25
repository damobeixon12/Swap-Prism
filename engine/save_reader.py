# Read-only adaptation of nixvio64/RLSaveViewer, commit 85fb1988499f8a58357a4e47e42c6370a3485636.
# Format research: Drogebot/RocketRP. Encryption and save-writing functions removed.
# decrypt/recrypt RL save files. needs pycryptodome
import argparse
import json
import struct
import sys
from pathlib import Path

from cryptography.hazmat.primitives.ciphers import Cipher, algorithms, modes

# ripped from RocketRP AES.cs
AES_KEY = bytes([
    0xD7, 0x8C, 0x32, 0x4A, 0x94, 0x42, 0x94, 0x3C, 0x6D, 0x65, 0xCE, 0x98,
    0x81, 0x85, 0x4C, 0x41, 0x68, 0x99, 0x22, 0x0C, 0xC7, 0xA1, 0x46, 0x40,
    0x93, 0x9B, 0x96, 0x3C, 0x93, 0x2A, 0x6F, 0xAF,
])
CRC_SEED = 0xEFCBF201
OBJHEADER = 0xFFFFFFFF
TYPE_TAGS = {
    "BoolProperty", "IntProperty", "QWordProperty", "FloatProperty",
    "StrProperty", "NameProperty", "ByteProperty",
    "ObjectProperty", "StructProperty", "ArrayProperty",
}
SPECIAL_STRUCTS = {"Vector", "Rotator", "Guid"}

# crc32 matching C# Crc32.CalculateCRC
def _make_crc_table():
    t = []
    for i in range(256):
        c = i << 24
        for _ in range(8):
            c = ((c << 1) ^ 0x04C11DB7) if c & 0x80000000 else (c << 1)
            c &= 0xFFFFFFFF
        t.append(c)
    return t
_CRC_TABLE = _make_crc_table()

def crc32(data: bytes, seed: int = CRC_SEED) -> int:
    crc = ~seed & 0xFFFFFFFF
    for b in data:
        crc = ((crc << 8) ^ _CRC_TABLE[(crc >> 24) ^ b]) & 0xFFFFFFFF
    return ~crc & 0xFFFFFFFF

# aes wrappers
def aes_decrypt(data: bytes) -> bytes:
    decryptor = Cipher(algorithms.AES(AES_KEY), modes.ECB()).decryptor()
    return decryptor.update(data) + decryptor.finalize()

# ue3 string io (int32 len + chars + null).
def read_ue3(data: bytes, offset: int):
    length = struct.unpack_from('<i', data, offset)[0]
    offset += 4
    if length == 0:
        return "", offset
    if length < 0:
        byte_count = (-length) * 2
        raw = data[offset:offset + byte_count]
        if len(raw) >= 2 and raw[-2:] == b'\x00\x00':
            raw = raw[:-2]
        return raw.decode('utf-16-le'), offset + byte_count
    if length > 0:
        s = data[offset:offset + length - 1].decode('utf-8')
        return s, offset + length

def write_ue3(s: str) -> bytes:
    encoded = s.encode('utf-8')
    return struct.pack('<i', len(encoded) + 1) + encoded + b'\x00'

# property stream parser
def parse_property_stream(data: bytes, offset: int):
    # reads tagged props until None sentinel, handles fixed-size arrays via vidx
    props = {}
    fixed = {}

    while True:
        name, offset = read_ue3(data, offset)
        if name == "None":
            break
        tag_offset = offset
        tag, offset = read_ue3(data, offset)
        if tag not in TYPE_TAGS:
            offset = tag_offset
            inner, offset = parse_property_stream(data, offset)
            inner["__type"] = name
            val = inner
            vidx = 0
        else:
            vlen = struct.unpack_from('<i', data, offset)[0]; offset += 4
            vidx = struct.unpack_from('<i', data, offset)[0]; offset += 4
            val, offset = _parse_value(data, offset, tag, vlen)

        if vidx != 0:
            fixed.setdefault(name, {})[vidx] = val
        elif name in props or name in fixed:
            fixed.setdefault(name, {})[vidx] = val
        else:
            props[name] = val

    for name, idxmap in fixed.items():
        if name in props:
            idxmap[0] = props.pop(name)
        props[name] = [idxmap[i] for i in sorted(idxmap)]
    return props, offset


def _parse_value(data: bytes, offset: int, tag: str, vlen: int = 0):
    # dispatch on type tag from the binary
    if tag == "BoolProperty":
        return bool(data[offset]), offset + 1
    elif tag == "IntProperty":
        return struct.unpack_from('<i', data, offset)[0], offset + 4
    elif tag == "QWordProperty":
        return struct.unpack_from('<Q', data, offset)[0], offset + 8
    elif tag == "FloatProperty":
        return round(struct.unpack_from('<f', data, offset)[0], 6), offset + 4
    elif tag in ("StrProperty", "NameProperty"):
        return read_ue3(data, offset)
    elif tag == "ByteProperty":
        tn, offset = read_ue3(data, offset)
        if tn == "None":
            return data[offset], offset + 1
        val, offset = read_ue3(data, offset)
        return val, offset
    elif tag == "ObjectProperty":
        return struct.unpack_from('<i', data, offset)[0], offset + 4
    elif tag == "StructProperty":
        return _parse_struct(data, offset)
    elif tag == "ArrayProperty":
        return _parse_array(data, offset, vlen)
    raise ValueError(f"Unknown tag {tag!r} at offset {offset}")


def _parse_struct(data: bytes, offset: int):
    tn, offset = read_ue3(data, offset)
    # ISpecialSerialized types have fixed binary layout, not a property stream
    if tn == "Vector":
        x, y, z = struct.unpack_from('<fff', data, offset)
        return {"x": round(x, 6), "y": round(y, 6), "z": round(z, 6)}, offset + 12
    if tn == "Rotator":
        p, y, r = struct.unpack_from('<fff', data, offset)
        return {"pitch": round(p, 6), "yaw": round(y, 6), "roll": round(r, 6)}, offset + 12
    if tn == "Guid":
        a, b, c, d = struct.unpack_from('<IIII', data, offset)
        return f"{a:08X}-{b:08X}-{c:08X}-{d:08X}", offset + 16

    # classes in arrays have 0xFFFFFFFF marker between type name and props
    marker = struct.unpack_from('<I', data, offset)[0]
    if marker == OBJHEADER:
        props, offset = parse_property_stream(data, offset + 4)
        props["__type"] = tn
        return props, offset
    # value type struct, prop stream follows type name directly
    props, offset = parse_property_stream(data, offset)
    props["__type"] = tn
    return props, offset


def _parse_array(data: bytes, offset: int, vlen: int):
    count = struct.unpack_from('<i', data, offset)[0]
    offset += 4
    if count <= 0:
        return [], offset

    # use vlen to figure out per-element size for uniform arrays (ints, bools, qwords)
    payload = vlen - 4
    elem_hint = payload // count if payload > 0 else 0
    elems = []

    if elem_hint == 4:
        for _ in range(count):
            elems.append(struct.unpack_from('<i', data, offset)[0])
            offset += 4
        return elems, offset
    if elem_hint == 1:
        for _ in range(count):
            elems.append(data[offset]); offset += 1
        return elems, offset
    if elem_hint == 8:
        for _ in range(count):
            elems.append(struct.unpack_from('<Q', data, offset)[0])
            offset += 8
        return elems, offset

    for _ in range(count):
        elem, offset = _parse_array_elem(data, offset)
        elems.append(elem)
    return elems, offset


def _parse_array_elem(data: bytes, offset: int):
    # array elements have no type tags, gotta sniff the format
    try:
        s, after1 = read_ue3(data, offset)
    except (UnicodeDecodeError, struct.error):
        return struct.unpack_from('<i', data, offset)[0], offset + 4

    if s == "None":
        try:
            read_ue3(data, after1)
            return {}, after1  # empty struct
        except Exception:
            return data[after1], after1 + 1  # byte val (ByteProperty in array)

    if not s:
        return s, after1

    # class elem in array: TypeName + 0xFFFFFFFF + propstream
    if after1 + 4 <= len(data) and struct.unpack_from('<I', data, after1)[0] == OBJHEADER:
        props, off = parse_property_stream(data, after1 + 4)
        props["__type"] = s
        return props, off

    # sniff 2 strings ahead to tell apart struct-with-typename from plain-string.
    # Important: check maybe_prop before reading maybe_tag when maybe_prop == "StructProperty" the bytes after it are binary, which will make read_ue3 throw and abort the whole sniff.
    try:
        maybe_prop, after2 = read_ue3(data, after1)

        if maybe_prop == "StructProperty":
            content = after2 + 8                     
            if content + 4 <= len(data) and \
               struct.unpack_from('<I', data, content)[0] == OBJHEADER:
                content += 4
            props, off = parse_property_stream(data, content)
            props["__type"] = s
            return props, off

        if maybe_prop in TYPE_TAGS:
            # got: PropName TypeTag, value type struct, no type name written
            props, off = parse_property_stream(data, offset)
            return props, off

        maybe_tag, _ = read_ue3(data, after2)
        if '.' in s and maybe_tag in TYPE_TAGS:
            # got: TypeName PropName TypeTag, so TypeName was struct name
            props, off = parse_property_stream(data, after1)
            props["__type"] = s
            return props, off
    except Exception:
        pass

    return s, after1

def parse_savedata(filepath: str, check_crc: bool = True) -> dict:
    with open(filepath, 'rb') as f:
        raw = f.read()
    off = 0
    part1_len = struct.unpack_from('<I', raw, off)[0]; off += 4
    part1_crc = struct.unpack_from('<I', raw, off)[0]; off += 4
    encrypted = raw[off:off + part1_len]
    crc_actual = crc32(encrypted)
    crc_ok = part1_crc == crc_actual
    if check_crc and not crc_ok:
        print(f"CRC mismatch: expected 0x{part1_crc:08X}, got 0x{crc_actual:08X}", file=sys.stderr)

    dec = aes_decrypt(encrypted)
    off = 0
    foosball = struct.unpack_from('<I', dec, off)[0]; off += 4
    magic    = struct.unpack_from('<I', dec, off)[0]; off += 4
    eng  = struct.unpack_from('<i', dec, off)[0]; off += 4
    lic  = struct.unpack_from('<i', dec, off)[0]; off += 4
    typv = struct.unpack_from('<i', dec, off)[0]; off += 4
    svlen = struct.unpack_from('<i', dec, off)[0]; off += 4
    svdata = dec[off:off + svlen - 4]; off += svlen - 4

    ntypes = struct.unpack_from('<i', dec, off)[0]; off += 4
    objtypes = []
    for _ in range(ntypes):
        tn, off = read_ue3(dec, off)
        fp = struct.unpack_from('<I', dec, off)[0]; off += 4
        oi = struct.unpack_from('<I', dec, off)[0]; off += 4
        objtypes.append({"type": tn, "object_index": oi, "file_position": fp})

    # root Save_TA properties (skip OBJHEADER at start of savedata)
    sdpos = 4
    props, _ = parse_property_stream(svdata, sdpos)

    # each ObjectType entry maps to an object blob in savedata
    objects = []
    for i, ot in enumerate(objtypes):
        obj_pos = ot["file_position"] - 4
        if obj_pos >= len(svdata):
            objects.append({"__type": ot["type"], "__error": "out of range"})
            continue
        end = objtypes[i + 1]["file_position"] - 4 if i + 1 < len(objtypes) else len(svdata)
        obj_bytes = svdata[obj_pos + 4:end]  # skip per-object OBJHEADER
        try:
            oprop, _ = parse_property_stream(obj_bytes, 0)
            oprop["__type"] = ot["type"]
            objects.append(oprop)
        except Exception as e:
            objects.append({"__type": ot["type"], "__parse_error": str(e),
                            "__raw_hex": obj_bytes.hex()})

    return {
        "file_info": {
            "source_file": Path(filepath).name,
            "encrypted_size": part1_len,
            "crc_expected": f"0x{part1_crc:08X}",
            "crc_calculated": f"0x{crc_actual:08X}",
            "crc_match": crc_ok,
        },
        "header": {
            "foosball": f"0x{foosball:08X}",
            "magic": f"0x{magic:08X}",
            "version_info": {"engine_version": eng, "licensee_version": lic,
                             "type_version": typv},
        },
        "object_types": objtypes,
        "properties": props,
        "objects": objects,
    }


