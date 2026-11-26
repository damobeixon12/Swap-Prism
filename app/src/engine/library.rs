//! Texture packs (AlphaConsole image/config data). Never extracts or executes archive code. Port of engine/texture_library.py.
use super::util::{err, R};
use serde_json::{json, Map, Value};
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};

#[derive(Clone, Debug)]
pub struct TexturePack {
    pub name: String,
    pub category: String,
    pub source: String,
    pub layers: Vec<(String, Vec<u8>)>,
    pub body_id: Value,
    pub skin_id: Value,
    pub digest: String,
}

impl TexturePack {
    pub fn layer(&self, role: &str) -> Option<&Vec<u8>> { self.layers.iter().find(|l| l.0 == role).map(|l| &l.1) }
}

/// A relative archive path without escapes (PurePosixPath normalisation).
pub fn safe_member(name: &str) -> R<String> {
    let text = name.replace('\\', "/");
    let parts: Vec<&str> = text.split('/').filter(|p| !p.is_empty() && *p != ".").collect();
    if text.starts_with('/') || parts.contains(&"..") || name.contains(':') { return Err("Ruta fuera del pack.".into()); }
    Ok(if parts.is_empty() { ".".into() } else { parts.join("/") })
}

fn check_image(raw: &[u8]) -> R<()> {
    let img = super::dxt::load(raw)?;
    if img.width() as u64 * img.height() as u64 > 4096 * 4096 { return Err("Imagen supera 4096².".into()); }
    Ok(())
}

/// Python's str() of a JSON value used in the digest (None for null, '' when the key is missing).
fn py_str(v: Option<&Value>) -> String {
    match v { None => String::new(), Some(Value::Null) => "None".into(), Some(Value::String(s)) => s.clone(), Some(Value::Bool(b)) => if *b { "True".into() } else { "False".into() }, Some(other) => other.to_string() }
}

