//! Native engine, replacing engine/*.py one module at a time. Actions ported here run in-process; every other action
//! still goes to the Python engine (_engine/SWAPPRISMEngine.exe) through the same JSON protocol.
pub mod aes;
pub mod body;
pub mod colors;
pub mod display;
pub mod dxt;
pub mod epic;
pub mod game_settings;
pub mod hud;
pub mod inventory;
pub mod jobs;
pub mod launcher;
pub mod library;
pub mod media;
pub mod profile;
pub mod save;
pub mod state;
pub mod studio;
pub mod textures;
pub mod upk;
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
        "bootstrap" => bootstrap(root),
        "refresh_state" => state::inspect_state(&root.join("data")).map(|s| serde_json::json!({"state": s})),
        "open_epic" => util::shell_open("com.epicgames.launcher://store/library").map(|_| serde_json::json!({"opened": true})),
        "verify_files" => verify_files(root, request),
        "restore" => restore(root, request),
        "profile_import" => {
            let kind = request["kind"].as_str().unwrap_or("");
            profile::import_image(root, Path::new(request["path"].as_str().unwrap_or("")), kind).map(|path| serde_json::json!({"path": path, "kind": kind}))
        }
        "media_catalog" => media::catalog(root, request["source"].as_str().unwrap_or(""), request["query"].as_str().unwrap_or(""), request["refresh"].as_bool().unwrap_or(false), request["url"].as_str().unwrap_or(""))
            .map(|m| serde_json::json!({"media": m, "sounds": media::audio_library(root)})),
        "media_preview" => media::preview_sound(root, request["key"].as_str().unwrap_or(""), request["kind"].as_str().unwrap_or("post")),
        "media_download" => {
            let source = request["source"].as_str().unwrap_or("");
            media::download(root, source, request["key"].as_str().unwrap_or(""), request["kind"].as_str().unwrap_or("post"), request["preview"].as_bool().unwrap_or(false))
                .and_then(|mut r| { if source != "sounds" { r["packs"] = library::packs(root)?.into(); } Ok(r) })
        }
        "studio_apply" => game(request).and_then(|g| studio::apply(root, &g, &request["settings"], progress)),
        "sound_import" => studio::import_mp3(root, Path::new(request["path"].as_str().unwrap_or("")), request["kind"].as_str().unwrap_or("")),
        "sound_preview" => studio::preview(root, request),
        "profile_apply" => game(request).and_then(|g| profile::apply(root, &g, &request["settings"], progress)),
        "import" => {
            let (found, errors) = library::scan(Path::new(request["path"].as_str().unwrap_or("")));
            library::save_library(&root.join("data").join("texture_library"), &found).and_then(|_| library::packs(root)).map(|packs| serde_json::json!({"packs": packs, "errors": errors}))
        }
        _ => return None,
    })
}

fn verify_files(root: &Path, request: &Value) -> Result<Value, String> {
    let game = util::resolve(Path::new(request["game"].as_str().ok_or("Elige la carpeta del juego en Ajustes.")?));
    let mut result = state::verify_files(&game, &root.join("data"))?;
    let healed: Vec<String> = result["healed"].as_array().map(|a| a.iter().filter_map(|v| v.as_str().map(str::to_owned)).collect()).unwrap_or_default();
    if !healed.is_empty() { profile::after_restore(root, Some(&healed))?; }
    result["profile"] = profile::load(root)?;
    Ok(result)
}

/// Restore selected packages (or all); a restored HUD or ball also restores its companion copies.
fn restore(root: &Path, request: &Value) -> Result<Value, String> {
    require_closed()?;
    let folder = root.join("data");
    let mut names: Option<Vec<String>> = request["names"].as_array().map(|a| a.iter().filter_map(|v| v.as_str().map(str::to_owned)).collect());
    if let Some(list) = names.as_mut() {
        let state = state::load_state(&folder)?;
        for (main, group) in [("GFX_Hud_SF.upk", METER_COMPANIONS.iter().map(|s| s.to_string()).collect()), ("GameInfo_Soccar_SF.upk", ball_companions())] {
            if !list.iter().any(|n| n == main) { continue; }
            for n in group { if state.get(&n).is_some_and(|e| e["hud_display_companion"] == true) && !list.contains(&n) { list.push(n); } }
        }
    }
    let count = state::restore_all(&folder, names.as_deref())?;
    profile::after_restore(root, names.as_deref())?;
    Ok(serde_json::json!({"count": count, "state": state::load_state(&folder)?, "profile": profile::load(root)?, "studio": studio::after_restore(root, names.as_deref())?}))
}

