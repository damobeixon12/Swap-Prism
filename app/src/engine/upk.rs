//! Rocket League package (UPK) codec and same-depth ProductAsset rewire. Port of engine/modern_upk.py plus the parts
//! of engine/motor/rl_upk_editor.py (RLUPKTools) it uses. Format/rewiring research: c1yq/RL-Swapper (MIT).
//! Never alters serialized donor object payloads, gameplay or process memory.
use super::aes;
use super::util::{err, R};
use serde_json::{json, Value};
use std::path::{Path, PathBuf};

pub const PACKAGE_FILE_TAG: u32 = 0x9E2A_83C1;
const DEFAULT_KEY: [u8; 32] = [
    0xC7, 0xDF, 0x6B, 0x13, 0x25, 0x2A, 0xCC, 0x71, 0x47, 0xBB, 0x51, 0xC9, 0x8A, 0xD7, 0xE3, 0x4B,
    0x7F, 0xE5, 0x00, 0xB7, 0x7F, 0xA5, 0xFA, 0xB2, 0x93, 0xE2, 0xF2, 0x4E, 0x6B, 0x17, 0xE7, 0x79,
];
const KEYS: &str = include_str!("../../assets/engine/keys.txt");
const KEYS_MAP: &str = include_str!("../../assets/engine/keys_map.json");

pub fn i32_at(d: &[u8], o: usize) -> i32 { i32::from_le_bytes(d[o..o + 4].try_into().unwrap()) }
pub fn i64_at(d: &[u8], o: usize) -> i64 { i64::from_le_bytes(d[o..o + 8].try_into().unwrap()) }
pub fn put32(d: &mut [u8], o: usize, v: i32) { d[o..o + 4].copy_from_slice(&v.to_le_bytes()); }
pub fn put64(d: &mut [u8], o: usize, v: i64) { d[o..o + 8].copy_from_slice(&v.to_le_bytes()); }

/// Windows-1252 bytes 0x80..0x9F (None = undefined, skipped like errors="ignore").
const CP1252_HIGH: [Option<char>; 32] = [
    Some('€'), None, Some('‚'), Some('ƒ'), Some('„'), Some('…'), Some('†'), Some('‡'), Some('ˆ'), Some('‰'), Some('Š'), Some('‹'), Some('Œ'), None, Some('Ž'), None,
    None, Some('‘'), Some('’'), Some('“'), Some('”'), Some('•'), Some('–'), Some('—'), Some('˜'), Some('™'), Some('š'), Some('›'), Some('œ'), None, Some('ž'), Some('Ÿ'),
];
pub fn decode_cp1252(raw: &[u8]) -> String {
    raw.iter().filter_map(|&b| if (0x80..0xA0).contains(&b) { CP1252_HIGH[(b - 0x80) as usize] } else { Some(b as char) }).collect()
}
pub fn encode_cp1252(text: &str) -> R<Vec<u8>> {
    text.chars().map(|c| {
        let code = c as u32;
        if code < 0x80 || (0xA0..0x100).contains(&code) { return Ok(code as u8); }
        CP1252_HIGH.iter().position(|x| *x == Some(c)).map(|i| 0x80 + i as u8).ok_or(format!("'charmap' codec can't encode character '{c}'"))
    }).collect()
}

fn base64(text: &str) -> R<Vec<u8>> {
    let value = |c: u8| -> R<u32> { Ok(match c { b'A'..=b'Z' => c - b'A', b'a'..=b'z' => c - b'a' + 26, b'0'..=b'9' => c - b'0' + 52, b'+' => 62, b'/' => 63, _ => return Err("base64 no válido".into()) } as u32) };
    let bytes: Vec<u8> = text.bytes().filter(|b| !b.is_ascii_whitespace()).collect();
    let data = bytes.iter().position(|b| *b == b'=').map(|p| &bytes[..p]).unwrap_or(&bytes);
    let mut out = Vec::new();
    for chunk in data.chunks(4) {
        let mut acc = 0u32;
        for (i, c) in chunk.iter().enumerate() { acc |= value(*c)? << (18 - 6 * i); }
        let n = chunk.len() * 6 / 8;
        out.extend(&acc.to_be_bytes()[1..1 + n]);
    }
    Ok(out)
}

