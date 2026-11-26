//! Rocket League save format: CRC, AES and UE3 tagged property streams.
//! Port of engine/save_reader.py (adaptation of nixvio64/RLSaveViewer; format research: Drogebot/RocketRP).
use serde_json::{Map, Value};

pub const AES_KEY: [u8; 32] = [
    0xD7, 0x8C, 0x32, 0x4A, 0x94, 0x42, 0x94, 0x3C, 0x6D, 0x65, 0xCE, 0x98, 0x81, 0x85, 0x4C, 0x41,
    0x68, 0x99, 0x22, 0x0C, 0xC7, 0xA1, 0x46, 0x40, 0x93, 0x9B, 0x96, 0x3C, 0x93, 0x2A, 0x6F, 0xAF,
];
const CRC_SEED: u32 = 0xEFCB_F201;
const OBJHEADER: u32 = 0xFFFF_FFFF;
pub const TYPE_TAGS: [&str; 10] = ["BoolProperty", "IntProperty", "QWordProperty", "FloatProperty", "StrProperty", "NameProperty", "ByteProperty", "ObjectProperty", "StructProperty", "ArrayProperty"];

pub type R<T> = Result<T, String>;

/// CRC32 as RocketRP computes it (MSB-first, polynomial 0x04C11DB7, seeded).
pub fn crc32(data: &[u8]) -> u32 {
    static TABLE: std::sync::OnceLock<[u32; 256]> = std::sync::OnceLock::new();
    let table = TABLE.get_or_init(|| {
        let mut t = [0u32; 256];
        for (i, slot) in t.iter_mut().enumerate() {
            let mut c = (i as u32) << 24;
            for _ in 0..8 { c = if c & 0x8000_0000 != 0 { (c << 1) ^ 0x04C1_1DB7 } else { c << 1 }; }
            *slot = c;
        }
        t
    });
    let mut crc = !CRC_SEED;
    for &b in data { crc = (crc << 8) ^ table[((crc >> 24) as u8 ^ b) as usize]; }
    !crc
}

pub fn bytes_at<const N: usize>(d: &[u8], off: usize) -> R<[u8; N]> {
    d.get(off..off + N).and_then(|s| s.try_into().ok()).ok_or_else(|| format!("Fin de datos inesperado en {off}."))
}
pub fn i32_at(d: &[u8], off: usize) -> R<i32> { Ok(i32::from_le_bytes(bytes_at(d, off)?)) }
pub fn u32_at(d: &[u8], off: usize) -> R<u32> { Ok(u32::from_le_bytes(bytes_at(d, off)?)) }
fn u64_at(d: &[u8], off: usize) -> R<u64> { Ok(u64::from_le_bytes(bytes_at(d, off)?)) }
fn byte_at(d: &[u8], off: usize) -> R<u8> { d.get(off).copied().ok_or_else(|| format!("Fin de datos inesperado en {off}.")) }
/// Floats are reported rounded to 6 decimals, as the Python engine did.
fn f32_at(d: &[u8], off: usize) -> R<f64> { Ok(round6(f32::from_le_bytes(bytes_at(d, off)?) as f64)) }
pub fn round6(v: f64) -> f64 { (v * 1e6).round() / 1e6 }

/// UE3 string: int32 length (negative = UTF-16) + characters + terminator.
pub fn read_ue3(d: &[u8], off: usize) -> R<(String, usize)> {
    let length = i32_at(d, off)?;
    let off = off + 4;
    if length == 0 { return Ok((String::new(), off)); }
    if length < 0 {
        let count = (length as i64).unsigned_abs() as usize * 2;
        let mut raw = &d[off.min(d.len())..(off + count).min(d.len())];
        if raw.len() >= 2 && raw[raw.len() - 2..] == [0, 0] { raw = &raw[..raw.len() - 2]; }
        if raw.len() % 2 != 0 { return Err("Texto UTF-16 truncado.".into()); }
        let units: Vec<u16> = raw.chunks_exact(2).map(|c| u16::from_le_bytes([c[0], c[1]])).collect();
        return Ok((String::from_utf16(&units).map_err(|_| "Texto UTF-16 no válido.".to_string())?, off + count));
    }
    let length = length as usize;
    let raw = &d[off.min(d.len())..(off + length - 1).min(d.len())];
    Ok((std::str::from_utf8(raw).map_err(|_| "Texto UTF-8 no válido.".to_string())?.to_owned(), off + length))
}

