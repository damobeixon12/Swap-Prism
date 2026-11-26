//! Camera, controller and video settings, edited in the local save without opening the game. Port of engine/game_settings.py.
//!
//! Save layout: [len][crc][AES-ECB: 24-byte header, savedata, object table, zero padding]. Edits rewrite one tagged
//! property (or insert it when the game left it at its default), then fix the enclosing struct sizes, the object table
//! and the CRC. Every write keeps a backup.
use super::save::{self as sr, i32_at, u32_at, R};
use serde_json::{json, Map, Value};
use std::path::{Path, PathBuf};

/// (key, object type, path inside it, property type) — names checked against TAGame.upk.
const FIELDS: [(&str, &str, &[&str], &str); 18] = [
    ("camera.fov", "TAGame.ProfileCameraSave_TA", &["Camera", "FOV"], "FloatProperty"),
    ("camera.distance", "TAGame.ProfileCameraSave_TA", &["Camera", "Distance"], "FloatProperty"),
    ("camera.height", "TAGame.ProfileCameraSave_TA", &["Camera", "Height"], "FloatProperty"),
    ("camera.pitch", "TAGame.ProfileCameraSave_TA", &["Camera", "Pitch"], "FloatProperty"),
    ("camera.stiffness", "TAGame.ProfileCameraSave_TA", &["Camera", "Stiffness"], "FloatProperty"),
    ("camera.swivel", "TAGame.ProfileCameraSave_TA", &["Camera", "SwivelSpeed"], "FloatProperty"),
    ("camera.transition", "TAGame.ProfileCameraSave_TA", &["Camera", "TransitionSpeed"], "FloatProperty"),
    ("camera.shake", "TAGame.ProfileCameraSave_TA", &["bEnableCameraShake"], "BoolProperty"),
    ("camera.invert_swivel", "TAGame.ProfileCameraSave_TA", &["bInvertSwivelPitch"], "BoolProperty"),
    ("controls.deadzone", "TAGame.ProfileGamepadSave_TA", &["ControllerDeadzone"], "FloatProperty"),
    ("controls.dodge_deadzone", "TAGame.ProfileGamepadSave_TA", &["DodgeInputThreshold"], "FloatProperty"),
    ("controls.steering", "TAGame.ProfileGamepadSave_TA", &["SteeringSensitivity"], "FloatProperty"),
    ("controls.aerial", "TAGame.ProfileGamepadSave_TA", &["AirControlSensitivity"], "FloatProperty"),
    ("controls.vibration", "TAGame.ProfileControlsSave_TA", &["ForceFeedbackScale"], "FloatProperty"),
    ("video.max_fps", "TAGame.VideoSettingsSavePC_TA", &["MaxFPS"], "IntProperty"),
    ("video.weather", "TAGame.VideoSettingsSavePC_TA", &["bShowWeatherFX"], "BoolProperty"),
    ("video.light_shafts", "TAGame.VideoSettingsSavePC_TA", &["bShowLightShafts"], "BoolProperty"),
    ("video.lens_flares", "TAGame.VideoSettingsSavePC_TA", &["bShowLensFlares"], "BoolProperty"),
];
/// System ini keys, [SystemSettings] only.
const INI_FIELDS: [(&str, &str); 5] = [("video.res_x", "ResX"), ("video.res_y", "ResY"), ("video.fullscreen", "Fullscreen"), ("video.borderless", "Borderless"), ("video.vsync", "UseVsync")];

fn field(key: &str) -> Option<&'static (&'static str, &'static str, &'static [&'static str], &'static str)> { FIELDS.iter().find(|f| f.0 == key) }

/// The game's documents folder; SWAPPRISM_GAME_DOCS overrides it for tests on copies.
pub fn folder() -> PathBuf {
    if let Some(path) = std::env::var_os("SWAPPRISM_GAME_DOCS") { return PathBuf::from(path); }
    PathBuf::from(std::env::var_os("USERPROFILE").unwrap_or_default()).join("Documents").join("My Games").join("Rocket League").join("TAGame")
}

/// Every copy of the profile save (the game keeps a second '_2' copy), newest first.
pub fn saves(base: &Path) -> Vec<PathBuf> {
    let mut found: Vec<(std::time::SystemTime, PathBuf)> = Vec::new();
    for dir in ["SaveDataEpic", "SaveData"] {
        for sub in std::fs::read_dir(base.join(dir)).into_iter().flatten().flatten() {
            for file in std::fs::read_dir(sub.path()).into_iter().flatten().flatten() {
                let path = file.path();
                if path.extension().is_some_and(|e| e == "save") && path.is_file() {
                    found.push((file.metadata().and_then(|m| m.modified()).unwrap_or(std::time::UNIX_EPOCH), path));
                }
            }
        }
    }
    found.sort_by(|a, b| b.0.cmp(&a.0));
    found.into_iter().map(|f| f.1).collect()
}