/// Sequential little-endian reader that fails on short data (like BinaryReader.read_exact).
pub struct Reader<'a> { pub data: &'a [u8], pub pos: usize }
impl<'a> Reader<'a> {
    pub fn take(&mut self, n: usize) -> R<&'a [u8]> {
        let chunk = self.data.get(self.pos..self.pos + n).ok_or(format!("Expected {n} bytes, got {}", self.data.len().saturating_sub(self.pos)))?;
        self.pos += n;
        Ok(chunk)
    }
    pub fn u16(&mut self) -> R<u16> { Ok(u16::from_le_bytes(self.take(2)?.try_into().unwrap())) }
    pub fn u32(&mut self) -> R<u32> { Ok(u32::from_le_bytes(self.take(4)?.try_into().unwrap())) }
    pub fn i32(&mut self) -> R<i32> { Ok(i32::from_le_bytes(self.take(4)?.try_into().unwrap())) }
    pub fn u64(&mut self) -> R<u64> { Ok(u64::from_le_bytes(self.take(8)?.try_into().unwrap())) }
    /// UE3 string: positive length = Windows-1252, negative = UTF-16 (invalid units dropped).
    pub fn fstring(&mut self) -> R<String> {
        let length = self.i32()?;
        if length == 0 { return Ok(String::new()); }
        if length < 0 {
            let raw = self.take((length as i64).unsigned_abs() as usize * 2)?;
            let raw = &raw[..raw.len().saturating_sub(2)];
            let units = raw.chunks_exact(2).map(|c| u16::from_le_bytes([c[0], c[1]]));
            return Ok(char::decode_utf16(units).filter_map(Result::ok).collect());
        }
        let raw = self.take(length as usize - 1)?;
        self.take(1)?;
        Ok(decode_cp1252(raw))
    }
}

#[derive(Clone, Debug, Default)]
pub struct Summary {
    pub file_version: u16,
    pub licensee_version: u16,
    pub total_header_size: i32,
    pub package_flags_offset: usize,
    pub package_flags: u32,
    pub name_count: i32,
    pub name_offset: i32,
    pub export_count: i32,
    pub export_offset: i32,
    pub import_count: i32,
    pub import_offset: i32,
    pub depends_offset: i32,
    pub compression_flags_offset: usize,
}

fn parse_summary(r: &mut Reader) -> R<Summary> {
    let mut s = Summary::default();
    if r.u32()? != PACKAGE_FILE_TAG { return Err("Not a valid Unreal Engine package".into()); }
    s.file_version = r.u16()?;
    s.licensee_version = r.u16()?;
    s.total_header_size = r.i32()?;
    r.fstring()?;
    s.package_flags_offset = r.pos;
    s.package_flags = r.u32()?;
    s.name_count = r.i32()?;
    s.name_offset = r.i32()?;
    s.export_count = r.i32()?;
    s.export_offset = r.i32()?;
    s.import_count = r.i32()?;
    s.import_offset = r.i32()?;
    s.depends_offset = r.i32()?;
    for _ in 0..4 { r.i32()?; } // import/export guids offset and counts, thumbnail table
    r.take(16)?; // guid
    let generations = r.i32()?;
    for _ in 0..generations.max(0) { r.take(12)?; }
    r.u32()?; r.u32()?; // engine, cooker version
    s.compression_flags_offset = r.pos;
    r.u32()?;
    let chunks = r.i32()?;
    for _ in 0..chunks.max(0) { r.take(16)?; }
    r.i32()?;
    let packages = r.i32()?;
    for _ in 0..packages.max(0) { r.fstring()?; }
    let allocations = r.i32()?;
    for _ in 0..allocations.max(0) { r.take(20)?; let n = r.i32()?; for _ in 0..n.max(0) { r.i32()?; } }
    Ok(s)
}

/// Byte offsets of the summary fields a header rewrite must patch.
#[derive(Debug)]
pub struct Offsets {
    pub total_header_size: usize,
    pub name_count: usize,
    pub import_count: usize,
    pub import_offset: usize,
    pub export_offset: usize,
    pub depends_offset: usize,
    pub import_export_guids_offset: usize,
    pub thumbnail_table_offset: usize,
    pub generation_entries: usize,
    pub generation_count: i32,
}