pub fn write_ue3(s: &str) -> Vec<u8> {
    let mut out = ((s.len() + 1) as i32).to_le_bytes().to_vec();
    out.extend_from_slice(s.as_bytes());
    out.push(0);
    out
}

/// Tagged properties until "None"; repeated names / fixed-array indices become lists.
pub fn parse_property_stream(d: &[u8], mut off: usize) -> R<(Map<String, Value>, usize)> {
    let mut props = Map::new();
    let mut fixed: Vec<(String, std::collections::BTreeMap<i32, Value>)> = Vec::new();
    loop {
        let (name, next) = read_ue3(d, off)?;
        off = next;
        if name == "None" { break; }
        let tag_off = off;
        let (tag, next) = read_ue3(d, off)?;
        off = next;
        let (value, vidx);
        if !TYPE_TAGS.contains(&tag.as_str()) {
            off = tag_off;
            let (mut inner, next) = parse_property_stream(d, off)?;
            off = next;
            inner.insert("__type".into(), Value::String(name.clone()));
            value = Value::Object(inner);
            vidx = 0;
        } else {
            let vlen = i32_at(d, off)?;
            vidx = i32_at(d, off + 4)?;
            off += 8;
            let (v, next) = parse_value(d, off, &tag, vlen)?;
            value = v;
            off = next;
        }
        let in_fixed = fixed.iter().position(|f| f.0 == name);
        if vidx != 0 || props.contains_key(&name) || in_fixed.is_some() {
            let slot = match in_fixed { Some(i) => i, None => { fixed.push((name.clone(), Default::default())); fixed.len() - 1 } };
            fixed[slot].1.insert(vidx, value);
        } else {
            props.insert(name, value);
        }
    }
    for (name, mut map) in fixed {
        if let Some(first) = props.remove(&name) { map.insert(0, first); }
        props.insert(name, Value::Array(map.into_values().collect()));
    }
    Ok((props, off))
}

pub fn parse_value(d: &[u8], off: usize, tag: &str, vlen: i32) -> R<(Value, usize)> {
    Ok(match tag {
        "BoolProperty" => (Value::Bool(byte_at(d, off)? != 0), off + 1),
        "IntProperty" => (i32_at(d, off)?.into(), off + 4),
        "QWordProperty" => (u64_at(d, off)?.into(), off + 8),
        "FloatProperty" => (f32_at(d, off)?.into(), off + 4),
        "StrProperty" | "NameProperty" => { let (s, o) = read_ue3(d, off)?; (s.into(), o) }
        "ByteProperty" => {
            let (kind, o) = read_ue3(d, off)?;
            if kind == "None" { (byte_at(d, o)?.into(), o + 1) } else { let (s, o) = read_ue3(d, o)?; (s.into(), o) }
        }
        "ObjectProperty" => (i32_at(d, off)?.into(), off + 4),
        "StructProperty" => parse_struct(d, off)?,
        "ArrayProperty" => parse_array(d, off, vlen)?,
        _ => return Err(format!("Unknown tag {tag:?} at offset {off}")),
    })
}

fn typed(mut props: Map<String, Value>, kind: &str) -> Value {
    props.insert("__type".into(), kind.into());
    Value::Object(props)
}

fn parse_struct(d: &[u8], off: usize) -> R<(Value, usize)> {
    let (kind, off) = read_ue3(d, off)?;
    match kind.as_str() {
        "Vector" => return Ok((serde_json::json!({"x": f32_at(d, off)?, "y": f32_at(d, off + 4)?, "z": f32_at(d, off + 8)?}), off + 12)),
        "Rotator" => return Ok((serde_json::json!({"pitch": f32_at(d, off)?, "yaw": f32_at(d, off + 4)?, "roll": f32_at(d, off + 8)?}), off + 12)),
        "Guid" => {
            let p: Vec<u32> = (0..4).map(|i| u32_at(d, off + i * 4)).collect::<R<_>>()?;
            return Ok((format!("{:08X}-{:08X}-{:08X}-{:08X}", p[0], p[1], p[2], p[3]).into(), off + 16));
        }
        _ => {}
    }
    let start = if u32_at(d, off)? == OBJHEADER { off + 4 } else { off };
    let (props, off) = parse_property_stream(d, start)?;
    Ok((typed(props, &kind), off))
}

