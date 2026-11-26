//! Saved inventory and loadouts, read-only. Port of engine/player_inventory.py, save_reader.parse_savedata and
//! swapprism_engine.player_name. No network access and no save-file writes.
use super::save::{self as sr, i32_at, u32_at, R};
use super::util::err;
use serde_json::{json, Map, Value};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

const PAINTS: [&str; 15] = ["", "Crimson", "Lime", "Black", "Sky Blue", "Cobalt", "Burnt Sienna", "Forest Green", "Purple", "Pink", "Orange", "Grey", "Titanium White", "Saffron", "Gold"];
fn slot_index(i: usize) -> Option<&'static str> {
    Some(match i { 0 => "Body", 1 => "Decal", 2 => "Wheels", 3 => "Rocket Boost", 4 => "Topper", 5 => "Antenna", 13 => "Engine Audio", 14 => "Trail", 15 => "Goal Explosion", _ => return None })
}

/// The user's Documents folder as Windows reports it (it may be redirected, e.g. to OneDrive).
fn documents() -> PathBuf {
    let mut buf = [0u16; 32768];
    let ok = unsafe { windows_sys::Win32::UI::Shell::SHGetFolderPathW(std::ptr::null_mut(), 5, std::ptr::null_mut(), 0, buf.as_mut_ptr()) } == 0;
    if ok { let n = buf.iter().position(|c| *c == 0).unwrap_or(0); return PathBuf::from(String::from_utf16_lossy(&buf[..n])); }
    PathBuf::from(std::env::var_os("USERPROFILE").unwrap_or_default()).join("Documents")
}

pub fn save_folders() -> Vec<PathBuf> {
    let base = documents().join("My Games").join("Rocket League").join("TAGame");
    vec![base.join("SaveDataEpic"), base.join("SaveData")]
}

fn walk(dir: &Path, out: &mut Vec<(std::time::SystemTime, PathBuf)>) {
    for e in std::fs::read_dir(dir).into_iter().flatten().flatten() {
        let p = e.path();
        if p.is_dir() { walk(&p, out); } else if p.extension().is_some_and(|x| x == "save") { out.push((e.metadata().and_then(|m| m.modified()).unwrap_or(std::time::UNIX_EPOCH), p)); }
    }
}

pub fn save_candidates() -> Vec<PathBuf> {
    let mut files = Vec::new();
    for folder in save_folders() { if folder.is_dir() { walk(&folder, &mut files); } }
    files.sort_by(|a, b| b.0.cmp(&a.0));
    files.into_iter().map(|f| f.1).collect()
}

/// Root properties and every object of the save (objects that fail to parse are marked, like the Python reader).
fn parse_savedata(raw: &[u8]) -> R<(Map<String, Value>, Vec<(u32, Value)>)> {
    let size = u32_at(raw, 0)? as usize;
    let dec = super::aes::ecb_decrypt(&sr::AES_KEY, raw.get(8..8 + size).ok_or("Guardado incompleto.")?)?;
    let svlen = i32_at(&dec, 20)? as usize;
    let svdata = dec.get(24..24 + svlen - 4).ok_or("Guardado incompleto.")?;
    let mut off = 24 + svlen - 4;
    let ntypes = i32_at(&dec, off)?;
    off += 4;
    let mut types = Vec::new();
    for _ in 0..ntypes {
        let (name, o) = sr::read_ue3(&dec, off)?;
        types.push((name, u32_at(&dec, o)? as usize, u32_at(&dec, o + 4)?));
        off = o + 8;
    }
    let (props, _) = sr::parse_property_stream(svdata, 4)?;
    let mut objects = Vec::new();
    for (i, (kind, position, index)) in types.iter().enumerate() {
        let obj_pos = position.wrapping_sub(4);
        if *position < 4 || obj_pos >= svdata.len() { objects.push((*index, json!({"__type": kind, "__error": "out of range"}))); continue; }
        let end = if i + 1 < types.len() { types[i + 1].1.saturating_sub(4) } else { svdata.len() };
        let bytes = svdata.get(obj_pos + 4..end.max(obj_pos + 4)).unwrap_or(&[]);
        let obj = match sr::parse_property_stream(bytes, 0) {
            Ok((mut p, _)) => { p.insert("__type".into(), kind.clone().into()); Value::Object(p) }
            Err(e) => json!({"__type": kind, "__parse_error": e}),
        };
        objects.push((*index, obj));
    }
    Ok((props, objects))
}