pub fn summary_offsets(data: &[u8]) -> R<Offsets> {
    let mut r = Reader { data, pos: 0 };
    if r.u32()? != PACKAGE_FILE_TAG { return Err("Not a valid Unreal Engine package".into()); }
    r.u16()?; r.u16()?;
    let total_header_size = r.pos; r.i32()?;
    r.fstring()?;
    r.u32()?;
    let name_count = r.pos; r.i32()?;
    r.i32()?; r.i32()?; // name offset, export count
    let export_offset = r.pos; r.i32()?;
    let import_count = r.pos; r.i32()?;
    let import_offset = r.pos; r.i32()?;
    let depends_offset = r.pos; r.i32()?;
    let import_export_guids_offset = r.pos; r.i32()?;
    r.i32()?; r.i32()?;
    let thumbnail_table_offset = r.pos; r.i32()?;
    r.take(16)?;
    let generation_count = r.i32()?;
    Ok(Offsets { total_header_size, name_count, import_count, import_offset, export_offset, depends_offset, import_export_guids_offset, thumbnail_table_offset, generation_entries: r.pos, generation_count })
}

/// The last generation records the export and name counts.
pub fn patch_generation_counts(data: &mut [u8], offsets: &Offsets, export_count: i32, name_count: i32) {
    if offsets.generation_count <= 0 { return; }
    let base = offsets.generation_entries + (offsets.generation_count as usize - 1) * 12;
    if base + 8 > data.len() { return; }
    put32(data, base, export_count);
    put32(data, base + 4, name_count);
}

/// AES-ECB, or AES-CTR when the package carries a nonce.
pub fn crypt(key: &[u8], data: &[u8], nonce: Option<&[u8]>, decrypt: bool) -> R<Vec<u8>> {
    match nonce {
        Some(n) => { let mut counter = [0u8; 16]; counter[..12].copy_from_slice(n); aes::ctr(key, &counter, data) }
        None => if decrypt { aes::ecb_decrypt(key, data) } else { aes::ecb_encrypt(key, data) },
    }
}

pub struct Chunk { pub uoff: i64, pub usize_: i32, pub coff: i64, pub csize: i32, pub nonce: Option<Vec<u8>> }

pub struct Package {
    pub file: PathBuf,
    pub raw: Vec<u8>,
    pub summary: Summary,
    pub meta_offset: usize,
    pub garbage_size: i32,
    pub chunks_offset: i32,
    pub nonce: Option<Vec<u8>>,
    pub stride: usize,
    pub enc_size: usize,
    pub enc_aligned: usize,
    pub key: Vec<u8>,
    pub header: Vec<u8>,
    pub chunks: Vec<Chunk>,
    pub names: Vec<String>,
    pub name_flags: Vec<u64>,
    pub names_end: usize,
    pub imports: Vec<Vec<u8>>,
    pub exports: Vec<Vec<u8>>,
    pub exports_end: usize,
    flat: std::sync::OnceLock<Vec<u8>>,
}

/// Candidate keys: the package's mapped key first, then the default and the known list (deduplicated).
fn keys_for(stem: &str) -> R<Vec<Vec<u8>>> {
    static LIST: std::sync::OnceLock<Vec<Vec<u8>>> = std::sync::OnceLock::new();
    static MAP: std::sync::OnceLock<serde_json::Map<String, Value>> = std::sync::OnceLock::new();
    let list = LIST.get_or_init(|| {
        let mut keys = vec![DEFAULT_KEY.to_vec()];
        keys.extend(KEYS.lines().map(str::trim).filter(|l| !l.is_empty() && !l.starts_with('#')).filter_map(|l| base64(l).ok()));
        keys
    });
    let map = MAP.get_or_init(|| serde_json::from_str(KEYS_MAP).unwrap_or_default());
    let stem = stem.to_lowercase();
    let preferred = map.get(&stem).or_else(|| map.get(stem.strip_suffix("_sf").unwrap_or(&stem)));
    let mut keys: Vec<Vec<u8>> = Vec::with_capacity(list.len() + 1);
    match preferred {
        Some(Value::Number(n)) => if let Some(i) = n.as_i64() { if i + 1 >= 0 && ((i + 1) as usize) < list.len() { keys.push(list[(i + 1) as usize].clone()); } },
        Some(Value::String(s)) => keys.push(base64(s)?),
        _ => {}
    }
    keys.extend(list.iter().cloned());
    let mut seen = std::collections::HashSet::new();
    Ok(keys.into_iter().filter(|k| seen.insert(k.clone())).collect())
}

impl Package {
    pub fn open(file: &Path) -> R<Package> { Package::from_raw(file, std::fs::read(file).map_err(err)?) }

