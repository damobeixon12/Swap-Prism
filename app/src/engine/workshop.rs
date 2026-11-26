//! Public map catalog and local library. Only Utopia Retro may be replaced. Port of engine/workshop.py
//! (catalog, download, import, remove; applying a map is part of the change queue).
use super::util::{self, err, R};
use serde_json::{json, Map, Value};
use sha2::{Digest, Sha256};
use std::io::Read;
use std::path::{Path, PathBuf};

pub const TARGET: &str = "Labs_Utopia_P.upk";
const CATALOG: &str = "https://lethamyr.com/api/v1/maps";
const LIMIT: u64 = 768 * 1024 * 1024;

pub fn folder(root: &Path) -> R<PathBuf> {
    let p = root.join("data").join("workshop");
    std::fs::create_dir_all(&p).map_err(err)?;
    Ok(p)
}

/// JSON with 2-space indent, raw non-ASCII and CRLF, written through a temporary file (as the Python engine wrote it).
pub fn write(path: &Path, data: &Value) -> R<()> {
    let tmp = path.with_extension("tmp");
    // Python's text mode writes Windows line breaks; JSON strings never hold a raw newline.
    std::fs::write(&tmp, serde_json::to_string_pretty(data).map_err(err)?.replace('\n', "\r\n")).map_err(err)?;
    std::fs::rename(&tmp, path).map_err(err)
}

pub fn read(path: &Path, default: Value) -> R<Value> {
    if !path.exists() { return Ok(default); }
    serde_json::from_slice(&std::fs::read(path).map_err(err)?).map_err(err)
}

/// Only public HTTPS links to the known map sources.
fn https(url: &str) -> R<url::Url> {
    let parsed = url::Url::parse(url).map_err(|_| "Solo enlaces HTTPS públicos.".to_string())?;
    if parsed.scheme() != "https" || !parsed.username().is_empty() || parsed.password().is_some() { return Err("Solo enlaces HTTPS públicos.".into()); }
    let host = parsed.host_str().unwrap_or("").to_lowercase();
    let allowed = ["lethamyr.com", "www.lethamyr.com", "drive.google.com", "drive.usercontent.google.com", "docs.google.com"];
    if !allowed.contains(&host.as_str()) && !host.ends_with(".googleusercontent.com") {
        return Err("Fuente de descarga no admitida. Importa el mapa descargado desde su autor.".into());
    }
    Ok(parsed)
}

fn agent() -> R<ureq::Agent> {
    let tls = native_tls::TlsConnector::new().map_err(err)?;
    Ok(ureq::AgentBuilder::new().redirects(0).timeout_connect(std::time::Duration::from_secs(30)).timeout_read(std::time::Duration::from_secs(30))
        .user_agent("SwapPrism/0.23 (map catalog)").tls_connector(std::sync::Arc::new(tls)).build())
}

/// GET following HTTPS redirects; the final address must be an allowed source.
fn open(url: &str) -> R<ureq::Response> {
    let agent = agent()?;
    let mut next = https(url)?;
    for _ in 0..10 {
        let response = match agent.get(next.as_str()).set("Accept", "*/*").call() {
            Ok(r) => r,
            Err(ureq::Error::Status(code, _)) => return Err(format!("Este mapa ya no está disponible en su enlace (error {code}). Prueba otro o impórtalo desde la web del autor.")),
            Err(_) => return Err("No se pudo descargar: sin conexión o el servidor no responde.".into()),
        };
        if (300..400).contains(&response.status()) {
            let location = response.header("Location").ok_or("No se pudo descargar: redirección sin destino.")?;
            next = next.join(location).map_err(|_| "Solo enlaces HTTPS públicos.".to_string())?;
            if next.scheme() != "https" { return Err("Solo enlaces HTTPS públicos.".into()); }
            continue;
        }
        https(next.as_str())?;
        return Ok(response);
    }
    Err("No se pudo descargar: demasiadas redirecciones.".into())
}

