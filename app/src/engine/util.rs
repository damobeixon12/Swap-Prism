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

pub fn sha256_file(path: &Path) -> R<String> {
    use sha2::{Digest, Sha256};
    use std::io::Read;
    let mut file = std::fs::File::open(path).map_err(err)?;
    let mut hash = Sha256::new();
    let mut buf = vec![0u8; 1 << 20];
    loop {
        let n = file.read(&mut buf).map_err(err)?;
        if n == 0 { break; }
        hash.update(&buf[..n]);
    }
    Ok(format!("{:x}", hash.finalize()))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn quote_matches_python() {
        assert_eq!(quote("0a4d:Sugar b/ñ"), "0a4d%3ASugar%20b%2F%C3%B1");
    }
}