    /// Names, imports and exports only (reads just the header); `flat()` is not available on it.
    pub fn open_header(file: &Path) -> R<Package> {
        use std::io::Read;
        let mut f = std::fs::File::open(file).map_err(err)?;
        let mut raw = vec![0u8; 4096];
        let n = f.read(&mut raw).map_err(err)?;
        raw.truncate(n);
        let size = parse_summary(&mut Reader { data: &raw, pos: 0 })?.total_header_size.max(0) as usize + 16;
        if size > raw.len() {
            f.by_ref().take((size - raw.len()) as u64).read_to_end(&mut raw).map_err(err)?;
        }
        Package::from_raw_sized(file, raw, f.metadata().map_err(err)?.len() as usize)
    }

    pub fn from_raw(file: &Path, raw: Vec<u8>) -> R<Package> { let len = raw.len(); Package::from_raw_sized(file, raw, len) }

    /// `file_len`: size of the whole file when `raw` holds only its header.
    fn from_raw_sized(file: &Path, raw: Vec<u8>, file_len: usize) -> R<Package> {
        let mut r = Reader { data: &raw, pos: 0 };
        let summary = parse_summary(&mut r)?;
        let meta_offset = r.pos;
        let garbage_size = r.i32()?;
        let chunks_offset = r.i32()?;
        r.i32()?;
        let nonce = (summary.package_flags & 0x800 != 0).then(|| raw.get(meta_offset + 12..meta_offset + 24).map(<[u8]>::to_vec)).flatten();
        if summary.package_flags & 0x800 != 0 && nonce.is_none() { return Err("Cabecera cifrada fuera de rango.".into()); }
        let stride = if summary.licensee_version >= 33 || nonce.is_some() { 36 } else { 24 };
        let enc_size = summary.total_header_size as i64 - garbage_size as i64 - summary.name_offset as i64;
        let enc_aligned = (enc_size + 15) & !15;
        if !(0 < enc_aligned && enc_aligned <= raw.len() as i64 - summary.name_offset as i64) { return Err("Cabecera cifrada fuera de rango.".into()); }
        let (enc_size, enc_aligned) = (enc_size as usize, enc_aligned as usize);
        let name_offset = summary.name_offset as usize;
        let encrypted = &raw[name_offset..name_offset + enc_aligned];
        let stem = file.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
        let offset = chunks_offset as i64;
        let (block_start, inner) = ((offset - offset.rem_euclid(16)) as usize, offset.rem_euclid(16) as usize);
        let mut found = None;
        if 0 <= offset && offset <= enc_aligned as i64 - 12 {
            let probe = &encrypted[block_start..(block_start + 32).min(enc_aligned)];
            for key in keys_for(&stem)? {
                if key.len() != 32 { continue; }
                // Only the block holding the chunk table is decrypted per candidate key.
                let plain = match &nonce {
                    Some(n) => { let mut counter = [0u8; 16]; counter[..12].copy_from_slice(n); aes::ctr_at(&key, &counter, (block_start / 16) as u128, probe)? }
                    None => aes::ecb_decrypt(&key, probe)?,
                };
                if plain.len() >= inner + 12 && (1..=512).contains(&i32_at(&plain, inner)) && i64_at(&plain, inner + 4) == summary.depends_offset as i64 { found = Some(key); break; }
            }
        }
        let Some(key) = found else { return Err(format!("No se reconoce el cifrado de {}", file.file_name().unwrap_or_default().to_string_lossy())) };
        let mut header = crypt(&key, encrypted, nonce.as_deref(), true)?;
        header.truncate(enc_size);
        let mut chunks = Vec::new();
        let count = i32_at(&header, chunks_offset as usize);
        for index in 0..count.max(0) as usize {
            let pos = chunks_offset as usize + 4 + index * stride;
            let (uoff, usize_, coff, csize) = (i64_at(&header, pos), i32_at(&header, pos + 8), i64_at(&header, pos + 12), i32_at(&header, pos + 20));
            let chunk_nonce = (stride == 36).then(|| header[pos + 24..pos + 36].to_vec());
            if uoff.min(usize_ as i64).min(coff).min(csize as i64) < 0 || coff + csize as i64 > file_len as i64 || uoff + usize_ as i64 > 512 * 1024 * 1024 { return Err("Chunk fuera de rango.".into()); }
            chunks.push(Chunk { uoff, usize_, coff, csize, nonce: chunk_nonce });
        }
        let mut reader = Reader { data: &header, pos: 0 };
        let (mut names, mut name_flags) = (Vec::new(), Vec::new());
        for _ in 0..summary.name_count.max(0) { names.push(reader.fstring()?); name_flags.push(reader.u64()?); }
        let names_end = reader.pos;
        let base = (summary.import_offset - summary.name_offset) as usize;
        let imports = (0..summary.import_count.max(0) as usize).map(|i| header[base + i * 28..base + (i + 1) * 28].to_vec()).collect();
        let mut exports = Vec::new();
        let mut cursor = (summary.export_offset - summary.name_offset) as usize;
        let limit = (summary.depends_offset - summary.name_offset) as usize;
        for _ in 0..summary.export_count.max(0) {
            let net = i32_at(&header, cursor + 48);
            let length = 72 + net as i64 * 4;
            if net < 0 || cursor as i64 + length > limit as i64 { return Err("Tabla de exports no válida.".into()); }
            exports.push(header[cursor..cursor + length as usize].to_vec());
            cursor += length as usize;
        }
        Ok(Package { file: file.to_path_buf(), summary, meta_offset, garbage_size, chunks_offset, nonce, stride, enc_size, enc_aligned, key, header, chunks, names, name_flags, names_end, imports, exports, exports_end: cursor, raw, flat: std::sync::OnceLock::new() })
    }