/// Small downloads (catalog pages, images) into memory.
fn fetch(url: &str, maximum: u64) -> R<Vec<u8>> {
    let mut raw = Vec::new();
    open(url)?.into_reader().take(maximum + 1).read_to_end(&mut raw).map_err(|_| "No se pudo descargar: sin conexión o el servidor no responde.".to_string())?;
    if raw.len() as u64 > maximum { return Err("Archivo demasiado grande.".into()); }
    Ok(raw)
}

/// Large download straight to a file, reporting progress in MB.
fn fetch_to(url: &str, dest: &Path, label: &str, progress: &dyn Fn(Value)) -> R<()> {
    let response = open(url)?;
    let total: u64 = response.header("Content-Length").and_then(|v| v.parse().ok()).unwrap_or(0);
    let mut reader = response.into_reader();
    let mut out = std::io::BufWriter::new(std::fs::File::create(dest).map_err(err)?);
    let mut buf = vec![0u8; 1 << 20];
    let mut done = 0u64;
    loop {
        let n = reader.read(&mut buf).map_err(|_| "No se pudo descargar: sin conexión o el servidor no responde.".to_string())?;
        if n == 0 { break; }
        done += n as u64;
        if done > LIMIT { return Err("Archivo demasiado grande.".into()); }
        std::io::Write::write_all(&mut out, &buf[..n]).map_err(err)?;
        progress(json!({"phase": "download", "current": done >> 20, "total": total >> 20, "message": label}));
    }
    std::io::Write::flush(&mut out).map_err(err)
}

pub fn snapshot(root: &Path) -> R<Value> {
    let base = folder(root)?;
    let mut data = read(&base.join("catalog.json"), json!({"maps": []}))?;
    let library = read(&base.join("library.json"), json!({}))?;
    let library = library.as_object().cloned().unwrap_or_default();
    let maps = data["maps"].as_array_mut().ok_or("catalog.json no válido.")?;
    for item in maps.iter_mut() {
        let key = item["key"].as_str().unwrap_or("").to_owned();
        if let Some(entry) = library.get(&key).filter(|e| !is_empty(e)) { item["local"] = entry.clone(); }
        let thumb = base.join("images").join(format!("{key}.png"));
        if thumb.is_file() { item["preview"] = thumb.display().to_string().into(); }
    }
    let known: Vec<String> = maps.iter().filter_map(|m| m["key"].as_str().map(str::to_owned)).collect();
    for (k, v) in &library {
        if known.contains(k) { continue; }
        let mut entry = v.as_object().cloned().unwrap_or_default();
        entry.insert("key".into(), k.clone().into());
        entry.insert("local".into(), v.clone());
        entry.insert("source".into(), "Importado".into());
        maps.push(Value::Object(entry));
    }
    Ok(data)
}

/// Python truthiness for the library entry check (`if entry:`).
fn is_empty(v: &Value) -> bool { matches!(v, Value::Null) || v.as_object().is_some_and(Map::is_empty) }

