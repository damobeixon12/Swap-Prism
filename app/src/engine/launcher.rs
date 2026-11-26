//! Start Rocket League with Epic's own launch link or through Steam as a non-Steam shortcut. Port of engine/launcher.py.
//!
//! Steam's shortcuts.vdf is binary KeyValues: 0x00 map, 0x01 string, 0x02 int32, 0x08 end of map.
use super::util::{self, err, R};
use serde_json::{json, Value};
use std::path::{Path, PathBuf};

const NAME: &str = "Rocket League";

pub fn exe(game: &Path) -> R<PathBuf> {
    let path = game.join("Binaries").join("Win64").join("RocketLeague.exe");
    if !path.is_file() { return Err("No se encuentra RocketLeague.exe en la carpeta del juego.".into()); }
    Ok(path)
}

/// Epic's own launch link for this install (namespace:item:app from LauncherInstalled.dat).
pub fn epic_url(game: &Path, installed: Option<&Path>) -> R<String> {
    let default = PathBuf::from(std::env::var_os("ProgramData").unwrap_or_default()).join(r"Epic\UnrealEngineLauncher\LauncherInstalled.dat");
    let installed = installed.unwrap_or(&default);
    let entries: Vec<Value> = std::fs::read(installed).ok().and_then(|raw| serde_json::from_slice::<Value>(&raw).ok())
        .and_then(|v| v["InstallationList"].as_array().cloned()).unwrap_or_default();
    for e in &entries {
        let text = |k: &str| e[k].as_str().filter(|s| !s.is_empty());
        let (Some(app), Some(location)) = (text("AppName"), e["InstallLocation"].as_str()) else { continue };
        if !util::same_path(Path::new(location), game) { continue; }
        let parts: Vec<&str> = match (text("NamespaceId"), text("ItemId")) { (Some(n), Some(i)) => vec![n, i, app], _ => vec![app] };
        return Ok(format!("com.epicgames.launcher://apps/{}?action=launch&silent=true", parts.iter().map(|p| util::quote(p)).collect::<Vec<_>>().join("%3A")));
    }
    Err("Este juego no está instalado con Epic Games. Elige Steam en Ajustes.".into())
}

pub fn play_epic(game: &Path) -> R<Value> {
    util::shell_open(&epic_url(game, None)?)?;
    Ok(json!({"started": "epic"}))
}

fn steam_path() -> Option<PathBuf> {
    let path = PathBuf::from(util::registry_string(r"Software\Valve\Steam", "SteamPath")?);
    path.join("steam.exe").is_file().then_some(path)
}

/// Binary KeyValues keep their order, which matters for writing the file back unchanged.
#[derive(Clone, Debug, PartialEq)]
pub enum Vdf { Map(Vec<(String, Vdf)>), Str(String), Int(i32) }

impl Vdf {
    fn get(&self, key: &str) -> Option<&Vdf> { match self { Vdf::Map(m) => m.iter().find(|e| e.0 == key).map(|e| &e.1), _ => None } }
    fn str(&self, key: &str) -> &str { match self.get(key) { Some(Vdf::Str(s)) => s, _ => "" } }
}

pub fn read_vdf(data: &[u8], mut pos: usize) -> R<(Vdf, usize)> {
    let mut out = Vec::new();
    let cstr = |pos: usize| -> R<(String, usize)> {
        let end = data[pos..].iter().position(|b| *b == 0).map(|n| pos + n).ok_or("shortcuts.vdf truncado.")?;
        Ok((String::from_utf8(data[pos..end].to_vec()).map_err(err)?, end + 1))
    };
    loop {
        let kind = *data.get(pos).ok_or("shortcuts.vdf truncado.")?;
        pos += 1;
        if kind == 8 { return Ok((Vdf::Map(out), pos)); }
        let (key, p) = cstr(pos)?;
        pos = p;
        let value = match kind {
            0 => { let (v, p) = read_vdf(data, pos)?; pos = p; v }
            1 => { let (s, p) = cstr(pos)?; pos = p; Vdf::Str(s) }
            2 => { let v = i32::from_le_bytes(data.get(pos..pos + 4).ok_or("shortcuts.vdf truncado.")?.try_into().unwrap()); pos += 4; Vdf::Int(v) }
            _ => return Err("shortcuts.vdf con un tipo no reconocido.".into()),
        };
        // Python dicts keep the last value of a repeated key, in the first key's position.
        match out.iter_mut().find(|e: &&mut (String, Vdf)| e.0 == key) { Some(e) => e.1 = value, None => out.push((key, value)) }
    }
}