    pub fn fname(&self, index: i32, instance: i32) -> String {
        let name = self.names.get(index as usize).cloned().unwrap_or_default();
        if instance > 0 { format!("{name}_{}", instance - 1) } else { name }
    }
    pub fn export_name(&self, export: &[u8]) -> String { self.fname(i32_at(export, 12), i32_at(export, 16)) }
    pub fn class_name(&self, export: &[u8]) -> String {
        let index = i32_at(export, 0);
        if index < 0 { let imported = &self.imports[(-index - 1) as usize]; return self.fname(i32_at(imported, 20), i32_at(imported, 24)); }
        if index > 0 { self.export_name(&self.exports[index as usize - 1]) } else { "Class".into() }
    }
    pub fn path(&self, mut position: usize) -> R<String> {
        let (mut parts, mut seen) = (Vec::new(), std::collections::HashSet::new());
        loop {
            if !seen.insert(position) || position >= self.exports.len() { return Err("Cadena de exports cíclica o fuera de rango.".into()); }
            let export = &self.exports[position];
            parts.push(self.export_name(export));
            let outer = i32_at(export, 8);
            if outer <= 0 { break; }
            position = outer as usize - 1;
        }
        parts.reverse();
        Ok(parts.join("."))
    }
    pub fn find_asset(&self, asset_path: &str) -> R<usize> {
        let want = asset_path.to_lowercase();
        let mut matches = Vec::new();
        for i in 0..self.exports.len() { if self.path(i)?.to_lowercase() == want { matches.push(i); } }
        let [one] = matches[..] else { return Err(format!("No se puede identificar de forma unívoca el ProductAsset: {asset_path}")) };
        Ok(one)
    }

    /// The whole package decompressed (and decrypted), cached.
    pub fn flat(&self) -> R<&[u8]> {
        if let Some(f) = self.flat.get() { return Ok(f); }
        let s = &self.summary;
        let size = self.chunks.iter().map(|c| (c.uoff + c.usize_ as i64) as usize).max().unwrap_or(0);
        let mut flat = vec![0u8; size.max(s.depends_offset as usize)];
        let name_offset = s.name_offset as usize;
        flat[..name_offset].copy_from_slice(&self.raw[..name_offset]);
        let head = (s.depends_offset - s.name_offset) as usize;
        flat[name_offset..name_offset + head].copy_from_slice(self.header.get(..head).ok_or("Cabecera incompleta.")?);
        for chunk in &self.chunks {
            let raw = &self.raw[chunk.coff as usize..(chunk.coff + chunk.csize as i64) as usize];
            let payload = match &self.nonce { Some(_) => crypt(&self.key, raw, chunk.nonce.as_deref(), true)?, None => raw.to_vec() };
            if payload.len() < 16 || u32::from_le_bytes(payload[..4].try_into().unwrap()) != PACKAGE_FILE_TAG { return Err("No se reconoce la cabecera de compresión.".into()); }
            let total = i32_at(&payload, 12) as i64;
            let (mut cursor, mut amount, mut blocks) = (16usize, 0i64, Vec::new());
            while amount < total {
                let (comp, uncomp) = (i32_at(&payload, cursor), i32_at(&payload, cursor + 4));
                if comp <= 0 || uncomp <= 0 { return Err("Tamaño de bloque no válido.".into()); }
                blocks.push((comp as usize, uncomp as usize));
                amount += uncomp as i64;
                cursor += 8;
            }
            let mut data = Vec::with_capacity(chunk.usize_ as usize);
            for (comp, uncomp) in blocks {
                let before = data.len();
                std::io::Read::read_to_end(&mut flate2::read::ZlibDecoder::new(payload.get(cursor..cursor + comp).ok_or("Bloque fuera de rango.")?), &mut data).map_err(err)?;
                if data.len() - before != uncomp { return Err("El bloque descomprimido no coincide con su tamaño.".into()); }
                cursor += comp;
            }
            if data.len() != chunk.usize_ as usize { return Err("El chunk descomprimido no coincide con su tamaño.".into()); }
            flat[chunk.uoff as usize..chunk.uoff as usize + data.len()].copy_from_slice(&data);
        }
        put32(&mut flat, s.compression_flags_offset, 0);
        flat[s.package_flags_offset..s.package_flags_offset + 4].copy_from_slice(&(s.package_flags & !8).to_le_bytes());
        Ok(self.flat.get_or_init(|| flat))
    }