pub fn read(source: &Path) -> R<Vec<TexturePack>> {
    let ext = source.extension().map(|e| e.to_string_lossy().to_lowercase()).unwrap_or_default();
    if ext == "png" || ext == "dds" {
        let raw = std::fs::read(source).map_err(err)?;
        check_image(&raw)?;
        let stem = source.file_stem().unwrap_or_default().to_string_lossy().into_owned();
        return Ok(vec![TexturePack { name: stem, category: "Boostmeter".into(), source: source.display().to_string(), layers: vec![("Background".into(), raw.clone())], body_id: Value::Null, skin_id: Value::Null, digest: format!("{:x}", Sha256::digest(&raw)) }]);
    }
    let (data, configs): (Vec<(String, Vec<u8>)>, Vec<String>) = match ext.as_str() {
        "zip" => {
            if std::fs::metadata(source).map_err(err)?.len() > 256 * 1024 * 1024 { return Err("ZIP demasiado grande.".into()); }
            let members = super::zip::members(source)?;
            if members.len() > 2000 || members.iter().map(|m| m.size).sum::<u64>() > 512 * 1024 * 1024 { return Err("ZIP supera los límites.".into()); }
            let mut data = Vec::new();
            for m in members.iter().filter(|m| !m.is_dir()) {
                let name = safe_member(&m.name)?;
                let lower = name.to_lowercase();
                if [".json", ".png", ".jpg", ".jpeg", ".dds"].iter().any(|e| lower.ends_with(e)) {
                    let bytes = super::zip::read(source, m)?;
                    match data.iter_mut().find(|d: &&mut (String, Vec<u8>)| d.0 == name) { Some(d) => d.1 = bytes, None => data.push((name, bytes)) }
                }
            }
            let configs = data.iter().filter(|d| d.0.to_lowercase().ends_with(".json")).map(|d| d.0.clone()).collect();
            (data, configs)
        }
        "json" => {
            let name = source.file_name().unwrap_or_default().to_string_lossy().into_owned();
            (vec![(name.clone(), std::fs::read(source).map_err(err)?)], vec![name])
        }
        _ => return Err("Selecciona ZIP, JSON, PNG o DDS.".into()),
    };
    let mut result = Vec::new();
    for config in configs {
        let raw = &data.iter().find(|d| d.0 == config).unwrap().1;
        if raw.len() > 1024 * 1024 { return Err("JSON demasiado grande.".into()); }
        let text = String::from_utf8(raw.clone()).map_err(err)?;
        let obj: Value = serde_json::from_str(text.trim_start_matches('\u{feff}')).map_err(err)?;
        let Some(obj) = obj.as_object() else { continue };
        for (name, record) in obj {
            let Some(record) = record.as_object() else { continue };
            let category = if record.contains_key("Background") && record.contains_key("Fill") { "Boostmeter" } else if record.contains_key("Params") { "Pelota" } else if record.contains_key("Body") { "Decal" } else { continue };
            let empty = Map::new();
            let params = if category == "Boostmeter" { record } else { record.get("Params").or(record.get("Body")).and_then(Value::as_object).unwrap_or(&empty) };
            let mut layers: Vec<(String, Vec<u8>)> = Vec::new();
            for (role, filename) in params {
                let Some(filename) = filename.as_str().filter(|f| !f.trim().is_empty()) else { continue };
                // AlphaConsole packs sometimes use an extension-only placeholder for an intentionally absent layer.
                if [".png", ".dds", ".jpg", ".jpeg"].contains(&filename.trim().to_lowercase().as_str()) { continue; }
                let relative = safe_member(filename)?;
                let parent = config.rsplit_once('/').map(|p| p.0).unwrap_or(".");
                let key = safe_member(&format!("{parent}/{relative}"))?;
                let bytes = if ext == "zip" {
                    data.iter().find(|d| d.0.to_lowercase() == key.to_lowercase()).map(|d| d.1.clone()).ok_or(format!("Falta imagen: {relative}"))?
                } else {
                    let folder = super::util::resolve(source.parent().unwrap_or(Path::new(".")));
                    let file = super::util::resolve(&folder.join(&relative));
                    if !file.starts_with(&folder) || file == folder { return Err("Imagen fuera del pack.".into()); }
                    if std::fs::metadata(&file).map_err(err)?.len() > 64 * 1024 * 1024 { return Err("Imagen demasiado grande.".into()); }
                    std::fs::read(&file).map_err(err)?
                };
                if bytes.len() > 64 * 1024 * 1024 { return Err("Imagen demasiado grande.".into()); }
                check_image(&bytes)?;
                match layers.iter_mut().find(|l| l.0 == *role) { Some(l) => l.1 = bytes, None => layers.push((role.clone(), bytes)) }
            }
            if layers.is_empty() { continue; }
            // Equal directory/ZIP copies produce one library entry.
            let mut sorted: Vec<&(String, Vec<u8>)> = layers.iter().collect();
            sorted.sort_by(|a, b| a.0.cmp(&b.0));
            let mut hash = Sha256::new();
            hash.update(category.as_bytes());
            hash.update(py_str(record.get("SkinID")).as_bytes());
            for (k, v) in sorted { hash.update(k.as_bytes()); hash.update(Sha256::digest(v)); }
            result.push(TexturePack { name: name.clone(), category: category.into(), source: source.display().to_string(), layers,
                body_id: record.get("BodyID").cloned().unwrap_or(Value::Null), skin_id: record.get("SkinID").cloned().unwrap_or(Value::Null), digest: format!("{:x}", hash.finalize()) });
        }
    }
    Ok(result)
}

fn walk(dir: &Path, ext: &str, out: &mut Vec<PathBuf>) {
    for e in std::fs::read_dir(dir).into_iter().flatten().flatten() {
        let p = e.path();
        if p.is_dir() { walk(&p, ext, out); } else if p.extension().is_some_and(|x| x.to_string_lossy().to_lowercase() == ext) { out.push(p); }
    }
}

pub fn scan(root: &Path) -> (Vec<TexturePack>, Vec<String>) {
    let sources = if root.is_file() { vec![root.to_path_buf()] } else {
        let (mut json, mut zip) = (Vec::new(), Vec::new());
        walk(root, "json", &mut json);
        walk(root, "zip", &mut zip);
        json.sort_by_key(|p| p.display().to_string().to_lowercase());
        zip.sort_by_key(|p| p.display().to_string().to_lowercase());
        json.into_iter().chain(zip).collect()
    };
    let (mut packs, mut errors, mut seen) = (Vec::new(), Vec::new(), std::collections::HashSet::new());
    for source in sources {
        match read(&source) {
            Ok(found) => for pack in found { if seen.insert(pack.digest.clone()) { packs.push(pack); } },
            Err(e) => errors.push(format!("{}: {e}", source.file_name().unwrap_or_default().to_string_lossy())),
        }
    }
    (packs, errors)
}

