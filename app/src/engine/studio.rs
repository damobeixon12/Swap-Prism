//! Studio settings bookkeeping (data/studio.json). Port of the restore part of engine/studio_backend.py;
//! building colours and sounds comes with the sound step.
use super::util::{self, err, R};
use serde_json::{json, Value};
use std::path::Path;

pub fn manifest() -> Value { serde_json::from_str(include_str!("../../assets/audio_manifest.json")).unwrap_or(json!({})) }

pub fn load(root: &Path) -> R<Value> {
    let path = root.join("data").join("studio.json");
    if !path.exists() { return Ok(json!({})); }
    serde_json::from_slice(&std::fs::read(&path).map_err(err)?).map_err(err)
}

fn truthy(v: &Value) -> bool {
    match v { Value::Null => false, Value::Bool(b) => *b, Value::Object(m) => !m.is_empty(), Value::Array(a) => !a.is_empty(), Value::String(s) => !s.is_empty(), Value::Number(n) => n.as_f64() != Some(0.) }
}

/// Settings whose packages were restored are switched off.
pub fn after_restore(root: &Path, names: Option<&[String]>) -> R<Value> {
    let mut settings = load(root)?;
    if !truthy(&settings) { return Ok(settings); }
    match names {
        None => settings = json!({}),
        Some(names) => {
            let has = |n: &str| names.iter().any(|x| x == n);
            if has("TAGame.upk") {
                settings["reset_enabled"] = false.into();
                if settings["palette"].is_object() { settings["palette"]["enabled"] = false.into(); }
            }
            if has("GFX_StartMenu_SF.upk") { settings["season_enabled"] = false.into(); }
            if let Some(kinds) = manifest().as_object() {
                for (kind, spec) in kinds {
                    if spec["bank"].as_str().is_some_and(has) { if let Some(sounds) = settings.get_mut("sounds").and_then(Value::as_object_mut) { sounds.remove(kind); } }
                }
            }
        }
    }
    util::write_text(&root.join("data").join("studio.json"), &util::dumps(&settings, true))?;
    Ok(settings)
}

#[cfg(test)]
mod tests {
    #[test]
    fn manifest_parses() {
        let parsed: Result<serde_json::Value, _> = serde_json::from_str(include_str!("../../assets/audio_manifest.json"));
        assert!(parsed.is_ok(), "{:?}", parsed.err());
        assert!(super::manifest()["post"]["bank"].is_string());
    }
}