pub struct Save {
    head: Vec<u8>,
    pub data: Vec<u8>,
    pub table: Vec<(String, u32, u32)>,
}

impl Save {
    pub fn new(raw: &[u8]) -> R<Save> {
        let size = u32_at(raw, 0)? as usize;
        let body = raw.get(8..8 + size).ok_or("La partida guardada está dañada (tamaño).")?;
        if u32_at(raw, 4)? != sr::crc32(body) { return Err("La partida guardada está dañada (CRC).".into()); }
        let dec = super::aes::ecb_decrypt(&sr::AES_KEY, body)?;
        let svlen = i32_at(&dec, 20)? as usize;
        let data = dec.get(24..24 + svlen - 4).ok_or("Formato de partida guardada no reconocido.")?.to_vec();
        let mut off = 24 + svlen - 4;
        let count = i32_at(&dec, off)?;
        off += 4;
        let mut table = Vec::new();
        for _ in 0..count {
            let (name, o) = sr::read_ue3(&dec, off)?;
            table.push((name, u32_at(&dec, o)?, u32_at(&dec, o + 4)?));
            off = o + 8;
        }
        if dec[off.min(dec.len())..].iter().any(|b| *b != 0) { return Err("Formato de partida guardada no reconocido.".into()); }
        Ok(Save { head: dec[..20].to_vec(), data, table })
    }

    pub fn encode(&self) -> R<Vec<u8>> {
        let mut out = self.head.clone();
        out.extend(((self.data.len() + 4) as i32).to_le_bytes());
        out.extend(&self.data);
        out.extend((self.table.len() as i32).to_le_bytes());
        for (name, pos, index) in &self.table {
            out.extend(sr::write_ue3(name));
            out.extend(pos.to_le_bytes());
            out.extend(index.to_le_bytes());
        }
        out.resize(out.len().div_ceil(16) * 16, 0);
        let body = super::aes::ecb_encrypt(&sr::AES_KEY, &out)?;
        let mut file = (body.len() as u32).to_le_bytes().to_vec();
        file.extend(sr::crc32(&body).to_le_bytes());
        file.extend(body);
        Ok(file)
    }

    /// (table index, start, end) of one object in the save data.
    pub fn span(&self, kind: &str) -> R<(usize, usize, usize)> {
        let rows: Vec<usize> = self.table.iter().enumerate().filter(|r| r.1 .0 == kind).map(|r| r.0).collect();
        let [i] = rows[..] else { return Err(format!("No se encuentra {kind} en la partida guardada.")) };
        let start = self.table[i].1 as usize;
        let end = if i + 1 < self.table.len() { self.table[i + 1].1 as usize - 4 } else { self.data.len() };
        Ok((i, start, end))
    }
}

struct Entry { name: String, size_at: usize, value_at: usize, children: Vec<Entry>, none_at: usize }

/// Tagged properties with their offsets; stops after 'None'. Returns (entries, none_start, end).
fn walk(buf: &[u8], mut off: usize) -> R<(Vec<Entry>, usize, usize)> {
    let mut entries = Vec::new();
    loop {
        let start = off;
        let (name, o) = sr::read_ue3(buf, off)?;
        if name == "None" { return Ok((entries, start, o)); }
        let (tag, o) = sr::read_ue3(buf, o)?;
        let size = i32_at(buf, o)?;
        let (size_at, value_at) = (o, o + 8);
        let mut entry = Entry { name, size_at, value_at, children: Vec::new(), none_at: 0 };
        if tag == "StructProperty" {
            let (_, inner) = sr::read_ue3(buf, value_at)?;
            let (children, none_at, end) = walk(buf, inner)?;
            entry.children = children;
            entry.none_at = none_at;
            off = end;
        } else {
            off = sr::parse_value(buf, value_at, &tag, size)?.1;
        }
        entries.push(entry);
    }
}

/// Numbers as Python's float()/int() accept them: numbers, booleans and numeric text.
fn number(value: &Value) -> Option<f64> {
    value.as_f64().or_else(|| value.as_bool().map(|b| b as i32 as f64)).or_else(|| value.as_str().and_then(|s| s.trim().parse().ok()))
}