fn instance_key(value: &Value) -> Value {
    let Some(v) = value.as_object() else { return json!([0, 0]) };
    // 64-bit unsigned ids: keep the number as read (int(value) in Python).
    let n = |k: &str| v.get(k).filter(|x| x.is_number()).cloned().or_else(|| v.get(k).and_then(Value::as_str).and_then(|s| s.parse::<u64>().ok()).map(Value::from)).unwrap_or(json!(0));
    json!([n("UpperBits"), n("LowerBits")])
}

fn is_type(obj: &Value, t: &str) -> bool { obj["__type"] == t }

/// The inventory/presets snapshot the interface shows (Snapshot dataclass as a dict).
pub fn load_snapshot(database: &[Value]) -> R<Value> {
    let candidates = save_candidates();
    let source = candidates.first().ok_or("No se han encontrado guardados locales de Rocket League.")?.clone();
    let before = std::fs::metadata(&source).map_err(err)?;
    if before.len() > 64 * 1024 * 1024 { return Err("El guardado supera el tamaño admitido por el lector.".into()); }
    let raw = std::fs::read(&source).map_err(err)?;
    if raw.len() < 8 { return Err("Guardado incompleto.".into()); }
    let (size, expected) = (u32_at(&raw, 0)? as usize, u32_at(&raw, 4)?);
    if size < 16 || size % 16 != 0 || size > raw.len() - 8 || sr::crc32(&raw[8..8 + size]) != expected { return Err("Guardado incompleto o CRC incorrecto; espera al próximo guardado.".into()); }
    let (props, objects) = parse_savedata(&raw)?;
    let after = std::fs::metadata(&source).map_err(err)?;
    if before.modified().ok() != after.modified().ok() || before.len() != after.len() || std::fs::read(&source).map_err(err)? != raw {
        return Err("Rocket League está guardando. Se reintentará en la siguiente actualización.".into());
    }
    let modified_ns = after.modified().map_err(err)?.duration_since(std::time::UNIX_EPOCH).map_err(err)?.as_nanos() as u64;
    extract(&props, objects, database, &source, modified_ns)
}