pub fn write_vdf(value: &[(String, Vdf)]) -> Vec<u8> {
    let mut out = Vec::new();
    for (key, item) in value {
        let mut name = key.as_bytes().to_vec();
        name.push(0);
        match item {
            Vdf::Map(m) => { out.push(0); out.extend(name); out.extend(write_vdf(m)); }
            Vdf::Int(i) => { out.push(2); out.extend(name); out.extend(i.to_le_bytes()); }
            Vdf::Str(s) => { out.push(1); out.extend(name); out.extend(s.as_bytes()); out.push(0); }
        }
    }
    out.push(8);
    out
}

/// Steam's id for a non-Steam shortcut: crc32(exe+name) with the top bit set.
pub fn shortcut_id(target: &str) -> u32 { crc32fast::hash(format!("{target}{NAME}").as_bytes()) | 0x8000_0000 }

fn files(steam: &Path) -> Vec<PathBuf> {
    let mut dirs: Vec<PathBuf> = std::fs::read_dir(steam.join("userdata")).into_iter().flatten().flatten().map(|e| e.path())
        .filter(|p| p.file_name().and_then(|n| n.to_str()).is_some_and(|n| !n.is_empty() && n.chars().all(|c| c.is_ascii_digit()) && n != "0")).collect();
    dirs.sort();
    dirs.into_iter().map(|d| d.join("config").join("shortcuts.vdf")).collect()
}

fn shortcuts(vdf: &Path) -> R<Vec<(String, Vdf)>> {
    if !vdf.exists() { return Ok(Vec::new()); }
    let (root, _) = read_vdf(&std::fs::read(vdf).map_err(err)?, 0)?;
    Ok(match root.get("shortcuts") { Some(Vdf::Map(m)) => m.clone(), _ => Vec::new() })
}

fn has_target(entries: &[(String, Vdf)], target: &str) -> bool {
    entries.iter().any(|e| matches!(e.1, Vdf::Map(_)) && e.1.str("Exe").to_lowercase() == target.to_lowercase())
}

fn quoted(path: &Path) -> String { format!("\"{}\"", path.display()) }

pub fn add_to_steam(root: &Path, game: &Path) -> R<Value> {
    let steam = steam_path().ok_or("Steam no está instalado.")?;
    if crate::platform::process_id("steam.exe").is_some() { return Err("Cierra Steam por completo (también en la bandeja) y vuelve a pulsar.".into()); }
    let path = exe(game)?;
    let target = quoted(&path);
    let appid = shortcut_id(&target);
    let vdfs = files(&steam);
    if vdfs.is_empty() { return Err("Inicia sesión en Steam una vez antes de añadir el juego.".into()); }
    let backup = root.join("data").join("steam_backups").join(util::stamp());
    for vdf in vdfs {
        let mut data = shortcuts(&vdf)?;
        if has_target(&data, &target) { continue; }
        if vdf.exists() {
            std::fs::create_dir_all(&backup).map_err(err)?;
            let user = vdf.parent().and_then(Path::parent).and_then(Path::file_name).unwrap_or_default().to_string_lossy().into_owned();
            std::fs::copy(&vdf, backup.join(format!("{user}.vdf"))).map_err(err)?;
        }
        let s = |v: &str| Vdf::Str(v.into());
        let entry = Vdf::Map(vec![
            ("appid".into(), Vdf::Int(appid as i32)), ("AppName".into(), s(NAME)), ("Exe".into(), s(&target)),
            ("StartDir".into(), s(&quoted(path.parent().unwrap()))), ("icon".into(), s(&path.display().to_string())),
            ("ShortcutPath".into(), s("")), ("LaunchOptions".into(), s("")), ("IsHidden".into(), Vdf::Int(0)),
            ("AllowDesktopConfig".into(), Vdf::Int(1)), ("AllowOverlay".into(), Vdf::Int(1)), ("OpenVR".into(), Vdf::Int(0)),
            ("Devkit".into(), Vdf::Int(0)), ("DevkitGameID".into(), s("")), ("DevkitOverrideAppID".into(), Vdf::Int(0)),
            ("LastPlayTime".into(), Vdf::Int(0)), ("FlatpakAppID".into(), s("")), ("tags".into(), Vdf::Map(Vec::new())),
        ]);
        let key = data.len().to_string();
        match data.iter_mut().find(|e| e.0 == key) { Some(e) => e.1 = entry, None => data.push((key, entry)) }
        std::fs::create_dir_all(vdf.parent().unwrap()).map_err(err)?;
        let mut raw = b"\0shortcuts\0".to_vec();
        raw.extend(write_vdf(&data));
        raw.push(8);
        if read_vdf(&raw, 0)?.0.get("shortcuts") != Some(&Vdf::Map(data.clone())) { return Err("No se pudo verificar shortcuts.vdf.".into()); }
        util::replace_file(&vdf, &raw)?;
    }
    status(Some(game))
}