fn encode_value(tag: &str, value: &Value) -> R<(Vec<u8>, i32)> {
    let number = || number(value).ok_or(format!("Valor no válido: {value}"));
    match tag {
        "FloatProperty" => Ok(((number()? as f32).to_le_bytes().to_vec(), 4)),
        "IntProperty" => Ok(((number()? as i32).to_le_bytes().to_vec(), 4)),
        "BoolProperty" => Ok((vec![truthy(value) as u8], 0)),
        _ => Err(format!("Tipo no editable: {tag}")),
    }
}

fn truthy(v: &Value) -> bool {
    match v { Value::Bool(b) => *b, Value::Number(n) => n.as_f64() != Some(0.), Value::String(s) => !s.is_empty(), Value::Null => false, _ => true }
}

pub fn get(save: &Save, key: &str) -> R<Value> {
    let (_, kind, path, tag) = *field(key).ok_or(format!("Ajuste no válido: {key}"))?;
    let (_, start, end) = save.span(kind)?;
    let blob = &save.data[start..end];
    let (mut entries, _, _) = walk(blob, 0)?;
    let mut found = None;
    for name in path {
        let Some(i) = entries.iter().position(|e| e.name == *name) else { return Ok(Value::Null) };
        let entry = entries.swap_remove(i);
        found = Some(entry.value_at);
        entries = entry.children;
    }
    Ok(sr::parse_value(blob, found.unwrap_or(0), tag, 4)?.0)
}

pub fn put(save: &mut Save, key: &str, value: &Value) -> R<()> {
    let (_, kind, path, tag) = *field(key).ok_or(format!("Ajuste no válido: {key}"))?;
    let (index, start, end) = save.span(kind)?;
    let mut blob = save.data[start..end].to_vec();
    let (raw, size) = encode_value(tag, value)?;
    let (mut entries, mut none_at, _) = walk(&blob, 0)?;
    let mut parents: Vec<usize> = Vec::new();
    for (depth, name) in path.iter().enumerate() {
        let Some(i) = entries.iter().position(|e| e.name == *name) else {
            if depth != path.len() - 1 { return Err(format!("Falta la estructura {name}")); }
            let mut insert = sr::write_ue3(name);
            insert.extend(sr::write_ue3(tag));
            insert.extend(size.to_le_bytes());
            insert.extend(0i32.to_le_bytes());
            insert.extend(&raw);
            blob.splice(none_at..none_at, insert.iter().copied());
            for at in &parents {
                let grown = i32_at(&blob, *at)? + insert.len() as i32;
                blob[*at..*at + 4].copy_from_slice(&grown.to_le_bytes());
            }
            break;
        };
        let entry = entries.swap_remove(i);
        if depth == path.len() - 1 { blob[entry.value_at..entry.value_at + raw.len()].copy_from_slice(&raw); break; }
        parents.push(entry.size_at);
        none_at = entry.none_at;
        entries = entry.children;
    }
    let delta = blob.len() as i64 - (end - start) as i64;
    save.data.splice(start..end, blob);
    if delta != 0 { for row in &mut save.table[index + 1..] { row.1 = (row.1 as i64 + delta) as u32; } }
    Ok(())
}

/// Keys the player bound to an action in the save (custom bindings only; the game's defaults are not stored).
pub fn bindings(data: &[u8], action: &str) -> Vec<String> {
    let marker = sr::write_ue3(action);
    let mut keys: Vec<String> = Vec::new();
    let mut from = 0;
    while let Some(found) = data[from..].windows(marker.len()).position(|w| w == marker.as_slice()) {
        let at = from + found;
        let key = (|| -> R<Option<String>> {
            let (name, off) = sr::read_ue3(data, at + marker.len())?;
            if name != "Key" { return Ok(None); }
            let (_, off) = sr::read_ue3(data, off)?;
            Ok(Some(sr::read_ue3(data, off + 8)?.0))
        })();
        if let Ok(Some(key)) = key { if !key.is_empty() && key != "None" && !keys.contains(&key) { keys.push(key); } }
        from = at + 1;
    }
    keys
}

fn ini_path(base: &Path) -> PathBuf { base.join("Config").join("TASystemSettings.ini") }

/// Byte bounds of the first [SystemSettings] section.
fn ini_section(text: &str) -> R<(usize, usize)> {
    let start = text.find("[SystemSettings]").ok_or("substring not found")?;
    Ok((start, text[start + 1..].find("\n[").map(|n| start + 1 + n + 1).unwrap_or(text.len())))
}

