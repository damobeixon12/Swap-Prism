//! Original SHA-1 of every game file, read from Epic's .egstore/*.manifest (read-only). Port of engine/epic_manifest.py.
use super::util::{err, R};
use std::collections::HashMap;
use std::io::Read;
use std::path::{Path, PathBuf};

const MAGIC: u32 = 0x44BE_C00C;

struct Reader<'a> { data: &'a [u8], pos: usize }
impl<'a> Reader<'a> {
    fn take(&mut self, n: usize) -> R<&'a [u8]> {
        if self.pos + n > self.data.len() { return Err("Manifiesto de Epic truncado.".into()); }
        let chunk = &self.data[self.pos..self.pos + n];
        self.pos += n;
        Ok(chunk)
    }
    fn u8(&mut self) -> R<u8> { Ok(self.take(1)?[0]) }
    fn u32(&mut self) -> R<u32> { Ok(u32::from_le_bytes(self.take(4)?.try_into().unwrap())) }
    fn i32(&mut self) -> R<i32> { Ok(i32::from_le_bytes(self.take(4)?.try_into().unwrap())) }
    fn fstring(&mut self) -> R<String> {
        let size = self.i32()?;
        if size == 0 { return Ok(String::new()); }
        if size < 0 {
            let raw = self.take((size as i64).unsigned_abs() as usize * 2)?;
            let units: Vec<u16> = raw.chunks_exact(2).map(|c| u16::from_le_bytes([c[0], c[1]])).collect();
            return Ok(String::from_utf16(&units).map_err(err)?.trim_end_matches('\0').to_owned());
        }
        // ascii with 'replace': every non-ASCII byte becomes U+FFFD.
        Ok(self.take(size as usize)?.iter().map(|&b| if b < 0x80 { b as char } else { '\u{FFFD}' }).collect::<String>().trim_end_matches('\0').to_owned())
    }
}

/// {relative path (forward slashes, lowercase): sha1 hex}.
pub fn parse(path: &Path) -> R<HashMap<String, String>> {
    let raw = std::fs::read(path).map_err(err)?;
    let mut head = Reader { data: &raw, pos: 0 };
    if head.u32()? != MAGIC { return Err("No es un manifiesto binario de Epic.".into()); }
    let header_size = head.u32()? as usize;
    head.u32()?;
    head.u32()?;
    head.take(20)?;
    let stored_as = head.u8()?;
    let mut body = raw.get(header_size..).ok_or("Manifiesto de Epic truncado.")?.to_vec();
    if stored_as & 1 != 0 {
        let mut out = Vec::new();
        flate2::read::ZlibDecoder::new(&body[..]).read_to_end(&mut out).map_err(err)?;
        body = out;
    }
    let mut r = Reader { data: &body, pos: 0 };
    let start = r.pos; r.pos = start + r.u32()? as usize; // ManifestMeta
    let start = r.pos; r.pos = start + r.u32()? as usize; // ChunkDataList
    r.u32()?; r.u8()?;
    let count = r.u32()? as usize; // FileManifestList: size, version, count
    let names: Vec<String> = (0..count).map(|_| r.fstring()).collect::<R<_>>()?;
    for _ in 0..count { r.fstring()?; } // symlink targets
    let mut out = HashMap::with_capacity(count);
    for name in names {
        let digest: String = r.take(20)?.iter().map(|b| format!("{b:02x}")).collect();
        out.insert(name.replace('\\', "/").to_lowercase(), digest);
    }
    Ok(out)
}

/// Epic manifest for this install, or None (Steam or missing .egstore).
pub fn find(game: &Path) -> Option<PathBuf> {
    let folder = game.join(".egstore");
    let mut manifests: Vec<(std::time::SystemTime, PathBuf)> = std::fs::read_dir(&folder).ok()?.flatten()
        .filter(|e| e.path().extension().is_some_and(|x| x == "manifest"))
        .map(|e| (e.metadata().and_then(|m| m.modified()).unwrap_or(std::time::UNIX_EPOCH), e.path())).collect();
    manifests.sort_by(|a, b| a.0.cmp(&b.0));
    manifests.pop().map(|m| m.1)
}

/// Epic's SHA-1 for a CookedPCConsole package, or None when there is no Epic manifest.
pub fn expected(game: &Path, name: &str) -> R<Option<String>> {
    static CACHE: std::sync::Mutex<Option<((PathBuf, std::time::SystemTime), HashMap<String, String>)>> = std::sync::Mutex::new(None);
    let Some(manifest) = find(game) else { return Ok(None) };
    let key = (manifest.clone(), std::fs::metadata(&manifest).and_then(|m| m.modified()).map_err(err)?);
    let mut cache = CACHE.lock().unwrap();
    if cache.as_ref().is_none_or(|c| c.0 != key) { *cache = Some((key, parse(&manifest)?)); }
    Ok(cache.as_ref().unwrap().1.get(&format!("tagame/cookedpcconsole/{}", name.to_lowercase())).cloned())
}

pub fn sha1(path: &Path) -> R<String> { super::util::hash_file(path, "SHA1") }