pub fn refresh(root: &Path) -> R<Value> {
    let mut maps: Vec<Value> = Vec::new();
    let mut url = CATALOG.to_owned();
    let mut seen: Vec<String> = Vec::new();
    let mut finished = false;
    for _ in 0..30 {
        if seen.contains(&url) { return Err("Paginación repetida del catálogo.".into()); }
        seen.push(url.clone());
        let data: Value = serde_json::from_slice(&fetch(&url, 4 * 1024 * 1024)?).map_err(err)?;
        for item in data["data"].as_array().ok_or("Catálogo no válido.")? {
            let name = item["name"].as_str().ok_or("Catálogo no válido.")?;
            let download = item["download_url"].as_str().unwrap_or("");
            let key = format!("{:x}", Sha256::digest(format!("{name}\n{download}").as_bytes()));
            maps.push(json!({"key": key, "name": name, "description": item.get("description_short").cloned().unwrap_or("".into()), "url": download,
                "image": item["media"].as_array().and_then(|m| m.first()).cloned().unwrap_or("".into()),
                "downloads": item["counts"].get("downloads").cloned().unwrap_or(0.into()), "source": "Lethamyr.com"}));
        }
        match data["links"]["next"].as_str().filter(|s| !s.is_empty()) {
            None => { finished = true; break; }
            Some(next) => {
                let parsed = url::Url::parse(next).map_err(|_| "Paginación no válida.".to_string())?;
                let netloc = match parsed.port() { Some(p) => format!("{}:{p}", parsed.host_str().unwrap_or("")), None => parsed.host_str().unwrap_or("").to_owned() };
                if netloc != "lethamyr.com" || parsed.path() != "/api/v1/maps" { return Err("Paginación no válida.".into()); }
                url = next.to_owned();
            }
        }
    }
    if !finished { return Err("Catálogo demasiado grande.".into()); }
    // Unique by key: first position, last value (as a Python dict comprehension).
    let mut unique: Vec<Value> = Vec::new();
    for m in maps {
        match unique.iter_mut().find(|u| u["key"] == m["key"]) { Some(u) => *u = m, None => unique.push(m) }
    }
    let base = folder(root)?;
    let images = base.join("images");
    std::fs::create_dir_all(&images).map_err(err)?;
    let queue = std::sync::Mutex::new(unique.iter().collect::<Vec<_>>());
    std::thread::scope(|scope| {
        for _ in 0..4 {
            scope.spawn(|| loop {
                let Some(item) = queue.lock().unwrap().pop() else { break };
                let _ = thumbnail(&images, item);
            });
        }
    });
    write(&base.join("catalog.json"), &json!({"maps": unique, "source": CATALOG}))?;
    snapshot(root)
}

fn thumbnail(images: &Path, item: &Value) -> R<()> {
    let dest = images.join(format!("{}.png", item["key"].as_str().unwrap_or("")));
    let url = item["image"].as_str().unwrap_or("");
    if dest.exists() || url.is_empty() { return Ok(()); }
    let raw = fetch(url, 8 * 1024 * 1024)?;
    let reader = image::ImageReader::new(std::io::Cursor::new(&raw)).with_guessed_format().map_err(err)?;
    let (w, h) = reader.into_dimensions().map_err(err)?;
    if w as u64 * h as u64 > 30_000_000 { return Ok(()); }
    let picture = image::load_from_memory(&raw).map_err(err)?;
    // Like PIL's thumbnail: shrink to fit 480x270 keeping the aspect ratio, never enlarge.
    let picture = if picture.width() > 480 || picture.height() > 270 { picture.resize(480, 270, image::imageops::FilterType::CatmullRom) } else { picture };
    picture.to_rgb8().save_with_format(&dest, image::ImageFormat::Png).map_err(err)
}

pub fn validate_map(path: &Path) -> R<String> {
    let size = std::fs::metadata(path).map_err(err)?.len();
    if !(64..=LIMIT).contains(&size) { return Err("Tamaño del mapa no válido.".into()); }
    let mut header = [0u8; 4];
    std::fs::File::open(path).map_err(err)?.read_exact(&mut header).map_err(err)?;
    if header != [0xc1, 0x83, 0x2a, 0x9e] { return Err("No es un paquete Unreal de Rocket League.".into()); }
    // Real UDK map packages contain their own World object; header validation is not a compatibility guarantee.
    util::sha256_file(path)
}

/// A temporary folder inside the library, removed when dropped (like tempfile.TemporaryDirectory).
pub struct TempDir(pub PathBuf);
impl TempDir {
    pub fn new(base: &Path, prefix: &str) -> R<TempDir> {
        let nanos = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_nanos()).unwrap_or(0);
        let path = base.join(format!("{prefix}{:x}{:x}", std::process::id(), nanos));
        std::fs::create_dir_all(&path).map_err(err)?;
        Ok(TempDir(path))
    }
}
impl Drop for TempDir { fn drop(&mut self) { let _ = std::fs::remove_dir_all(&self.0); } }