fn extract(props: &Map<String, Value>, list: Vec<(u32, Value)>, database: &[Value], source: &Path, modified_ns: u64) -> R<Value> {
    let mut objects: Vec<(i64, Value)> = Vec::new();
    for (k, v) in list { match objects.iter_mut().find(|o| o.0 == k as i64) { Some(o) => o.1 = v, None => objects.push((k as i64, v)) } }
    let get = |r: &Value| -> Option<&Value> { let r = r.as_i64()?; objects.iter().find(|o| o.0 == r).map(|o| &o.1) };
    let products: HashMap<i64, &Value> = database.iter().filter_map(|r| Some((r["ID"].as_i64()?, r))).collect();
    let describe = |product_id: i64, instance: &Value, attribute_refs: &[Value], source_kind: &str| -> Value {
        let attrs: Vec<&Value> = attribute_refs.iter().filter(|r| r.is_i64() || r.is_u64()).map(|r| get(r).unwrap_or(&Value::Null)).collect();
        let paint = attrs.iter().find(|a| is_type(a, "TAGame.ProductAttribute_Painted_TA")).map(|a| a.get("PaintID").cloned().unwrap_or(json!(0))).unwrap_or(json!(0));
        let certified = attrs.iter().find(|a| is_type(a, "TAGame.ProductAttribute_Certified_TA")).map(|a| a.get("StatId").cloned().unwrap_or(Value::Null)).unwrap_or(Value::Null);
        let blueprint = attrs.iter().any(|a| is_type(a, "TAGame.ProductAttribute_Blueprint_TA"));
        let mut row = products.get(&product_id).and_then(|r| r.as_object().cloned()).unwrap_or_default();
        let product = row.get("Product").cloned().unwrap_or(format!("Producto #{product_id}").into());
        let slot = row.get("Slot").cloned().unwrap_or("Sin identificar".into());
        let paint_name = paint.as_i64().filter(|p| *p >= 0).and_then(|p| PAINTS.get(p as usize)).map(|s| s.to_string()).unwrap_or(format!("Paint #{paint}"));
        for (k, v) in [("ID", json!(product_id)), ("Product", product), ("Slot", slot), ("paint", paint_name.into()), ("paint_id", paint), ("certified", certified),
            ("blueprint", blueprint.into()), ("instance", instance_key(instance)), ("source_kind", source_kind.into())] { row.insert(k.into(), v); }
        if let Some(title) = attrs.iter().find(|a| is_type(a, "TAGame.ProductAttribute_TitleID_TA")).and_then(|a| a.get("TitleId")).filter(|t| truthy(t)) { row.insert("title_id".into(), title.clone()); }
        Value::Object(row)
    };
    let mut inventory: Vec<Value> = Vec::new();
    let mut by_instance: Vec<(Value, Value)> = Vec::new();
    for r in props.get("OnlineProducts").and_then(Value::as_array).cloned().unwrap_or_default() {
        let obj = get(&r).filter(|o| is_type(o, "TAGame.OnlineProduct_TA") && o.get("__parse_error").is_none()).ok_or("No se pudo leer íntegramente el inventario del guardado.")?;
        let id = obj["ProductID"].as_i64().unwrap_or(0);
        let item = describe(id, obj.get("InstanceID").unwrap_or(&Value::Null), obj["Attributes"].as_array().map(Vec::as_slice).unwrap_or(&[]), "online");
        match by_instance.iter_mut().find(|b| b.0 == item["instance"]) { Some(b) => b.1 = item.clone(), None => by_instance.push((item["instance"].clone(), item.clone())) }
        inventory.push(item);
    }
    let profile = objects.iter().map(|o| &o.1).find(|o| is_type(o, "TAGame.ProfileLoadoutSave_TA")).filter(|p| p.get("__parse_error").is_none()).ok_or("No se ha podido identificar el perfil de equipamiento.")?;
    let mut presets: Vec<Value> = Vec::new();
    let mut active_index = -1i64;
    for r in profile["LoadoutSets"].as_array().cloned().unwrap_or_default() {
        let preset = get(&r).filter(|p| is_type(p, "TAGame.LoadoutSet_TA") && p.get("__parse_error").is_none()).ok_or("Hay un preset ilegible en el guardado.")?;
        let mut teams = Vec::new();
        for lr in preset["Loadouts"].as_array().cloned().unwrap_or_default() {
            let loadout = get(&lr).filter(|l| is_type(l, "TAGame.Loadout_TA") && l.get("__parse_error").is_none()).ok_or("No se ha podido leer el equipamiento de un equipo.")?;
            let instances = loadout["OnlineProducts128"].as_array().cloned().unwrap_or_default();
            let mut equipped = Vec::new();
            for (slot, product) in loadout["Products"].as_array().cloned().unwrap_or_default().iter().enumerate() {
                let Some(product_id) = product.as_i64() else { continue };
                if product_id <= 0 { continue; }
                let instance = instances.get(slot).cloned().unwrap_or(json!({}));
                let found = by_instance.iter().find(|b| b.0 == instance_key(&instance)).map(|b| b.1.clone());
                let mut item = match found { Some(i) if i["ID"] == product_id => i, _ => describe(product_id, &instance, &[], "equipped-local") };
                if item["Slot"] == "Sin identificar" { if let Some(s) = slot_index(slot) { item["Slot"] = s.into(); } }
                item["slot_index"] = slot.into();
                equipped.push(item);
            }
            teams.push(json!({"items": equipped, "paint": loadout.get("TeamPaint").cloned().unwrap_or(json!({}))}));
        }
        if Some(&r) == profile.get("EquippedLoadoutSet") { active_index = presets.len() as i64; }
        presets.push(json!({"name": preset.get("LoadoutSetName").cloned().unwrap_or("Preset".into()), "teams": teams}));
    }
    let mut extras = Vec::new();
    if let Some(title) = profile.get("PlayerTitle").filter(|t| truthy(t)) {
        extras.push(json!({"ID": 0, "Product": title, "Slot": "Player Title", "paint": "", "instance": [0, 0], "blueprint": false}));
    }
    if let Some(item) = by_instance.iter().find(|b| b.0 == instance_key(profile.get("MusicStingerInstanceID").unwrap_or(&Value::Null))).map(|b| b.1.clone()) { extras.push(item); }
    for typ in ["TAGame.PlayerBannerSave_TA", "TAGame.PlayerAvatarBorderSave_TA"] {
        let obj = objects.iter().map(|o| &o.1).find(|o| is_type(o, typ)).cloned().unwrap_or(json!({}));
        if obj.get("ProductID").is_some_and(truthy) {
            let found = by_instance.iter().find(|b| b.0 == instance_key(obj.get("InstanceID").unwrap_or(&Value::Null))).map(|b| b.1.clone());
            extras.push(found.unwrap_or_else(|| describe(obj["ProductID"].as_i64().unwrap_or(0), obj.get("InstanceID").unwrap_or(&Value::Null), &[], "equipped-local")));
        }
    }
    // Only entries present in the save; never a full default inventory.
    let mut known: Vec<Value> = inventory.iter().filter(|i| i["blueprint"] != true).map(|i| i["ID"].clone()).collect();
    for preset in &presets {
        for team in preset["teams"].as_array().unwrap() {
            for item in team["items"].as_array().unwrap() {
                if !known.contains(&item["ID"]) { inventory.push(item.clone()); known.push(item["ID"].clone()); }
            }
        }
    }
    let unknown = inventory.iter().filter(|i| !i["ID"].as_i64().is_some_and(|id| products.contains_key(&id))).count();
    let active = if active_index >= 0 { presets.get(active_index as usize).cloned().unwrap_or(Value::Null) } else { Value::Null };
    Ok(json!({"inventory": inventory, "presets": presets, "active_index": active_index, "account_equipped": extras, "source": source.display().to_string(),
        "modified_ns": modified_ns, "crc_match": true, "unknown_products": unknown, "active": active}))
}