pub fn save_library(folder: &Path, packs: &[TexturePack]) -> R<()> {
    std::fs::create_dir_all(folder).map_err(err)?;
    for pack in packs {
        let mut layers = Map::new();
        for (role, raw) in &pack.layers {
            let name = format!("{:x}.image", Sha256::digest(raw));
            let file = folder.join(&name);
            if !file.exists() { std::fs::write(&file, raw).map_err(err)?; }
            layers.insert(role.clone(), name.into());
        }
        let record = json!({"name": pack.name, "category": pack.category, "source": pack.source, "body_id": pack.body_id, "skin_id": pack.skin_id, "digest": pack.digest, "layers": layers});
        std::fs::write(folder.join(format!("{}.json", pack.digest)), super::util::dumps_compact(&record)).map_err(err)?;
    }
    Ok(())
}

pub fn load_library(folder: &Path) -> Vec<TexturePack> {
    let mut result = Vec::new();
    let Ok(entries) = std::fs::read_dir(folder) else { return result };
    let folder = super::util::resolve(folder);
    for config in entries.flatten().map(|e| e.path()).filter(|p| p.extension().is_some_and(|x| x == "json")) {
        let pack = (|| -> R<TexturePack> {
            let obj: Map<String, Value> = serde_json::from_slice(&std::fs::read(&config).map_err(err)?).map_err(err)?;
            let allowed = ["name", "category", "source", "body_id", "skin_id", "digest", "layers"];
            if obj.keys().any(|k| !allowed.contains(&k.as_str())) || !obj.contains_key("name") || !obj.contains_key("category") || !obj.contains_key("source") { return Err("Pack no válido.".into()); }
            let mut layers = Vec::new();
            for (role, name) in obj["layers"].as_object().ok_or("Pack no válido.")? {
                let file = super::util::resolve(&folder.join(safe_member(name.as_str().ok_or("Pack no válido.")?)?));
                if file.parent() != Some(folder.as_path()) { return Err("Ruta inválida.".into()); }
                if std::fs::metadata(&file).map_err(err)?.len() > 64 * 1024 * 1024 { return Err("Imagen demasiado grande.".into()); }
                layers.push((role.clone(), std::fs::read(&file).map_err(err)?));
            }
            let mut category = obj["category"].as_str().unwrap_or("").to_owned();
            if category == "Boost" {
                category = "Boostmeter".into();
                if let Some(d) = layers.iter().find(|l| l.0 == "Diffuse").map(|l| l.1.clone()) { layers = vec![("Background".into(), d)]; }
            }
            Ok(TexturePack { name: obj["name"].as_str().unwrap_or("").into(), category, source: obj["source"].as_str().unwrap_or("").into(), layers,
                body_id: obj.get("body_id").cloned().unwrap_or(Value::Null), skin_id: obj.get("skin_id").cloned().unwrap_or(Value::Null), digest: obj.get("digest").and_then(Value::as_str).unwrap_or("").into() })
        })();
        if let Ok(pack) = pack { result.push(pack); }
    }
    result
}

/// The picture that represents a pack.
fn image_for(pack: &TexturePack) -> R<image::RgbaImage> {
    let raw = ["1_Diffuse_Skin", "Diffuse", "Background", "Fill"].iter().find_map(|r| pack.layer(r)).or(pack.layers.first().map(|l| &l.1)).ok_or("Pack vacío.")?;
    super::dxt::load(raw)
}

/// The library for the interface, with a cached preview per pack.
pub fn packs(root: &Path) -> R<Vec<Value>> {
    let library = load_library(&root.join("data").join("texture_library"));
    let preview = root.join("data").join("texture_previews");
    std::fs::create_dir_all(&preview).map_err(err)?;
    let mut result = Vec::new();
    for p in library {
        let image = preview.join(format!("{}.png", p.digest));
        if !image.exists() {
            let picture = image_for(&p)?;
            let (w, h) = (picture.width() as f64, picture.height() as f64);
            let scale = (256. / w).min(192. / h).min(1.);
            let small = super::dxt::resize(&picture, ((w * scale).round() as u32).max(1), ((h * scale).round() as u32).max(1));
            std::fs::write(&image, super::dxt::png(&small)?).map_err(err)?;
        }
        result.push(json!({"name": p.name, "category": p.category, "body_id": p.body_id, "skin_id": p.skin_id, "digest": p.digest, "preview": image.display().to_string()}));
    }
    Ok(result)
}