fn parse_array(d: &[u8], off: usize, vlen: i32) -> R<(Value, usize)> {
    let count = i32_at(d, off)?;
    let mut off = off + 4;
    if count <= 0 { return Ok((Value::Array(Vec::new()), off)); }
    let payload = vlen as i64 - 4;
    let hint = if payload > 0 { payload.div_euclid(count as i64) } else { 0 };
    let mut out = Vec::with_capacity(count as usize);
    for _ in 0..count {
        match hint {
            4 => { out.push(i32_at(d, off)?.into()); off += 4; }
            1 => { out.push(byte_at(d, off)?.into()); off += 1; }
            8 => { out.push(u64_at(d, off)?.into()); off += 8; }
            _ => { let (v, o) = parse_array_elem(d, off)?; out.push(v); off = o; }
        }
    }
    Ok((Value::Array(out), off))
}

/// Array elements carry no type tag: sniff the layout exactly as the Python reader did.
fn parse_array_elem(d: &[u8], off: usize) -> R<(Value, usize)> {
    let Ok((s, after1)) = read_ue3(d, off) else { return Ok((i32_at(d, off)?.into(), off + 4)) };
    if s == "None" {
        return if read_ue3(d, after1).is_ok() { Ok((Value::Object(Map::new()), after1)) } else { Ok((byte_at(d, after1)?.into(), after1 + 1)) };
    }
    if s.is_empty() { return Ok((s.into(), after1)); }
    if after1 + 4 <= d.len() && u32_at(d, after1)? == OBJHEADER {
        let (props, o) = parse_property_stream(d, after1 + 4)?;
        return Ok((typed(props, &s), o));
    }
    let sniff = || -> R<Option<(Value, usize)>> {
        let (maybe_prop, after2) = read_ue3(d, after1)?;
        if maybe_prop == "StructProperty" {
            let mut content = after2 + 8;
            if content + 4 <= d.len() && u32_at(d, content)? == OBJHEADER { content += 4; }
            let (props, o) = parse_property_stream(d, content)?;
            return Ok(Some((typed(props, &s), o)));
        }
        if TYPE_TAGS.contains(&maybe_prop.as_str()) {
            let (props, o) = parse_property_stream(d, off)?;
            return Ok(Some((Value::Object(props), o)));
        }
        let (maybe_tag, _) = read_ue3(d, after2)?;
        if s.contains('.') && TYPE_TAGS.contains(&maybe_tag.as_str()) {
            let (props, o) = parse_property_stream(d, after1)?;
            return Ok(Some((typed(props, &s), o)));
        }
        Ok(None)
    };
    if let Ok(Some(found)) = sniff() { return Ok(found); }
    Ok((s.into(), after1))
}

#[cfg(test)]
mod tests {
    use super::*;
    fn prop(name: &str, tag: &str, value: &[u8]) -> Vec<u8> {
        let mut out = write_ue3(name);
        out.extend(write_ue3(tag));
        out.extend((value.len() as i32).to_le_bytes());
        out.extend(0i32.to_le_bytes());
        out.extend(value);
        out
    }
    #[test]
    fn crc_matches_known_value() {
        // RocketRP CRC of an empty buffer is the complement of the complemented seed.
        assert_eq!(crc32(&[]), CRC_SEED);
        assert_ne!(crc32(b"a"), crc32(b"b"));
    }
    #[test]
    fn strings_roundtrip() {
        let raw = write_ue3("Hola");
        assert_eq!(read_ue3(&raw, 0).unwrap(), ("Hola".into(), raw.len()));
        let mut wide = (-3i32).to_le_bytes().to_vec();
        wide.extend([b'a', 0, 0xf1, 0, 0, 0]);
        assert_eq!(read_ue3(&wide, 0).unwrap().0, "añ");
    }
    #[test]
    fn property_stream_with_repeats() {
        let mut d = prop("FOV", "FloatProperty", &110f32.to_le_bytes());
        d.extend(prop("On", "BoolProperty", &[1]));
        d.extend(prop("Rank", "IntProperty", &3i32.to_le_bytes()));
        d.extend(prop("Rank", "IntProperty", &5i32.to_le_bytes()));
        d.extend(write_ue3("None"));
        let (props, end) = parse_property_stream(&d, 0).unwrap();
        assert_eq!(end, d.len());
        // Same quirk as the Python reader: a repeated index-0 name keeps the first value.
        assert_eq!(Value::Object(props), serde_json::json!({"FOV": 110.0, "On": true, "Rank": [3]}));
    }
}
