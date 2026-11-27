//! Disk use of the app, and cleanup of what can be regenerated. Backups of originals and the user's library
//! (textures, sounds, maps, profile pictures) are only measured, never deleted.
use super::util::R;
use serde_json::{json, Value};
use std::path::{Path, PathBuf};

fn size(path: &Path) -> u64 {
    match std::fs::symlink_metadata(path) {
        Ok(m) if m.is_dir() => std::fs::read_dir(path).into_iter().flatten().flatten().map(|e| size(&e.path())).sum(),
        Ok(m) => m.len(),
        Err(_) => 0,
    }
}

/// Leftovers that are safe to delete when nothing is running: prepared packages of failed applies,
/// downloaded updates already installed, and temporary copies from interrupted transactions or previews.
fn disposable(root: &Path) -> Vec<PathBuf> {
    let mut list = vec![root.join("paquetes"), root.join("data").join("updates")];
    list.extend(std::fs::read_dir(std::env::temp_dir()).into_iter().flatten().flatten()
        .filter(|e| { let n = e.file_name().to_string_lossy().to_lowercase(); n.starts_with("localskins-transaction-") || n.starts_with("sp-preview-") })
        .map(|e| e.path()));
    list.retain(|p| p.exists());
    list
}

pub fn report(root: &Path) -> R<Value> {
    let data = root.join("data");
    let library: u64 = ["texture_library", "sounds", "workshop", "profile"].iter().map(|d| size(&data.join(d))).sum();
    Ok(json!({"backups": size(&data.join("backups")), "library": library, "temporary": disposable(root).iter().map(|p| size(p)).sum::<u64>()}))
}

pub fn clean(root: &Path) -> R<Value> {
    let before = report(root)?["temporary"].as_u64().unwrap_or(0);
    for path in disposable(root) {
        // Files in use (an open preview) are skipped; the rest goes.
        let _ = if path.is_dir() { std::fs::remove_dir_all(&path) } else { std::fs::remove_file(&path) };
    }
    let mut result = report(root)?;
    result["freed"] = before.saturating_sub(result["temporary"].as_u64().unwrap_or(0)).into();
    Ok(result)
}

#[cfg(test)]
mod tests {
    #[test]
    fn cleans_only_disposable() {
        let root = std::env::temp_dir().join(format!("sp-storage-{}", std::process::id()));
        for d in ["paquetes/job", "data/backups", "data/sounds"] { std::fs::create_dir_all(root.join(d)).unwrap(); }
        for f in ["paquetes/job/a.upk", "data/backups/b.upk", "data/sounds/c.mp3"] { std::fs::write(root.join(f), b"12345").unwrap(); }
        let result = super::clean(&root).unwrap();
        assert_eq!(result["freed"], 5);
        assert!(!root.join("paquetes").exists());
        assert!(root.join("data/backups/b.upk").exists() && root.join("data/sounds/c.mp3").exists());
        std::fs::remove_dir_all(&root).unwrap();
    }
}