pub fn import_map(root: &Path, path: &Path, metadata: Option<&Value>) -> R<Value> {
    let empty = json!({});
    let meta = metadata.unwrap_or(&empty);
    let base = folder(root)?;
    let mut library = read(&base.join("library.json"), json!({}))?;
    let suffix = path.extension().map(|e| format!(".{}", e.to_string_lossy().to_lowercase())).unwrap_or_default();
    if ![".zip", ".udk", ".upk"].contains(&suffix.as_str()) { return Err("Importa ZIP, UDK o UPK.".into()); }
    if std::fs::metadata(path).map_err(err)?.len() > LIMIT { return Err("Archivo demasiado grande.".into()); }
    let tmp = TempDir::new(&base, "import-")?;
    let prepared = if suffix == ".zip" {
        let members = super::zip::members(path)?;
        if members.len() > 2048 || members.iter().map(|m| m.size).sum::<u64>() > LIMIT { return Err("ZIP demasiado grande.".into()); }
        for m in &members {
            let name = m.name.replace('\\', "/");
            let parts: Vec<&str> = name.split('/').collect();
            if name.starts_with('/') || parts.contains(&"..") || name.contains(':') || (m.external >> 16) & 0o170000 == 0o120000 { return Err("Ruta no válida dentro del ZIP.".into()); }
        }
        let with = |ext: &str| members.iter().filter(|m| !m.is_dir() && m.name.to_lowercase().ends_with(ext)).collect::<Vec<_>>();
        let mut candidates = with(".udk");
        if candidates.is_empty() { candidates = with(".upk"); }
        let [selected] = candidates[..] else { return Err("El ZIP debe contener un único mapa. Extrae e importa el UDK principal.".into()) };
        let out = tmp.0.join("map.upk");
        super::zip::extract(path, selected, &out)?;
        out
    } else { path.to_path_buf() };
    let digest = validate_map(&prepared)?;
    let key = meta["key"].as_str().filter(|k| !k.is_empty()).map(str::to_owned).unwrap_or(digest.clone());
    if key.len() != 64 || !key.bytes().all(|c| matches!(c, b'0'..=b'9' | b'a'..=b'f')) { return Err("Identificador de biblioteca no válido.".into()); }
    let dest = base.join("maps").join(&key);
    std::fs::create_dir_all(&dest).map_err(err)?;
    std::fs::copy(&prepared, dest.join("map.upk")).map_err(err)?;
    let name = meta["name"].as_str().filter(|n| !n.is_empty()).map(str::to_owned).unwrap_or_else(|| path.file_stem().unwrap_or_default().to_string_lossy().into_owned());
    let source = meta["source"].as_str().unwrap_or("Importado");
    library[&key] = json!({"name": name, "path": dest.join("map.upk").display().to_string(), "sha256": digest, "source": source, "key": key});
    write(&base.join("library.json"), &library)?;
    drop(tmp);
    snapshot(root)
}

/// Google Drive's "can't scan for viruses" page: the form to confirm the download.
fn drive_form(html: &str) -> (String, Vec<(String, String)>) {
    let (mut action, mut fields, mut active) = (String::new(), Vec::new(), false);
    let mut rest = html;
    while let Some(start) = rest.find('<') {
        let Some(end) = rest[start..].find('>') else { break };
        let tag = &rest[start + 1..start + end];
        rest = &rest[start + end + 1..];
        let lower = tag.to_lowercase();
        if lower.starts_with("/form") { active = false; continue; }
        let name = lower.split(|c: char| c.is_whitespace() || c == '/').next().unwrap_or("");
        let attrs = attributes(tag);
        let get = |k: &str| attrs.iter().find(|a| a.0 == k).map(|a| a.1.clone());
        if name == "form" && get("id").as_deref() == Some("download-form") { active = true; action = get("action").unwrap_or_default(); }
        if name == "input" && active && get("type").as_deref() == Some("hidden") {
            if let Some(n) = get("name").filter(|n| ["id", "export", "confirm", "uuid"].contains(&n.as_str())) {
                let value = get("value").unwrap_or_default();
                match fields.iter_mut().find(|f: &&mut (String, String)| f.0 == n) { Some(f) => f.1 = value, None => fields.push((n, value)) }
            }
        }
    }
    (action, fields)
}