pub fn read(base: Option<&Path>) -> R<Value> {
    let base = base.map(Path::to_path_buf).unwrap_or_else(folder);
    let files = saves(&base);
    let Some(file) = files.first() else { return Err("No se encuentra la partida guardada de Rocket League.".into()) };
    let save = Save::new(&std::fs::read(file).map_err(|e| e.to_string())?)?;
    let present = |kind: &str| save.table.iter().any(|r| r.0 == kind);
    let mut values = Map::new();
    for (key, kind, _, _) in FIELDS { values.insert(key.into(), if present(kind) { get(&save, key)? } else { Value::Null }); }
    let mut skills = Vec::new();
    if present("TAGame.PlaylistSkillDataSave_TA") {
        let (_, start, end) = save.span("TAGame.PlaylistSkillDataSave_TA")?;
        let (props, _) = sr::parse_property_stream(&save.data[start..end], 0)?;
        if let Some(Value::Array(list)) = props.get("SkillData") { skills = list.iter().filter(|s| s.is_object()).cloned().collect(); }
    }
    let ini = ini_path(&base);
    if ini.exists() {
        let text = String::from_utf8_lossy(&std::fs::read(&ini).map_err(|e| e.to_string())?).into_owned();
        let (a, b) = ini_section(&text)?;
        for (key, name) in INI_FIELDS {
            let line = text[a..b].lines().find(|l| l.split('=').next().unwrap_or("").trim() == name);
            values.insert(key.into(), line.map(|l| l.split_once('=').map(|x| x.1).unwrap_or("").trim().into()).unwrap_or(Value::Null));
        }
    }
    Ok(json!({"values": values, "skills": skills, "save": file.display().to_string(), "scoreboard_keys": bindings(&save.data, "ToggleScoreboard")}))
}

fn stamp() -> String {
    let mut t = unsafe { std::mem::zeroed() };
    unsafe { windows_sys::Win32::System::SystemInformation::GetLocalTime(&mut t) };
    let t: windows_sys::Win32::Foundation::SYSTEMTIME = t;
    format!("{:04}{:02}{:02}-{:02}{:02}{:02}", t.wYear, t.wMonth, t.wDay, t.wHour, t.wMinute, t.wSecond)
}