pub fn status(game: Option<&Path>) -> R<Value> {
    let steam = steam_path();
    let mut result = json!({"steam": steam.is_some(), "in_steam": false});
    let (Some(steam), Some(game)) = (steam, game) else { return Ok(result) };
    let Ok(path) = exe(game) else { return Ok(result) };
    let target = quoted(&path);
    for vdf in files(&steam) {
        if vdf.exists() && has_target(&shortcuts(&vdf)?, &target) { result["in_steam"] = true.into(); }
    }
    Ok(result)
}

pub fn play_steam(root: &Path, game: &Path) -> R<Value> {
    if status(Some(game))?["in_steam"] != true { add_to_steam(root, game)?; }
    let gameid = ((shortcut_id(&quoted(&exe(game)?)) as u64) << 32) | 0x0200_0000;
    util::shell_open(&format!("steam://rungameid/{gameid}"))?;
    Ok(json!({"started": "steam"}))
}

/// The engine's launcher_* actions.
pub fn handle(root: &Path, action: &str, request: &Value) -> R<Value> {
    let game = request["game"].as_str().filter(|g| !g.is_empty()).map(|g| util::resolve(Path::new(g)));
    if action == "launcher_status" { return status(game.as_deref()); }
    let game = game.ok_or("Elige la carpeta del juego en Ajustes.")?;
    match action {
        "launcher_play" => if request["via"] == "steam" { play_steam(root, &game) } else { play_epic(&game) },
        "launcher_steam_add" => add_to_steam(root, &game),
        "launcher_steam_play" => play_steam(root, &game),
        _ => Err(format!("Acción no válida: {action}")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn epic_link_from_installed_list() {
        let dir = std::env::temp_dir().join(format!("sp-epic-{}", std::process::id()));
        let game = dir.join("rocketleague");
        std::fs::create_dir_all(&game).unwrap();
        let installed = dir.join("LauncherInstalled.dat");
        std::fs::write(&installed, json!({"InstallationList":[{"InstallLocation":game.display().to_string().to_uppercase(),"AppName":"Sugar","NamespaceId":"9773aa1aa54f4f7b80e44bef04986cea","ItemId":"530145df28a24424923f5828cc9031a1"}]}).to_string()).unwrap();
        assert_eq!(epic_url(&game, Some(&installed)).unwrap(), "com.epicgames.launcher://apps/9773aa1aa54f4f7b80e44bef04986cea%3A530145df28a24424923f5828cc9031a1%3ASugar?action=launch&silent=true");
        assert!(epic_url(&dir, Some(&installed)).is_err());
        let _ = std::fs::remove_dir_all(dir);
    }
    #[test]
    fn vdf_roundtrip_keeps_order() {
        let data = vec![("0".to_string(), Vdf::Map(vec![("appid".into(), Vdf::Int(-5)), ("Exe".into(), Vdf::Str("\"x\"".into())), ("tags".into(), Vdf::Map(vec![]))]))];
        let mut raw = b"\0shortcuts\0".to_vec();
        raw.extend(write_vdf(&data));
        raw.push(8);
        // Same bytes as Python's launcher._write for {'0':{'appid':-5,'Exe':'"x"','tags':{}}}.
        assert_eq!(raw.iter().map(|b| format!("{b:02x}")).collect::<String>(), "0073686f7274637574730000300002617070696400fbffffff01457865002278220000746167730008080808");
        assert_eq!(read_vdf(&raw, 0).unwrap().0, Vdf::Map(vec![("shortcuts".into(), Vdf::Map(data))]));
    }
    #[test]
    fn shortcut_id_matches_python() {
        // launcher.shortcut_id('"C:\\RL.exe"') in Python.
        assert_eq!(shortcut_id("\"C:\\RL.exe\""), 3809033575);
    }
}

/// Parity with the Python engine on this PC: SP_GAME=<game folder> SP_EXPECTED_URL=<python epic_url> cargo test -- --ignored real_epic
#[cfg(test)]
mod parity {
    #[test]
    #[ignore]
    fn real_epic_url_matches_python() {
        let (Some(game), Some(expected)) = (std::env::var_os("SP_GAME"), std::env::var("SP_EXPECTED_URL").ok()) else { return };
        let game = super::util::resolve(std::path::Path::new(&game));
        assert_eq!(super::epic_url(&game, None).unwrap(), expected);
    }
}
