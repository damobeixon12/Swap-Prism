//! Objects of the installed game that the bundled catalog lacks. The game files hold each object's package and asset
//! path but not its official name or product ID (those come from Psyonix's servers), so these rows carry the internal
//! asset name and an "asset:<path>" key, marked `Unofficial`. Read only; cached in data/catalog-extra.json per game version.
use super::upk::Package;
use super::util;
use serde_json::{json, Value};
use std::collections::HashSet;
use std::path::Path;

type R<T> = Result<T, String>;

/// Catalogue slot for a product class; attachments are told apart by their ProductSlots export or package prefix.
fn slot(class: &str, slots: &[String], package: &str) -> &'static str {
    match class {
        "ProductAsset_Skin_TA" => "Decal",
        "ProductAsset_Wheel_TA" => "Wheels",
        "ProductAsset_Body_TA" => "Body",
        "ProductAsset_Boost_TA" => "Rocket Boost",
        "ProductAsset_GoalExplosion_TA" => "Goal Explosion",
        "ProductAsset_MusicStingers_TA" => "Player Anthem",
        "ProductAsset_SupersonicTrail_TA" => "Trail",
        "ProductAsset_PaintFinish_TA" => "Paint Finish",
        "ProductAsset_PlayerBanner_TA" => "Player Banner",
        "ProductAsset_PlayerAvatarBorder_TA" => "Avatar Border",
        "ProductAsset_EngineAudio_TA" => "Engine Audio",
        "ProductAsset_Container_TA" => "Crate",
        "ProductAsset_Attachment_TA" => {
            let hat = slots.iter().any(|s| s == "ProductSlots.Hat") || package.to_lowercase().starts_with("hat_");
            if hat { "Topper" } else if slots.iter().any(|s| s == "ProductSlots.Antenna") || package.to_lowercase().starts_with("antenna_") { "Antenna" } else { "" }
        }
        _ => "",
    }
}

/// Every product asset in the game: (package, asset path, class, ProductSlots exports). Header reads only, in parallel.
fn game_assets(cooked: &Path) -> R<Vec<(String, String, String, Vec<String>)>> {
    let mut files: Vec<String> = std::fs::read_dir(cooked).map_err(util::err)?.flatten()
        .map(|e| e.file_name().to_string_lossy().into_owned()).filter(|n| n.to_lowercase().ends_with(".upk")).collect();
    files.sort();
    let threads = std::thread::available_parallelism().map_or(4, |n| n.get()).min(8);
    let chunk = files.len().div_ceil(threads).max(1);
    Ok(std::thread::scope(|s| {
        let handles: Vec<_> = files.chunks(chunk).map(|part| s.spawn(move || {
            let mut out = Vec::new();
            for name in part {
                let Ok(p) = Package::open_header(&cooked.join(name)) else { continue };
                if !p.names.iter().any(|n| n.starts_with("ProductAsset_")) { continue; }
                let slots: Vec<String> = p.exports.iter().enumerate().filter(|(_, e)| p.class_name(e) == "ProductSlot_TA").filter_map(|(i, _)| p.path(i).ok()).collect();
                for (i, e) in p.exports.iter().enumerate() {
                    let class = p.class_name(e);
                    if !class.starts_with("ProductAsset_") { continue; }
                    let Ok(path) = p.path(i) else { continue };
                    if path.starts_with("ProductSlots.") || path.starts_with("Default__") { continue; }
                    out.push((name.clone(), path, class, slots.clone()));
                }
            }
            out
        })).collect();
        handles.into_iter().flat_map(|h| h.join().unwrap_or_default()).collect()
    }))
}

/// Rows for the objects the bundled catalog lacks.
fn extra_rows(assets: &[(String, String, String, Vec<String>)]) -> Vec<Value> {
    let base = super::jobs::base_database();
    let known: HashSet<String> = base.iter().filter_map(|r| r["AssetPath"].as_str()).map(str::to_lowercase).collect();
    let body_of = |package: &str| -> Option<String> {
        // body_<car>_premium_skin(s)_SF.upk holds extra decals of <car>; the car's own package is body_<car>_SF.upk.
        let lower = package.to_lowercase();
        let car = lower.strip_suffix("_premium_skins_sf.upk").or(lower.strip_suffix("_premium_skin_sf.upk"))?;
        let own = format!("{car}_sf.upk");
        base.iter().find(|r| r["Slot"] == "Body" && r["AssetPackage"].as_str().is_some_and(|p| p.to_lowercase() == own)).and_then(|r| r["Product"].as_str().map(str::to_owned))
    };
    let mut rows = Vec::new();
    for (package, path, class, slots) in assets {
        if known.contains(&path.to_lowercase()) { continue; }
        let name = path.rsplit('.').next().unwrap_or(path);
        let mut row = json!({"AssetPackage": package, "AssetPath": path, "ID": format!("asset:{path}"), "Product": name,
            "Quality": "", "Slot": slot(class, slots, package), "UnlockMethod": "", "Image": "",
            "Source": "Archivos del juego (sin nombre oficial)", "Unofficial": true});
        if let Some(car) = body_of(package) { row["Body"] = car.into(); }
        rows.push(row);
    }
    rows.sort_by(|a, b| a["AssetPath"].as_str().cmp(&b["AssetPath"].as_str()));
    rows
}

