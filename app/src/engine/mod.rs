//! Native engine, replacing engine/*.py one module at a time. Actions ported here run in-process; every other action
//! still goes to the Python engine (_engine/SWAPPRISMEngine.exe) through the same JSON protocol.
pub mod aes;
pub mod game_settings;
pub mod launcher;
pub mod save;
pub mod util;
pub mod workshop;
pub mod zip;

use serde_json::Value;
use std::path::Path;

/// None when the action is not ported yet. SWAPPRISM_PYTHON_ONLY=1 forces the Python engine (parity checks).
pub fn handle(root: &Path, request: &Value, progress: &dyn Fn(Value)) -> Option<Result<Value, String>> {
    if std::env::var_os("SWAPPRISM_PYTHON_ONLY").is_some() { return None; }
    Some(match request["action"].as_str()? {
        "game_settings" => game_settings::read(None),
        "game_settings_write" => game_settings::write(root, &request["changes"], None),
        action if action.starts_with("launcher_") => launcher::handle(root, action, request),
        "workshop_catalog" => if request["refresh"].as_bool().unwrap_or(false) { workshop::refresh(root) } else { workshop::snapshot(root) },
        "workshop_download" => workshop::download(root, request["key"].as_str().unwrap_or(""), progress),
        "workshop_import" => workshop::import_map(root, Path::new(request["path"].as_str().unwrap_or("")), None),
        "workshop_remove" => workshop::remove(root, request["key"].as_str().unwrap_or("")),
        _ => return None,
    })
}

/// Package and save writes only with the game closed.
pub fn require_closed() -> Result<(), String> {
    if crate::platform::process_id("RocketLeague.exe").is_some() { Err("Cierra Rocket League para aplicar o restaurar cambios.".into()) } else { Ok(()) }
}