    /// Serialized bytes of one export inside the flat package.
    pub fn payload(&self, index: usize) -> R<&[u8]> {
        let e = &self.exports[index];
        let (size, offset) = (i32_at(e, 32) as usize, i64_at(e, 36) as usize);
        self.flat()?.get(offset..offset + size).ok_or("Export fuera de rango.".into())
    }
}

/// Same-depth outer-chain rewire only, fail closed for complex packages.
pub fn rewire(donor: &Package, target: &Package, donor_item: &Value, target_item: &Value) -> R<(Vec<u8>, Value)> {
    let target_path = target_item["AssetPath"].as_str().unwrap_or("");
    let donor_position = donor.find_asset(donor_item["AssetPath"].as_str().unwrap_or(""))?;
    let target_position = target.find_asset(target_path)?;
    let donor_class = donor.class_name(&donor.exports[donor_position]);
    if donor_class != target.class_name(&target.exports[target_position]) || !donor_class.starts_with("ProductAsset_") {
        return Err("Las clases del objeto cosmético no son compatibles.".into());
    }
    let mut changes: Vec<(usize, String, i32)> = Vec::new();
    let (mut dindex, mut tindex) = (donor_position, target_position);
    loop {
        let (dexp, texp) = (&donor.exports[dindex], &target.exports[tindex]);
        changes.push((dindex, target.names[i32_at(texp, 12) as usize].clone(), i32_at(texp, 16)));
        let (dout, tout) = (i32_at(dexp, 8), i32_at(texp, 8));
        if dout <= 0 || tout <= 0 {
            if dout != tout { return Err("Profundidad de paquetes distinta; este par necesita edición adicional.".into()); }
            break;
        }
        dindex = dout as usize - 1;
        tindex = tout as usize - 1;
    }
    let stem = |p: &Package| p.file.file_stem().map(|s| s.to_string_lossy().to_lowercase()).unwrap_or_default();
    for (index, export) in donor.exports.iter().enumerate() {
        if i32_at(export, 8) == 0 && donor.export_name(export).to_lowercase() == stem(donor) {
            let roots: Vec<&Vec<u8>> = target.exports.iter().filter(|e| i32_at(e, 8) == 0 && target.export_name(e).to_lowercase() == stem(target)).collect();
            if let [root] = roots[..] { changes.push((index, target.names[i32_at(root, 12) as usize].clone(), i32_at(root, 16))); }
        }
    }
    let mut names = donor.names.clone();
    let mut additions = Vec::new();
    let mut exports = donor.exports.clone();
    let flags = donor.name_flags[i32_at(&donor.exports[donor_position], 12) as usize];
    for (position, name, instance) in &changes {
        let lower = name.to_lowercase();
        let name_index = match names.iter().position(|n| n.to_lowercase() == lower) {
            Some(i) => i,
            None => {
                let mut encoded = encode_cp1252(name)?;
                encoded.push(0);
                additions.extend((encoded.len() as i32).to_le_bytes());
                additions.extend(&encoded);
                additions.extend(flags.to_le_bytes());
                names.push(name.clone());
                names.len() - 1
            }
        };
        put32(&mut exports[*position], 12, name_index as i32);
        put32(&mut exports[*position], 16, *instance);
    }
    let delta = additions.len() as i64;
    let s = &donor.summary;
    let nameoff = s.name_offset as usize;
    let exportrel = (s.export_offset - s.name_offset) as usize;
    for export in exports.iter_mut() { if i64_at(export, 36) > 0 { let v = i64_at(export, 36) + delta; put64(export, 36, v); } }
    let mut header = donor.header[..donor.names_end].to_vec();
    header.extend(&additions);
    header.extend(&donor.header[donor.names_end..exportrel]);
    for e in &exports { header.extend(e); }
    header.extend(&donor.header[donor.exports_end..]);
    let chunkoffset = donor.chunks_offset as usize + delta as usize;
    for index in 0..donor.chunks.len() { let pos = chunkoffset + 4 + index * donor.stride; let v = i64_at(&header, pos) + delta; put64(&mut header, pos, v); }
    let newaligned = (header.len() + 15) & !15;
    let growth = newaligned as i64 - donor.enc_aligned as i64;
    let gap = s.total_header_size as i64 - nameoff as i64 - donor.enc_aligned as i64;
    let garbage = donor.garbage_size as i64;
    let extra = if growth > gap || delta > garbage { ((growth - gap).max(delta - garbage).max(0) + 255) & !15 } else { 0 };
    for index in 0..donor.chunks.len() { let pos = chunkoffset + 4 + index * donor.stride; let v = i64_at(&header, pos + 12) + extra; put64(&mut header, pos + 12, v); }
    header.resize(newaligned, 0);
    let mut prefix = donor.raw[..nameoff].to_vec();
    let offsets = summary_offsets(&donor.raw)?;
    put32(&mut prefix, offsets.total_header_size, (s.total_header_size as i64 + extra) as i32);
    put32(&mut prefix, offsets.name_count, names.len() as i32);
    put32(&mut prefix, offsets.import_offset, (s.import_offset as i64 + delta) as i32);
    put32(&mut prefix, offsets.export_offset, (s.export_offset as i64 + delta) as i32);
    put32(&mut prefix, offsets.depends_offset, (s.depends_offset as i64 + delta) as i32);
    for at in [offsets.import_export_guids_offset, offsets.thumbnail_table_offset] {
        if i32_at(&prefix, at) > 0 { let v = i32_at(&prefix, at) + delta as i32; put32(&mut prefix, at, v); }
    }
    patch_generation_counts(&mut prefix, &offsets, s.export_count, names.len() as i32);
    put32(&mut prefix, donor.meta_offset, (garbage + extra - delta) as i32);
    put32(&mut prefix, donor.meta_offset + 4, chunkoffset as i32);
    let guid_pos = offsets.import_export_guids_offset + 16;
    let target_guid_pos = summary_offsets(&target.raw)?.import_export_guids_offset + 16;
    prefix[guid_pos..guid_pos + 16].copy_from_slice(&target.raw[target_guid_pos..target_guid_pos + 16]);
    let existing_gap = &donor.raw[nameoff + donor.enc_aligned..s.total_header_size as usize];
    let newgap = (gap + extra - growth).max(0) as usize;
    let mut output = prefix;
    output.extend(crypt(&target.key, &header, donor.nonce.as_deref(), false)?);
    output.extend(&existing_gap[..newgap.min(existing_gap.len())]);
    output.extend(std::iter::repeat_n(0u8, newgap.saturating_sub(existing_gap.len())));
    output.extend(&donor.raw[s.total_header_size as usize..]);
    if donor.nonce.is_some() && donor.key != target.key {
        for index in 0..donor.chunks.len() {
            let pos = chunkoffset + 4 + index * donor.stride;
            let (coff, csize) = (i64_at(&header, pos + 12) as usize, i32_at(&header, pos + 20) as usize);
            let nonce = header[pos + 24..pos + 36].to_vec();
            let plain = crypt(&donor.key, &output[coff..coff + csize], Some(&nonce), true)?;
            let again = crypt(&target.key, &plain, Some(&nonce), false)?;
            output[coff..coff + csize].copy_from_slice(&again);
        }
    }
    let checked = Package::from_raw(&target.file, output)?;
    checked.find_asset(target_path)?;
    for (index, (old, new)) in donor.exports.iter().zip(&checked.exports).enumerate() {
        if i32_at(old, 32) != i32_at(new, 32) || donor.payload(index)? != checked.payload(index)? {
            return Err("Ha cambiado un contenido serializado del donante; operación rechazada.".into());
        }
    }
    let output = checked.raw;
    use sha2::{Digest, Sha256};
    Ok((output.clone(), json!({"method": "productasset-export-rewire", "exports": exports.len(), "serialized_payloads_unchanged": true,
        "target_asset_path": target_path, "donor_sha256": format!("{:x}", Sha256::digest(&donor.raw)), "generated_sha256": format!("{:x}", Sha256::digest(&output))})))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn cp1252_roundtrip() {
        assert_eq!(decode_cp1252(&[b'A', 0x80, 0x81, 0xE9]), "A€é");
        assert_eq!(encode_cp1252("A€é").unwrap(), vec![b'A', 0x80, 0xE9]);
        assert!(encode_cp1252("中").is_err());
    }
    #[test]
    fn base64_decodes() {
        assert_eq!(base64("aGVsbG8=").unwrap(), b"hello");
        assert_eq!(base64("x99rEyUqzHFHu1HJitfjS3/lALd/pfqyk+LyTmsX53k=").unwrap(), DEFAULT_KEY.to_vec());
    }
}

