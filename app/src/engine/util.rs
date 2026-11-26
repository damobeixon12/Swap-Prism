//! Small helpers shared by the native engine modules.
use std::path::{Path, PathBuf};

pub type R<T> = Result<T, String>;

pub fn err<E: std::fmt::Display>(e: E) -> String { e.to_string() }

/// Local time as Python's time.strftime('%Y%m%d-%H%M%S').
pub fn stamp() -> String {
    let mut t: windows_sys::Win32::Foundation::SYSTEMTIME = unsafe { std::mem::zeroed() };
    unsafe { windows_sys::Win32::System::SystemInformation::GetLocalTime(&mut t) };
    format!("{:04}{:02}{:02}-{:02}{:02}{:02}", t.wYear, t.wMonth, t.wDay, t.wHour, t.wMinute, t.wSecond)
}

/// Absolute path with real casing and no \\?\ prefix, as Python's Path.resolve() gives on Windows.
pub fn resolve(path: &Path) -> PathBuf {
    let full = std::fs::canonicalize(path).unwrap_or_else(|_| std::path::absolute(path).unwrap_or_else(|_| path.to_path_buf()));
    let text = full.display().to_string();
    match text.strip_prefix(r"\\?\UNC\") {
        Some(unc) => PathBuf::from(format!(r"\\{unc}")),
        None => PathBuf::from(text.strip_prefix(r"\\?\").unwrap_or(&text)),
    }
}

/// Windows paths compare without case, as WindowsPath does.
pub fn same_path(a: &Path, b: &Path) -> bool { resolve(a).display().to_string().to_lowercase() == resolve(b).display().to_string().to_lowercase() }

/// urllib.parse.quote(text, safe='').
pub fn quote(text: &str) -> String {
    text.bytes().map(|b| if b.is_ascii_alphanumeric() || b"_.-~".contains(&b) { (b as char).to_string() } else { format!("%{b:02X}") }).collect()
}

/// os.startfile: open a file or URL with its default handler.
pub fn shell_open(target: &str) -> R<()> {
    let wide: Vec<u16> = target.encode_utf16().chain([0]).collect();
    let verb: Vec<u16> = "open\0".encode_utf16().collect();
    let code = unsafe { windows_sys::Win32::UI::Shell::ShellExecuteW(std::ptr::null_mut(), verb.as_ptr(), wide.as_ptr(), std::ptr::null(), std::ptr::null(), 1) } as isize;
    if code <= 32 { Err(format!("No se pudo abrir {target} (código {code}).")) } else { Ok(()) }
}

/// A string value under HKEY_CURRENT_USER.
pub fn registry_string(key: &str, name: &str) -> Option<String> {
    use windows_sys::Win32::System::Registry::*;
    let key: Vec<u16> = key.encode_utf16().chain([0]).collect();
    let name: Vec<u16> = name.encode_utf16().chain([0]).collect();
    let mut size = 0u32;
    unsafe {
        if RegGetValueW(HKEY_CURRENT_USER, key.as_ptr(), name.as_ptr(), RRF_RT_REG_SZ, std::ptr::null_mut(), std::ptr::null_mut(), &mut size) != 0 { return None; }
        let mut buf = vec![0u16; size as usize / 2 + 1];
        if RegGetValueW(HKEY_CURRENT_USER, key.as_ptr(), name.as_ptr(), RRF_RT_REG_SZ, std::ptr::null_mut(), buf.as_mut_ptr() as *mut _, &mut size) != 0 { return None; }
        let text = String::from_utf16_lossy(&buf);
        Some(text.trim_end_matches('\0').to_owned())
    }
}

/// Write through a temporary file and an atomic replace.
pub fn replace_file(path: &Path, data: &[u8]) -> R<()> {
    let tmp = path.with_extension("swapprism.tmp");
    std::fs::write(&tmp, data).map_err(err)?;
    std::fs::rename(&tmp, path).map_err(err)
}

/// json.dumps(value, indent=2, ensure_ascii=ascii): same layout as the Python engine wrote.
pub fn dumps(value: &serde_json::Value, ascii: bool) -> String {
    let text = serde_json::to_string_pretty(value).unwrap_or_default();
    if !ascii { return text; }
    let mut out = String::with_capacity(text.len());
    for c in text.chars() {
        if c.is_ascii() { out.push(c); continue; }
        let mut units = [0u16; 2];
        for u in c.encode_utf16(&mut units) { out.push_str(&format!("\\u{u:04x}")); }
    }
    out
}

/// json.dumps(value, ensure_ascii=False) on one line: ", " and ": " separators.
pub fn dumps_compact(value: &serde_json::Value) -> String {
    use serde_json::Value;
    match value {
        Value::Array(items) => format!("[{}]", items.iter().map(dumps_compact).collect::<Vec<_>>().join(", ")),
        Value::Object(map) => format!("{{{}}}", map.iter().map(|(k, v)| format!("{}: {}", serde_json::to_string(k).unwrap_or_default(), dumps_compact(v))).collect::<Vec<_>>().join(", ")),
        other => serde_json::to_string(other).unwrap_or_default(),
    }
}

/// A random 32-hex-digit name (uuid4().hex) from the OS generator.
pub fn uuid_hex() -> String {
    let mut bytes = [0u8; 16];
    unsafe { windows_sys::Win32::Security::Cryptography::BCryptGenRandom(std::ptr::null_mut(), bytes.as_mut_ptr(), 16, windows_sys::Win32::Security::Cryptography::BCRYPT_USE_SYSTEM_PREFERRED_RNG) };
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

/// Text written the way Python's text mode does on Windows (CRLF line breaks, UTF-8).
pub fn write_text(path: &Path, text: &str) -> R<()> { std::fs::write(path, text.replace('\n', "\r\n")).map_err(err) }

/// File hash through Windows CNG (hardware SHA extensions, like Python's OpenSSL-backed hashlib).
pub fn hash_file(path: &Path, algorithm: &str) -> R<String> {
    use std::io::Read;
    use windows_sys::Win32::Security::Cryptography::*;
    let mut file = std::fs::File::open(path).map_err(err)?;
    let alg_name: Vec<u16> = algorithm.encode_utf16().chain([0]).collect();
    unsafe {
        let mut alg: BCRYPT_ALG_HANDLE = std::ptr::null_mut();
        if BCryptOpenAlgorithmProvider(&mut alg, alg_name.as_ptr(), std::ptr::null(), 0) != 0 { return Err(format!("{algorithm} no disponible.")); }
        let result = (|| {
            let mut hash: BCRYPT_HASH_HANDLE = std::ptr::null_mut();
            if BCryptCreateHash(alg, &mut hash, std::ptr::null_mut(), 0, std::ptr::null(), 0, 0) != 0 { return Err(format!("{algorithm} no disponible.")); }
            let mut buf = vec![0u8; 1 << 20];
            let digest = (|| {
                loop {
                    let n = file.read(&mut buf).map_err(err)?;
                    if n == 0 { break; }
                    if BCryptHashData(hash, buf.as_ptr(), n as u32, 0) != 0 { return Err("Error al calcular el hash.".to_string()); }
                }
                let size = if algorithm == "SHA1" { 20 } else { 32 };
                let mut out = vec![0u8; size];
                if BCryptFinishHash(hash, out.as_mut_ptr(), size as u32, 0) != 0 { return Err("Error al calcular el hash.".to_string()); }
                Ok(out.iter().map(|b| format!("{b:02x}")).collect())
            })();
            BCryptDestroyHash(hash);
            digest
        })();
        BCryptCloseAlgorithmProvider(alg, 0);
        result
    }
}

pub fn sha256_file(path: &Path) -> R<String> { hash_file(path, "SHA256") }

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn file_hashes_match_known_values() {
        let path = std::env::temp_dir().join(format!("sp-hash-{}", std::process::id()));
        std::fs::write(&path, b"abc").unwrap();
        assert_eq!(sha256_file(&path).unwrap(), "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad");
        assert_eq!(hash_file(&path, "SHA1").unwrap(), "a9993e364706816aba3e25717850c26c9cd0d89d");
        let _ = std::fs::remove_file(path);
    }
    #[test]
    fn quote_matches_python() {
        assert_eq!(quote("0a4d:Sugar b/ñ"), "0a4d%3ASugar%20b%2F%C3%B1");
    }
}