/// The installed game's version: Epic's AppVersionString when known, else the executable's FileVersion.
fn game_version(game: &Path, given: &str) -> String {
    if !given.is_empty() { return given.to_owned(); }
    std::fs::read(game.join("Binaries").join("Win64").join("RocketLeague.exe")).ok().and_then(|exe| {
        let key: Vec<u8> = "FileVersion\0".encode_utf16().flat_map(u16::to_le_bytes).collect();
        let at = exe.windows(key.len()).position(|w| w == key.as_slice())? + key.len();
        let units: Vec<u16> = exe[at..].chunks_exact(2).map(|c| u16::from_le_bytes([c[0], c[1]])).skip_while(|&u| u == 0).take_while(|&u| u != 0).collect();
        Some(String::from_utf16_lossy(&units).replace(", ", "."))
    }).unwrap_or_default()
}

/// Compares the installed game with the catalogue and keeps the missing objects available to the engine and the interface.
pub fn check(root: &Path, game: &Path, version: &str, force: bool) -> R<Value> {
    let cooked = game.join("TAGame").join("CookedPCConsole");
    let version = game_version(game, version);
    let mut names: Vec<String> = std::fs::read_dir(&cooked).map_err(util::err)?.flatten().map(|e| e.file_name().to_string_lossy().into_owned()).collect();
    names.sort();
    let fingerprint = format!("{:x}", <sha2::Sha256 as sha2::Digest>::digest(format!("{version}|{}", names.join("|")).as_bytes()));
    let cache = root.join("data").join("catalog-extra.json");
    let saved: Value = std::fs::read(&cache).ok().and_then(|b| serde_json::from_slice(&b).ok()).unwrap_or(Value::Null);
    let mut result = if !force && saved["fingerprint"] == fingerprint.as_str() { saved } else {
        let assets = game_assets(&cooked)?;
        let extra = extra_rows(&assets);
        let checked = json!({"fingerprint": fingerprint, "game_version": version, "checked_at": util::stamp(),
            "game_items": assets.len(), "catalog_items": super::jobs::base_database().len(), "extra": extra});
        std::fs::create_dir_all(root.join("data")).map_err(util::err)?;
        util::write_text(&cache, &util::dumps(&checked, false))?;
        checked
    };
    super::jobs::set_extra(result["extra"].as_array().cloned().unwrap_or_default());
    result["installed_version"] = version.into();
    Ok(result)
}

/// The last check's rows, so applies in a new session already know them.
pub fn load_saved(root: &Path) {
    static DONE: std::sync::Once = std::sync::Once::new();
    DONE.call_once(|| {
        let saved: Value = std::fs::read(root.join("data").join("catalog-extra.json")).ok().and_then(|b| serde_json::from_slice(&b).ok()).unwrap_or(Value::Null);
        if let Some(extra) = saved["extra"].as_array() { super::jobs::set_extra(extra.clone()); }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn attachment_slots_follow_the_package() {
        assert_eq!(slot("ProductAsset_Attachment_TA", &["ProductSlots.Hat".into()], "x_SF.upk"), "Topper");
        assert_eq!(slot("ProductAsset_Attachment_TA", &[], "Antenna_8Ball_SF.upk"), "Antenna");
        assert_eq!(slot("ProductAsset_Attachment_TA", &[], "odd_SF.upk"), "");
        assert_eq!(slot("ProductAsset_Skin_TA", &[], "body_x_premium_skins_SF.upk"), "Decal");
    }

    #[test]
    fn known_objects_are_not_repeated() {
        let assets = vec![("Antenna_8Ball_SF.upk".to_string(), "Antenna_8Ball.Antenna_8Ball".to_string(), "ProductAsset_Attachment_TA".to_string(), vec![]),
            ("body_octane_premium_skins_SF.upk".to_string(), "body_octane_premium_skins.Skin_Octane_PruebaInexistente".to_string(), "ProductAsset_Skin_TA".to_string(), vec![])];
        let rows = extra_rows(&assets);
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0]["ID"], "asset:body_octane_premium_skins.Skin_Octane_PruebaInexistente");
        assert_eq!(rows[0]["Product"], "Skin_Octane_PruebaInexistente");
        assert_eq!(rows[0]["Slot"], "Decal");
    }

    /// Read only: SP_GAME=<game> cargo test -- --ignored real_catalog_check --nocapture
    #[test]
    #[ignore]
    fn real_catalog_check() {
        let game = std::path::PathBuf::from(std::env::var("SP_GAME").unwrap());
        let root = std::env::temp_dir().join(format!("sp-catalog-{}", std::process::id()));
        let started = std::time::Instant::now();
        let v = check(&root, &game, "", false).unwrap();
        println!("{} objetos en el juego, {} del catálogo, {} añadidos, versión {} en {:?}", v["game_items"], v["catalog_items"], v["extra"].as_array().unwrap().len(), v["installed_version"], started.elapsed());
        for r in v["extra"].as_array().unwrap().iter().take(8) { println!("  {} | {} | {} | {}", r["Slot"], r["Product"], r["Body"], r["AssetPackage"]); }
        let cached = std::time::Instant::now();
        check(&root, &game, "", false).unwrap();
        println!("caché {:?}", cached.elapsed());
        std::fs::remove_dir_all(&root).unwrap();
    }
}