/// Parity with the Python codec on real game packages:
/// SP_UPK_EXPECTED=<json from the Python side> cargo test -- --ignored real_packages
#[cfg(test)]
mod parity {
    use super::*;
    use sha2::{Digest, Sha256};
    #[test]
    #[ignore]
    fn real_packages_match_python() {
        let Some(path) = std::env::var_os("SP_UPK_EXPECTED") else { return };
        let expected: Vec<Value> = serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
        let mut failures = Vec::new();
        for want in &expected {
            let file = PathBuf::from(want["path"].as_str().unwrap());
            let got = Package::open(&file).and_then(|p| {
                let flat = p.flat()?;
                let paths: Vec<String> = (0..p.exports.len()).map(|i| p.path(i)).collect::<R<_>>()?;
                let classes: Vec<String> = p.exports.iter().map(|e| p.class_name(e)).collect();
                Ok(json!({"names": p.names.len(), "exports": p.exports.len(), "key": format!("{:x}", Sha256::digest(&p.key)),
                    "header": format!("{:x}", Sha256::digest(&p.header)), "flat": format!("{:x}", Sha256::digest(flat)),
                    "paths": format!("{:x}", Sha256::digest(paths.join("\n").as_bytes())), "classes": format!("{:x}", Sha256::digest(classes.join("\n").as_bytes())),
                    "names_hash": format!("{:x}", Sha256::digest(p.names.join("\n").as_bytes()))}))
            });
            let mut want = want.clone();
            want.as_object_mut().unwrap().remove("path");
            match got { Ok(g) if g == want => {}, Ok(g) => failures.push(format!("{}: {g} != {want}", file.display())), Err(e) => failures.push(format!("{}: {e}", file.display())) }
        }
        println!("{} paquetes comparados", expected.len());
        assert!(failures.is_empty(), "{}", failures.join("\n"));
    }
}