/// Everything the interface needs at start: saved inventory, packs, sounds, change state, profile and studio.
fn bootstrap(root: &Path) -> Result<Value, String> {
    let mut errors = Vec::new();
    let snapshot = match inventory::load_snapshot(jobs::database()) {
        Ok(mut snap) => {
            let source = std::path::PathBuf::from(snap["source"].as_str().unwrap_or(""));
            snap["player_name"] = inventory::player_name(&source).into();
            let selected = snap["account_equipped"].as_array().into_iter().flatten().find(|r| r["Slot"] == "Player Title").and_then(|r| r["Product"].as_str()).unwrap_or("").to_owned();
            let mut ids: Vec<String> = snap["inventory"].as_array().into_iter().flatten()
                .filter(|r| r["blueprint"] != true).filter_map(|r| r["title_id"].as_str().filter(|t| !t.is_empty()).map(str::to_owned)).collect();
            if !selected.is_empty() { ids.push(selected.clone()); }
            ids.sort();
            ids.dedup();
            snap["titles"] = ids.into_iter().map(|k| serde_json::json!({"equipped": k == selected, "id": k})).collect::<Vec<_>>().into();
            snap
        }
        Err(e) => { errors.push(e); serde_json::json!({"inventory": [], "active": null}) }
    };
    let sounds = media::audio_library(root);
    Ok(serde_json::json!({"snapshot": snapshot, "packs": library::packs(root)?, "sounds": sounds, "state": state::inspect_state(&root.join("data"))?,
        "colors": colors::PAINTS.iter().map(|p| p.0).collect::<Vec<_>>(), "errors": errors, "profile": profile::load(root)?, "profile_codec": profile::CODEC_REVISION,
        "studio": studio::load(root)?, "audio_limits": studio::manifest()}))
}

pub const METER_COMPANIONS: [&str; 3] = ["GFxShell_SF.upk", "GFx_ReplayHud_SF.upk", "GFX_SpectatorHud_SF.upk"];

/// Every game mode (training, tutorial, LTMs...) ships its own copy of the ball textures.
pub fn ball_companions() -> Vec<String> {
    let modes = ["Basketball", "BladeBall", "Breakout", "FTE", "FootBall", "GodBall", "HeatseekerTerritory", "Hops", "Items", "KeepUp", "KnockOut",
        "LTM_AprilFool", "LTM_BeachBall", "LTM_BoomerBall", "LTM_Demolition", "LTM_DropshotRumble", "LTM_Eggstra", "LTM_GForce",
        "LTM_Moonball", "LTM_Pinball", "LTM_SpeedDemon", "LTM_SpikeRush", "LTM_SuperCube", "MagnusFutball", "Possession", "Season",
        "SnowDayTerritory", "SpikeDrop", "Territory", "TrainingEditor", "Tutorial"];
    let mut out: Vec<String> = modes.iter().map(|m| format!("GameInfo_{m}_SF.upk")).collect();
    out.push("Gameinfo_Hockey_SF.upk".into());
    out
}

/// The game folder of a request, checked to be a Rocket League installation.
fn game(request: &Value) -> Result<std::path::PathBuf, String> {
    let game = util::resolve(Path::new(request["game"].as_str().unwrap_or("")));
    if !game.join("TAGame").join("CookedPCConsole").is_dir() || !game.join("Binaries").join("Win64").join("RocketLeague.exe").is_file() { return Err("Selecciona la instalación de Rocket League.".into()); }
    Ok(game)
}

/// Package and save writes only with the game closed.
pub fn require_closed() -> Result<(), String> {
    if crate::platform::process_id("RocketLeague.exe").is_some() { Err("Cierra Rocket League para aplicar o restaurar cambios.".into()) } else { Ok(()) }
}
