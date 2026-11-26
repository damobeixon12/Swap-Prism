//! The engine: every action of the JSON protocol, run in-process (formerly the Python engine in _engine/).
pub mod aes;
pub mod apply;
pub mod body;
pub mod catalog;
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

pub fn handle(root: &Path, request: &Value, progress: &dyn Fn(Value)) -> Result<Value, String> {
    catalog::load_saved(root);
    match request["action"].as_str().unwrap_or("") {
        "catalog_check" => catalog::check(root, &game(request)?, request["version"].as_str().unwrap_or(""), request["force"] == true),
        "game_settings" => game_settings::read(None),
        "game_settings_write" => game_settings::write(root, &request["changes"], None),
        action if action.starts_with("launcher_") => launcher::handle(root, action, request),
        "workshop_catalog" => if request["refresh"].as_bool().unwrap_or(false) { workshop::refresh(root) } else { workshop::snapshot(root) },
        "workshop_download" => workshop::download(root, request["key"].as_str().unwrap_or(""), progress),
        "workshop_import" => workshop::import_map(root, Path::new(request["path"].as_str().unwrap_or("")), None),
        "workshop_remove" => workshop::remove(root, request["key"].as_str().unwrap_or("")),
        "bootstrap" => bootstrap(root),
        "apply" => apply::apply(root, request, progress),
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
        _ => Err("Acción no disponible.".into()),
    }
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
        // Ball companions are recognised by their label, so copies from any package list are restored.
        let balls: Vec<String> = state.iter()
            .filter(|(_, e)| e["target_item"]["Product"].as_str().is_some_and(|p| p.starts_with("Pelota · "))).map(|(k, _)| k.clone()).collect();
        for (main, group) in [("GFX_Hud_SF.upk", METER_COMPANIONS.iter().map(|s| s.to_string()).collect()), ("GameInfo_Soccar_SF.upk", balls)] {
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
    let snapshot = match inventory::load_snapshot(&jobs::database()) {
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
/// Every package (other than GameInfo_Soccar) carrying its own copy of the default ball texture: game modes, maps and menu
/// backgrounds. The game keeps the first copy it loads (usually the map's), so all of them need the custom ball.
/// Found by scanning package headers; cached in data/ball-packages.json until the set of game files changes.
pub fn ball_companions(cooked: &Path, data: &Path) -> Result<Vec<String>, String> {
    // Cosmetic packages never carry the default ball.
    const SKIP: [&str; 16] = ["skin", "wheel", "antenna", "hat", "boost", "explosion", "playerbanner", "avatarborder", "paintfinish", "ss", "body", "title", "anthem", "trail", "engine", "decal"];
    let mut files: Vec<String> = std::fs::read_dir(cooked).map_err(|e| e.to_string())?.flatten()
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .filter(|n| n.to_lowercase().ends_with(".upk") && !SKIP.contains(&n.split('_').next().unwrap_or("").to_lowercase().as_str()))
        .collect();
    files.sort();
    let fingerprint = format!("{:x}", <sha2::Sha256 as sha2::Digest>::digest(files.join("|").as_bytes()));
    let cache = data.join("ball-packages.json");
    if let Ok(saved) = std::fs::read(&cache).map_err(|e| e.to_string()).and_then(|b| serde_json::from_slice::<Value>(&b).map_err(|e| e.to_string())) {
        if saved["fingerprint"] == fingerprint.as_str() {
            return Ok(saved["packages"].as_array().into_iter().flatten().filter_map(|v| v.as_str().map(str::to_owned)).collect());
        }
    }
    let threads = std::thread::available_parallelism().map_or(4, |n| n.get()).min(8);
    let chunk = files.len().div_ceil(threads).max(1);
    let mut found: Vec<String> = std::thread::scope(|s| {
        let handles: Vec<_> = files.chunks(chunk).map(|part| s.spawn(move || part.iter().filter(|name| {
            !name.eq_ignore_ascii_case("GameInfo_Soccar_SF.upk") && upk::Package::open_header(&cooked.join(name)).is_ok_and(|p| {
                p.names.iter().any(|n| n == "Ball_Default00_D")
                    && p.exports.iter().any(|e| p.export_name(e) == "Ball_Default00_D" && p.class_name(e) == "Texture2D")
            })
        }).cloned().collect::<Vec<_>>())).collect();
        handles.into_iter().flat_map(|h| h.join().unwrap_or_default()).collect()
    });
    found.sort();
    std::fs::create_dir_all(data).map_err(|e| e.to_string())?;
    util::write_text(&cache, &util::dumps(&serde_json::json!({"fingerprint": fingerprint, "packages": found}), false))?;
    Ok(found)
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

#[cfg(test)]
mod tests {
    /// Read only: SP_GAME=<game folder> cargo test -- --ignored real_ball_companions
    #[test]
    #[ignore]
    fn real_ball_companions() {
        let game = std::path::PathBuf::from(std::env::var("SP_GAME").unwrap());
        let data = std::env::temp_dir().join(format!("sp-ball-{}", std::process::id()));
        let started = std::time::Instant::now();
        let found = super::ball_companions(&game.join("TAGame").join("CookedPCConsole"), &data).unwrap();
        println!("{} paquetes en {:?}: {:?}", found.len(), started.elapsed(), found);
        let cached = std::time::Instant::now();
        assert_eq!(super::ball_companions(&game.join("TAGame").join("CookedPCConsole"), &data).unwrap(), found);
        println!("caché {:?}", cached.elapsed());
        std::fs::remove_dir_all(&data).unwrap();
        assert!(found.iter().any(|n| n == "MENU_Main_p.upk") && found.iter().any(|n| n == "GameInfo_TrainingEditor_SF.upk"));
    }

    /// Read only on the game: SP_GAME=<game> SP_LIBRARY=<data	exture_library> cargo test -- --ignored real_ball_timing
    #[test]
    #[ignore]
    fn real_ball_timing() {
        let game = std::path::PathBuf::from(std::env::var("SP_GAME").unwrap());
        let pack = super::library::load_library(std::path::Path::new(&std::env::var("SP_LIBRARY").unwrap())).into_iter().find(|p| p.category == "Pelota").unwrap();
        let root = std::env::temp_dir().join(format!("sp-balltime-{}", std::process::id()));
        std::fs::create_dir_all(root.join("data")).unwrap();
        // Packages already modified on this PC are skipped (their source would be the user's backups).
        let cooked = game.join("TAGame").join("CookedPCConsole");
        let taken: Vec<String> = super::ball_companions(&cooked, &root.join("data")).unwrap().into_iter()
            .filter(|n| super::state::is_epic_original(&cooked, n).unwrap() != Some(true)).map(|n| n.to_lowercase()).collect();
        let started = std::time::Instant::now();
        let jobs = super::jobs::generate_ball_companions(&root, &game, &pack, &root.join("out"), &taken).unwrap();
        println!("{} paquetes en {:?} ({} omitidos por estar modificados)", jobs.len(), started.elapsed(), taken.len());
        std::fs::remove_dir_all(&root).unwrap();
    }
}