/// SP_REWIRE_EXPECTED=<json from the Python side> cargo test -- --ignored real_rewire
#[cfg(test)]
mod rewire_parity {
    use super::*;
    use sha2::{Digest, Sha256};
    #[test]
    #[ignore]
    fn real_rewire_matches_python() {
        let Some(path) = std::env::var_os("SP_REWIRE_EXPECTED") else { return };
        let expected: Vec<Value> = serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
        let mut failures = Vec::new();
        for want in &expected {
            let got = (|| {
                let donor = Package::open(Path::new(want["donor_file"].as_str().unwrap()))?;
                let target = Package::open(Path::new(want["target_file"].as_str().unwrap()))?;
                rewire(&donor, &target, &want["donor_item"], &want["target_item"])
            })();
            let ok = match (&got, want.get("sha256")) {
                (Ok((raw, v)), Some(sha)) => format!("{:x}", Sha256::digest(raw)) == sha.as_str().unwrap() && *v == want["validation"],
                (Err(e), None) => Some(e.as_str()) == want["error"].as_str(),
                _ => false,
            };
            if !ok { failures.push(format!("{} {}: {:?} vs {}", want["slot"], want["target_item"]["AssetPath"], got.map(|g| g.1), want)); }
        }
        println!("{} pares comparados", expected.len());
        assert!(failures.is_empty(), "{}", failures.join("\n"));
    }
}