fn attributes(tag: &str) -> Vec<(String, String)> {
    let mut out = Vec::new();
    let chars: Vec<char> = tag.chars().collect();
    let mut i = chars.iter().position(|c| c.is_whitespace()).unwrap_or(chars.len());
    while i < chars.len() {
        while i < chars.len() && (chars[i].is_whitespace() || chars[i] == '/') { i += 1; }
        let start = i;
        while i < chars.len() && !chars[i].is_whitespace() && chars[i] != '=' { i += 1; }
        let key: String = chars[start..i].iter().collect::<String>().to_lowercase();
        if key.is_empty() { break; }
        let mut value = String::new();
        if i < chars.len() && chars[i] == '=' {
            i += 1;
            if i < chars.len() && (chars[i] == '"' || chars[i] == '\'') {
                let quote = chars[i];
                i += 1;
                let s = i;
                while i < chars.len() && chars[i] != quote { i += 1; }
                value = chars[s..i].iter().collect();
                i += 1;
            } else {
                let s = i;
                while i < chars.len() && !chars[i].is_whitespace() { i += 1; }
                value = chars[s..i].iter().collect();
            }
        }
        out.push((key, unescape(&value)));
    }
    out
}

fn unescape(text: &str) -> String {
    text.replace("&quot;", "\"").replace("&#39;", "'").replace("&lt;", "<").replace("&gt;", ">").replace("&amp;", "&")
}

pub fn download(root: &Path, key: &str, progress: &dyn Fn(Value)) -> R<Value> {
    let snap = snapshot(root)?;
    let item = snap["maps"].as_array().and_then(|m| m.iter().find(|m| m["key"] == key)).cloned();
    let Some(item) = item.filter(|i| i["url"].as_str().is_some_and(|u| !u.is_empty())) else { return Err("El mapa no tiene enlace de descarga. Usa Importar.".into()) };
    let base = folder(root)?;
    let tmp = TempDir::new(&base, "download-")?;
    let file = tmp.0.join("map.zip");
    let label = format!("Descargando {} (MB)", item["name"].as_str().unwrap_or("").trim());
    // Public files only; never uses accounts, cookies, or executable downloaders.
    let url = item["url"].as_str().unwrap();
    fetch_to(url, &file, &label, progress)?;
    let starts_zip = |f: &Path| -> R<bool> { let mut b = [0u8; 2]; Ok(std::fs::File::open(f).map_err(err)?.read(&mut b).map_err(err)? == 2 && b == *b"PK") };
    if !starts_zip(&file)? && url::Url::parse(url).ok().and_then(|u| u.host_str().map(str::to_owned)).as_deref() == Some("drive.google.com") {
        let mut head = vec![0u8; 100_000];
        let n = std::fs::File::open(&file).map_err(err)?.read(&mut head).map_err(err)?;
        let (action, fields) = drive_form(&String::from_utf8_lossy(&head[..n]));
        if action == "https://drive.usercontent.google.com/download" && fields.iter().any(|f| f.0 == "id" && !f.1.is_empty()) && fields.iter().any(|f| f.0 == "confirm" && !f.1.is_empty()) {
            let query: String = url::form_urlencoded::Serializer::new(String::new()).extend_pairs(&fields).finish();
            fetch_to(&format!("{action}?{query}"), &file, &label, progress)?;
        }
    }
    if !starts_zip(&file)? { return Err("La fuente no entregó un ZIP directo. Descárgalo desde la web del autor y usa Importar.".into()); }
    let result = import_map(root, &file, Some(&item));
    drop(tmp);
    result
}