fn truthy(v: &Value) -> bool {
    match v { Value::Null => false, Value::Bool(b) => *b, Value::Number(n) => n.as_f64() != Some(0.), Value::String(s) => !s.is_empty(), Value::Array(a) => !a.is_empty(), Value::Object(o) => !o.is_empty() }
}

/// The primary login of this save's account from the game's own log, never a team name.
pub fn player_name(source: &Path) -> String {
    let Some(tagame) = source.ancestors().find(|p| p.file_name().is_some_and(|n| n.to_string_lossy().to_lowercase() == "tagame")) else { return String::new() };
    let log = tagame.join("Logs").join("Launch.log");
    let Ok(mut file) = std::fs::File::open(&log) else { return String::new() };
    let len = file.metadata().map(|m| m.len()).unwrap_or(0);
    let _ = std::io::Seek::seek(&mut file, std::io::SeekFrom::Start(len.saturating_sub(2 * 1024 * 1024)));
    let mut raw = Vec::new();
    if std::io::Read::read_to_end(&mut file, &mut raw).is_err() { return String::new(); }
    let text = String::from_utf8_lossy(&raw);
    let stem = source.file_stem().unwrap_or_default().to_string_lossy().to_lowercase();
    let mut name = String::new();
    for line in text.lines() {
        let Some(found) = parse_login(line) else { continue };
        let identity: Vec<&str> = found.1.split('|').collect();
        if identity.len() < 2 || identity[1].is_empty() || !stem.contains(&identity[1].to_lowercase()) { continue; }
        let candidate = found.0.trim();
        if !candidate.is_empty() && candidate.chars().count() <= 96 && !candidate.chars().any(|c| (c as u32) < 32) { name = candidate.to_owned(); }
    }
    name
}

/// `HandleLocalPlayerLoginStatusChanged PlayerName=(.*?) PlayerID=(\S+).*LoginStatus=LS_LoggedIn.*IsPrimary=True`
fn parse_login(line: &str) -> Option<(String, String)> {
    let marker = "HandleLocalPlayerLoginStatusChanged PlayerName=";
    let start = line.find(marker)? + marker.len();
    let rest = &line[start..];
    let mut from = 0;
    while let Some(i) = rest[from..].find(" PlayerID=") {
        let at = from + i;
        let after = &rest[at + " PlayerID=".len()..];
        let id: String = after.chars().take_while(|c| !c.is_whitespace()).collect();
        let tail = &after[id.len()..];
        if !id.is_empty() {
            if let Some(l) = tail.find("LoginStatus=LS_LoggedIn") { if tail[l..].contains("IsPrimary=True") { return Some((rest[..at].to_owned(), id)); } }
        }
        from = at + 1;
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn login_line() {
        let line = "[0010.00] Log: HandleLocalPlayerLoginStatusChanged PlayerName=damobe. PlayerID=Epic|abc|0 LoginStatus=LS_LoggedIn IsPrimary=True";
        assert_eq!(parse_login(line), Some(("damobe.".into(), "Epic|abc|0".into())));
        assert_eq!(parse_login("HandleLocalPlayerLoginStatusChanged PlayerName=x PlayerID=y LoginStatus=LS_NotLoggedIn IsPrimary=True"), None);
    }
}
