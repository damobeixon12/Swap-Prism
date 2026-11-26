//! Profile state (data/profile.json). Port of the bookkeeping part of engine/profile_manager.py;
//! generating profile packages comes with the HUD code step.
use super::util::{self, err, R};
use serde_json::{json, Value};
use std::path::Path;

pub const SCHEMA: i64 = 1;

pub fn load(root: &Path) -> R<Value> {
    let path = root.join("data").join("profile.json");
    if !path.exists() { return Ok(json!({"schema": SCHEMA, "settings": {"name": "", "avatar": "", "banner": ""}, "packages": {}})); }
    let value: Value = serde_json::from_slice(&std::fs::read(&path).map_err(err)?).map_err(err)?;
    if value["schema"] != SCHEMA { return Err("Versión de perfil no reconocida.".into()); }
    Ok(value)
}

pub fn save(root: &Path, value: &Value) -> R<()> {
    let path = root.join("data").join("profile.json");
    std::fs::create_dir_all(path.parent().unwrap()).map_err(err)?;
    let tmp = path.with_extension("tmp");
    util::write_text(&tmp, &util::dumps(value, true))?;
    std::fs::rename(&tmp, &path).map_err(err)
}

/// Packages restored outside the profile no longer belong to it.
pub fn after_restore(root: &Path, names: Option<&[String]>) -> R<()> {
    let mut profile = load(root)?;
    let packages = profile["packages"].as_object_mut().ok_or("profile.json no válido.")?;
    let selected: Vec<String> = packages.keys().filter(|k| names.is_none_or(|n| n.contains(k))).cloned().collect();
    for name in &selected { packages.remove(name); }
    let remaining = !packages.is_empty();
    // A partial package restore cannot represent a complete profile installation.
    profile["partial"] = (remaining && !selected.is_empty()).into();
    if !remaining { profile["settings"] = json!({"name": "", "avatar": "", "banner": ""}); profile["partial"] = false.into(); }
    let folder = root.join("data");
    let mut state = super::state::load_state(&folder)?;
    for name in &selected { if let Some(entry) = state.get_mut(name).and_then(Value::as_object_mut) { entry.remove("profile_layer"); } }
    super::state::write_state(&folder, &state)?;
    save(root, &profile)
}