pub fn remove(root: &Path, key: &str) -> R<Value> {
    let base = folder(root)?;
    let mut library = read(&base.join("library.json"), json!({}))?;
    let entry = library.get(key).filter(|e| !is_empty(e)).cloned().ok_or("El mapa no está descargado.")?;
    let state = read(&root.join("data").join("swaps.json"), json!({}))?;
    let state = &state[TARGET];
    if state["status"] == "installed" && state["installed_sha256"] == entry["sha256"] { return Err("Restaura Utopia Retro antes de borrar el mapa activo.".into()); }
    let path = util::resolve(Path::new(entry["path"].as_str().unwrap_or("")));
    let expected = util::resolve(&base.join("maps").join(key).join("map.upk"));
    let maps = util::resolve(&base.join("maps"));
    if !util::same_path(&path, &expected) || !path.parent().and_then(Path::parent).is_some_and(|p| util::same_path(p, &maps)) { return Err("Ruta de biblioteca no válida.".into()); }
    if path.exists() { std::fs::remove_file(&path).map_err(err)?; }
    std::fs::remove_dir(path.parent().unwrap()).map_err(err)?;
    library.as_object_mut().unwrap().remove(key);
    write(&base.join("library.json"), &library)?;
    snapshot(root)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn drive_confirm_form() {
        let html = r#"<html><form id="download-form" action="https://drive.usercontent.google.com/download" method="get"><input type="hidden" name="id" value="abc"><input type="hidden" name="confirm" value="t&amp;x"><input type="hidden" name="other" value="no"></form><input type="hidden" name="uuid" value="late">"#;
        let (action, fields) = drive_form(html);
        assert_eq!(action, "https://drive.usercontent.google.com/download");
        assert_eq!(fields, vec![("id".into(), "abc".into()), ("confirm".into(), "t&x".into())]);
    }
    #[test]
    fn only_known_sources() {
        assert!(https("https://drive.google.com/uc?id=1").is_ok());
        assert!(https("https://lh3.googleusercontent.com/x").is_ok());
        assert!(https("http://drive.google.com/").is_err());
        assert!(https("https://evil.com/").is_err());
        assert!(https("https://user:pw@lethamyr.com/").is_err());
    }
    #[test]
    fn import_zip_into_library() {
        // A stored ZIP with one .udk that starts with the Unreal package magic.
        let dir = std::env::temp_dir().join(format!("sp-ws-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let mut map = vec![0xc1, 0x83, 0x2a, 0x9e];
        map.resize(100, 7);
        let zip = make_zip("Folder/Map.udk", &map);
        std::fs::write(dir.join("m.zip"), zip).unwrap();
        let snap = import_map(&dir, &dir.join("m.zip"), None).unwrap();
        let entry = &snap["maps"][0];
        assert_eq!(entry["name"], "m");
        assert_eq!(entry["source"], "Importado");
        assert_eq!(std::fs::read(entry["local"]["path"].as_str().unwrap()).unwrap(), map);
        let key = entry["key"].as_str().unwrap().to_owned();
        let after = remove(&dir, &key).unwrap();
        assert!(after["maps"].as_array().unwrap().is_empty());
        let _ = std::fs::remove_dir_all(dir);
    }
    fn make_zip(name: &str, data: &[u8]) -> Vec<u8> {
        let crc = crc32fast::hash(data);
        let mut out = Vec::new();
        let local = |out: &mut Vec<u8>| {
            out.extend(0x0403_4b50u32.to_le_bytes()); out.extend([20, 0, 0, 0, 0, 0, 0, 0, 0, 0]);
            out.extend(crc.to_le_bytes()); out.extend((data.len() as u32).to_le_bytes()); out.extend((data.len() as u32).to_le_bytes());
            out.extend((name.len() as u16).to_le_bytes()); out.extend(0u16.to_le_bytes()); out.extend(name.as_bytes()); out.extend(data);
        };
        local(&mut out);
        let dir_at = out.len() as u32;
        out.extend(0x0201_4b50u32.to_le_bytes()); out.extend([20, 0, 20, 0, 0, 0, 0, 0, 0, 0, 0, 0]);
        out.extend(crc.to_le_bytes()); out.extend((data.len() as u32).to_le_bytes()); out.extend((data.len() as u32).to_le_bytes());
        out.extend((name.len() as u16).to_le_bytes()); out.extend([0u8; 12]); out.extend(0u32.to_le_bytes()); out.extend(name.as_bytes());
        let dir_len = out.len() as u32 - dir_at;
        out.extend(0x0605_4b50u32.to_le_bytes()); out.extend([0, 0, 0, 0, 1, 0, 1, 0]);
        out.extend(dir_len.to_le_bytes()); out.extend(dir_at.to_le_bytes()); out.extend(0u16.to_le_bytes());
        out
    }
}