pub fn write(root: &Path, changes: &Value, base: Option<&Path>) -> R<Value> {
    super::require_closed()?;
    let changes = changes.as_object().ok_or("Ajustes no válidos.")?;
    let mut unknown: Vec<&String> = changes.keys().filter(|k| field(k).is_none() && !INI_FIELDS.iter().any(|f| f.0 == k.as_str())).collect();
    unknown.sort();
    if !unknown.is_empty() { return Err(format!("Ajuste no válido: {}", unknown.iter().map(|s| s.as_str()).collect::<Vec<_>>().join(", "))); }
    let base = base.map(Path::to_path_buf).unwrap_or_else(folder);
    let files = saves(&base);
    if files.is_empty() { return Err("No se encuentra la partida guardada de Rocket League.".into()); }
    let backup = root.join("data").join("settings_backups").join(stamp());
    std::fs::create_dir_all(&backup).map_err(|e| e.to_string())?;
    let save_changes: Vec<(&String, &Value)> = changes.iter().filter(|(k, _)| field(k).is_some()).collect();
    for path in &files {
        let raw = std::fs::read(path).map_err(|e| e.to_string())?;
        std::fs::copy(path, backup.join(path.file_name().unwrap())).map_err(|e| e.to_string())?;
        let mut save = Save::new(&raw)?;
        for (key, value) in &save_changes { put(&mut save, key, value)?; }
        let out = save.encode()?;
        let check = Save::new(&out)?;
        for (key, value) in &save_changes {
            let got = get(&check, key)?;
            let bool_field = field(key).unwrap().3 == "BoolProperty";
            let ok = match (&got, bool_field) {
                (Value::Null, _) => false,
                (_, true) => truthy(&got) == truthy(value),
                _ => matches!((got.as_f64(), number(value)), (Some(g), Some(e)) if (g - e).abs() <= 1e-4),
            };
            if !ok { return Err(format!("No se pudo verificar {key}. No se ha cambiado nada.")); }
        }
        let tmp = path.with_extension("swapprism.tmp");
        std::fs::write(&tmp, &out).map_err(|e| e.to_string())?;
        super::require_closed()?;
        std::fs::rename(&tmp, path).map_err(|e| e.to_string())?;
    }
    let ini_changes: Vec<(&str, &Value)> = INI_FIELDS.iter().filter_map(|(k, name)| changes.get(*k).map(|v| (*name, v))).collect();
    let ini = ini_path(&base);
    if !ini_changes.is_empty() && ini.exists() {
        std::fs::copy(&ini, backup.join(ini.file_name().unwrap())).map_err(|e| e.to_string())?;
        let text = String::from_utf8_lossy(&std::fs::read(&ini).map_err(|e| e.to_string())?).into_owned();
        let (a, b) = ini_section(&text)?;
        let mut lines: Vec<String> = text[a..b].split('\n').map(str::to_owned).collect();
        for (name, value) in ini_changes {
            let value = match value {
                Value::Bool(b) => if *b { "True".into() } else { "False".into() },
                other => (other.as_f64().or_else(|| other.as_str().and_then(|s| s.trim().parse::<i64>().ok().map(|n| n as f64))).ok_or(format!("Valor no válido: {other}"))? as i64).to_string(),
            };
            match lines.iter().position(|l| l.split('=').next().unwrap_or("").trim() == name) {
                None => lines.insert(1, format!("{name}={value}")),
                Some(at) => { let cr = if lines[at].ends_with('\r') { "\r" } else { "" }; lines[at] = format!("{name}={value}{cr}"); }
            }
        }
        let tmp = ini.with_extension("swapprism.tmp");
        std::fs::write(&tmp, format!("{}{}{}", &text[..a], lines.join("\n"), &text[b..])).map_err(|e| e.to_string())?;
        std::fs::rename(&tmp, &ini).map_err(|e| e.to_string())?;
    }
    let mut result = read(Some(&base))?;
    result["backup"] = backup.display().to_string().into();
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn prop(name: &str, tag: &str, value: &[u8]) -> Vec<u8> {
        let mut out = sr::write_ue3(name);
        out.extend(sr::write_ue3(tag));
        out.extend((value.len() as i32).to_le_bytes());
        out.extend(0i32.to_le_bytes());
        out.extend(value);
        out
    }
    /// A save with one object holding MaxFPS, built from scratch.
    fn sample() -> Save {
        let mut object = 0xFFFF_FFFFu32.to_le_bytes().to_vec();
        object.extend(prop("MaxFPS", "IntProperty", &144i32.to_le_bytes()));
        object.extend(sr::write_ue3("None"));
        let mut data = 0xFFFF_FFFFu32.to_le_bytes().to_vec();
        data.extend(sr::write_ue3("None"));
        let position = data.len() as u32 + 4;
        data.extend(object);
        Save { head: vec![0; 20], data, table: vec![("TAGame.VideoSettingsSavePC_TA".into(), position, 0)] }
    }
    #[test]
    fn edits_survive_encode() {
        let mut save = sample();
        assert_eq!(get(&save, "video.max_fps").unwrap(), json!(144));
        put(&mut save, "video.max_fps", &json!(240)).unwrap();
        // Missing property is inserted before None.
        put(&mut save, "video.weather", &json!(true)).unwrap();
        let back = Save::new(&save.encode().unwrap()).unwrap();
        assert_eq!(get(&back, "video.max_fps").unwrap(), json!(240));
        assert_eq!(get(&back, "video.weather").unwrap(), json!(true));
    }
    #[test]
    fn reads_bound_key() {
        let mut data = b"junk".to_vec();
        data.extend(prop("Action", "NameProperty", &sr::write_ue3("ToggleScoreboard")));
        data.extend(sr::write_ue3("Key"));
        data.extend(sr::write_ue3("NameProperty"));
        data.extend([0u8; 8]);
        data.extend(sr::write_ue3("XboxTypeS_LeftThumbStick"));
        assert_eq!(bindings(&data, "ToggleScoreboard"), vec!["XboxTypeS_LeftThumbStick".to_string()]);
        assert!(bindings(b"nothing", "ToggleScoreboard").is_empty());
    }
    #[test]
    fn ini_section_bounds() {
        let text = "[Other]\nA=1\n[SystemSettings]\nResX=1920\n[Next]\n";
        let (a, b) = ini_section(text).unwrap();
        assert_eq!(&text[a..b], "[SystemSettings]\nResX=1920\n");
    }
}

/// Byte-for-byte parity with the Python engine on a copy of a real save:
/// SP_SAVE=<copy.save> SP_EXPECTED=<python output> cargo test -- --ignored real_save
#[cfg(test)]
mod parity {
    use super::*;
    #[test]
    #[ignore]
    fn real_save_put_matches_python() {
        let (Some(save), Some(expected)) = (std::env::var_os("SP_SAVE"), std::env::var_os("SP_EXPECTED")) else { return };
        let mut s = Save::new(&std::fs::read(save).unwrap()).unwrap();
        put(&mut s, "camera.fov", &json!(109.5)).unwrap();
        put(&mut s, "video.max_fps", &json!(237)).unwrap();
        put(&mut s, "camera.shake", &json!(false)).unwrap();
        assert!(s.encode().unwrap() == std::fs::read(expected).unwrap(), "el archivo no coincide con el de Python");
    }
}
