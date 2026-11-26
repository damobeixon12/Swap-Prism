//! Public creator catalogs (AlphaConsole, VideoGameMods, Myinstants) and local previews. Downloads never apply
//! game changes. Port of engine/media_store.py with a small tolerant HTML tree builder (like Python's HTMLParser).
use super::util::{self, err, R};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::io::Read;
use std::path::{Path, PathBuf};

const ALPHA: &str = "https://alphaconsole.io/browse";
const VGM: &str = "https://videogamemods.com/vgm/rocketleague/mods?page=3";
const SOUNDS: &str = "https://www.myinstants.com/es/search/";
const VGM_SLUGS: [&str; 5] = ["sakura-boost-meter-1020466", "car-meter-1022094", "pro-team-boost-gauge-1021662", "tribal-fire-boost-meter", "ice-dark-boost-meter"];
const HOSTS: [&str; 7] = ["alphaconsole.io", "s3.alphaconsole.io", "videogamemods.com", "www.videogamemods.com", "uploads.videogamemods.com", "myinstants.com", "www.myinstants.com"];

#[derive(Default, Debug)]
pub struct Node { pub tag: String, pub attrs: Vec<(String, String)>, pub children: Vec<Node>, pub text: String }

impl Node {
    pub fn attr(&self, key: &str) -> &str { self.attrs.iter().find(|a| a.0 == key).map(|a| a.1.as_str()).unwrap_or("") }
    pub fn has_attr(&self, key: &str) -> bool { self.attrs.iter().any(|a| a.0 == key) }
    pub fn classes(&self) -> Vec<&str> { self.attr("class").split_whitespace().collect() }
    /// Descendants in document order, optionally only one tag.
    pub fn all<'a>(&'a self, tag: Option<&str>, out: &mut Vec<&'a Node>) {
        for node in &self.children {
            if tag.is_none_or(|t| node.tag == t) { out.push(node); }
            node.all(tag, out);
        }
    }
    pub fn find_all(&self, tag: &str) -> Vec<&Node> { let mut v = Vec::new(); self.all(Some(tag), &mut v); v }
    /// Whitespace-normalised text of the node and its descendants.
    pub fn content(&self) -> String {
        let inner: Vec<String> = self.children.iter().map(Node::content).collect();
        format!("{} {}", self.text, inner.join(" ")).split_whitespace().collect::<Vec<_>>().join(" ")
    }
}

fn unescape(text: &str) -> String {
    if !text.contains('&') { return text.to_owned(); }
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(i) = rest.find('&') {
        out.push_str(&rest[..i]);
        rest = &rest[i..];
        let end = rest[1..].find(|c: char| !(c.is_ascii_alphanumeric() || c == '#')).map(|e| e + 1).unwrap_or(rest.len());
        let name = &rest[1..end];
        let semi = rest[end..].starts_with(';');
        let value = if let Some(num) = name.strip_prefix("#x").or(name.strip_prefix("#X")) { u32::from_str_radix(num, 16).ok().and_then(char::from_u32).map(String::from) }
            else if let Some(num) = name.strip_prefix('#') { num.parse::<u32>().ok().and_then(char::from_u32).map(String::from) }
            else { match name { "amp" => Some("&".into()), "lt" => Some("<".into()), "gt" => Some(">".into()), "quot" => Some("\"".into()), "apos" => Some("'".into()), "nbsp" => Some("\u{a0}".into()), "times" => Some("×".into()), "copy" => Some("©".into()), "reg" => Some("®".into()), "hellip" => Some("…".into()), "mdash" => Some("—".into()), "ndash" => Some("–".into()), _ => None } };
        match value {
            Some(v) => { out.push_str(&v); rest = &rest[end + semi as usize..]; }
            None => { out.push('&'); rest = &rest[1..]; }
        }
    }
    out.push_str(rest);
    out
}

/// A tolerant HTML tree: void elements never hold children, an end tag closes up to its open element, script/style are raw text.
pub fn parse_html(text: &str) -> Node {
    const VOID: [&str; 10] = ["img", "br", "hr", "input", "meta", "link", "source", "area", "wbr", "embed"];
    let mut stack: Vec<Node> = vec![Node::default()];
    let close = |stack: &mut Vec<Node>, tag: &str| {
        if let Some(i) = (1..stack.len()).rev().find(|&i| stack[i].tag == tag) {
            while stack.len() > i { let node = stack.pop().unwrap(); stack.last_mut().unwrap().children.push(node); }
        }
    };
    let mut pos = 0;
    let bytes = text.as_bytes();
    while pos < text.len() {
        let Some(lt) = text[pos..].find('<').map(|i| pos + i) else { stack.last_mut().unwrap().text.push_str(&unescape(&text[pos..])); break };
        if lt > pos { stack.last_mut().unwrap().text.push_str(&unescape(&text[pos..lt])); }
        if text[lt..].starts_with("<!--") { pos = text[lt + 4..].find("-->").map(|i| lt + 4 + i + 3).unwrap_or(text.len()); continue; }
        if text[lt..].starts_with("<!") || text[lt..].starts_with("<?") { pos = text[lt..].find('>').map(|i| lt + i + 1).unwrap_or(text.len()); continue; }
        let closing = bytes.get(lt + 1) == Some(&b'/');
        let name_start = lt + 1 + closing as usize;
        if !bytes.get(name_start).is_some_and(|b| b.is_ascii_alphabetic()) { stack.last_mut().unwrap().text.push('<'); pos = lt + 1; continue; }
        // Find the tag end, skipping quoted attribute values.
        let (mut i, mut quote) = (name_start, None);
        while i < bytes.len() { match (quote, bytes[i]) { (None, b'"') | (None, b'\'') => quote = Some(bytes[i]), (Some(q), b) if b == q => quote = None, (None, b'>') => break, _ => {} } i += 1; }
        let inner = &text[name_start..i.min(text.len())];
        pos = (i + 1).min(text.len());
        let name_end = inner.find(|c: char| c.is_whitespace() || c == '/' || c == '>').unwrap_or(inner.len());
        let tag = inner[..name_end].to_lowercase();
        if closing { close(&mut stack, &tag); continue; }
        let attrs = parse_attrs(&inner[name_end..]);
        let self_closing = inner.trim_end().ends_with('/');
        let node = Node { tag: tag.clone(), attrs, ..Default::default() };
        if tag == "script" || tag == "style" {
            // Raw text until the matching end tag.
            let lower = text[pos..].to_lowercase();
            let end = lower.find(&format!("</{tag}")).map(|e| pos + e).unwrap_or(text.len());
            let mut node = node;
            node.text = text[pos..end].to_owned();
            stack.last_mut().unwrap().children.push(node);
            pos = text[end..].find('>').map(|e| end + e + 1).unwrap_or(text.len());
            continue;
        }
        if VOID.contains(&tag.as_str()) { stack.last_mut().unwrap().children.push(node); continue; }
        stack.push(node);
        if self_closing { close(&mut stack, &tag); }
    }
    while stack.len() > 1 { let node = stack.pop().unwrap(); stack.last_mut().unwrap().children.push(node); }
    stack.pop().unwrap()
}

fn parse_attrs(text: &str) -> Vec<(String, String)> {
    let chars: Vec<char> = text.chars().collect();
    let mut out: Vec<(String, String)> = Vec::new();
    let mut i = 0;
    while i < chars.len() {
        while i < chars.len() && (chars[i].is_whitespace() || chars[i] == '/') { i += 1; }
        let start = i;
        while i < chars.len() && !chars[i].is_whitespace() && chars[i] != '=' && chars[i] != '/' { i += 1; }
        if start == i { i += 1; continue; }
        let key: String = chars[start..i].iter().collect::<String>().to_lowercase();
        let mut j = i;
        while j < chars.len() && chars[j].is_whitespace() { j += 1; }
        let mut value = String::new();
        if j < chars.len() && chars[j] == '=' {
            j += 1;
            while j < chars.len() && chars[j].is_whitespace() { j += 1; }
            if j < chars.len() && (chars[j] == '"' || chars[j] == '\'') {
                let q = chars[j];
                let s = j + 1;
                j = s;
                while j < chars.len() && chars[j] != q { j += 1; }
                value = chars[s..j.min(chars.len())].iter().collect();
                j += 1;
            } else {
                let s = j;
                while j < chars.len() && !chars[j].is_whitespace() { j += 1; }
                value = chars[s..j].iter().collect();
            }
            i = j;
        }
        // Python's dict(attrs): a repeated attribute keeps the last value.
        let value = unescape(&value);
        match out.iter_mut().find(|a| a.0 == key) { Some(a) => a.1 = value, None => out.push((key, value)) }
    }
    out
}

fn allowed(url: &str) -> R<url::Url> {
    let p = url::Url::parse(url).map_err(|_| "Usa un enlace HTTPS de AlphaConsole, VGM o Myinstants.".to_string())?;
    if p.scheme() != "https" || !HOSTS.contains(&p.host_str().unwrap_or("")) || !p.username().is_empty() || p.password().is_some() {
        return Err("Usa un enlace HTTPS de AlphaConsole, VGM o Myinstants.".into());
    }
    Ok(p)
}

fn fetch(url: &str, limit: u64) -> R<Vec<u8>> {
    let tls = native_tls::TlsConnector::new().map_err(err)?;
    let agent = ureq::AgentBuilder::new().redirects(0).timeout_connect(std::time::Duration::from_secs(30)).timeout_read(std::time::Duration::from_secs(30))
        .user_agent("SwapPrism/0.25 (public cosmetic browser)").tls_connector(std::sync::Arc::new(tls)).build();
    let mut next = allowed(url)?;
    for _ in 0..10 {
        let response = agent.get(next.as_str()).call().map_err(|e| match e { ureq::Error::Status(code, _) => format!("HTTP Error {code}"), other => other.to_string() })?;
        if (300..400).contains(&response.status()) {
            next = next.join(response.header("Location").ok_or("Redirección sin destino.")?).map_err(err)?;
            continue;
        }
        allowed(next.as_str())?;
        let mut raw = Vec::new();
        response.into_reader().take(limit + 1).read_to_end(&mut raw).map_err(err)?;
        if raw.len() as u64 > limit { return Err("La descarga supera el tamaño permitido.".into()); }
        return Ok(raw);
    }
    Err("Demasiadas redirecciones.".into())
}

fn base(root: &Path) -> R<PathBuf> { let p = root.join("data").join("media"); std::fs::create_dir_all(&p).map_err(err)?; Ok(p) }

fn load(root: &Path, source: &str) -> R<Value> {
    let p = base(root)?.join(format!("{source}.json"));
    if !p.exists() { return Ok(json!({"items": [], "source": source})); }
    serde_json::from_slice(&std::fs::read(&p).map_err(err)?).map_err(err)
}

fn write(root: &Path, source: &str, data: &Value) -> R<()> {
    let p = base(root)?.join(format!("{source}.json"));
    let tmp = p.with_extension("tmp");
    std::fs::write(&tmp, util::dumps_compact(data)).map_err(err)?;
    std::fs::rename(&tmp, &p).map_err(err)
}

fn make(source: &str, name: &str, page: &str, category: &str, download: &str, image: &str, author: &str) -> Value {
    json!({"key": format!("{:x}", Sha256::digest(page.as_bytes())), "source": source, "name": name, "page": page, "category": category, "download": download, "image": image, "author": author})
}

fn join_url(base: &str, href: &str) -> String { url::Url::parse(base).and_then(|b| b.join(href)).map(|u| u.to_string()).unwrap_or_else(|_| href.to_owned()) }

fn alpha(html: &str) -> Vec<Value> {
    let root = parse_html(html);
    let mut result = Vec::new();
    for card in root.find_all("div") {
        let classes = card.classes();
        if !["card", "ratio", "hover-zoom"].iter().all(|c| classes.contains(c)) { continue; }
        let image = card.find_all("img").into_iter().find(|n| n.attr("class").contains("preview-image-img"));
        let link = card.find_all("a").into_iter().find(|n| n.attr("href").starts_with("/view/"));
        let label = card.find_all("div").into_iter().find(|n| n.attr("class").contains("item-label-"));
        let (Some(image), Some(link), Some(label)) = (image, link, label) else { continue };
        let category = [("item-label-boostmeter", "Boostmeter"), ("item-label-ball", "Pelota"), ("item-label-decal", "Decal")].iter().find(|(k, _)| label.classes().contains(k)).map(|c| c.1);
        let Some(category) = category else { continue };
        let author = card.find_all("a").into_iter().find(|n| n.attr("href").starts_with("/user/")).map(Node::content).unwrap_or_default();
        let download = card.find_all("form").into_iter().find(|n| n.attr("action").starts_with("https://s3.alphaconsole.io/uploads/")).map(|n| n.attr("action").to_owned()).unwrap_or_default();
        let name = if image.attr("title").is_empty() { if image.has_attr("alt") { image.attr("alt") } else { "Textura" } } else { image.attr("title") };
        result.push(make("alpha", name, &join_url(ALPHA, link.attr("href")), category, &download, image.attr("src"), &author));
    }
    result
}

/// Public Next/RSC strings are decoded as data, never executed.
fn next_strings(text: &str) -> String {
    let mut out = String::new();
    let marker = "self.__next_f.push([1,\"";
    let mut rest = text;
    while let Some(i) = rest.find(marker) {
        let start = i + marker.len() - 1;
        let body = &rest[start..];
        let (mut j, mut escaped) = (1, false);
        let b = body.as_bytes();
        while j < b.len() { if escaped { escaped = false; } else if b[j] == b'\\' { escaped = true; } else if b[j] == b'"' || b[j] == b'\n' || b[j] == b'\r' { break; } j += 1; }
        if j < b.len() && b[j] == b'"' && body[j + 1..].starts_with("])") {
            if let Ok(s) = serde_json::from_str::<String>(&body[..=j]) { out.push('\n'); out.push_str(&s); }
        }
        rest = &rest[i + marker.len()..];
    }
    out
}

fn file_path(text: &str) -> Option<String> {
    let mut rest = text;
    while let Some(i) = rest.find("\"file_path\"") {
        let after = rest[i + 11..].trim_start();
        if let Some(after) = after.strip_prefix(':') {
            if let Some(value) = after.trim_start().strip_prefix('"') {
                let end = value.find(['"', '\r', '\n']).unwrap_or(value.len());
                if value[end..].starts_with('"') && end > 4 && value[..end].ends_with(".zip") { return Some(value[..end].to_owned()); }
            }
        }
        rest = &rest[i + 11..];
    }
    None
}

fn vgm_detail(url: &str) -> R<Value> {
    let html = String::from_utf8(fetch(url, 5 * 1024 * 1024)?).map_err(err)?;
    let root = parse_html(&html);
    let mut meta: Vec<(String, String)> = Vec::new();
    for n in root.find_all("meta") {
        let key = if n.has_attr("property") && !n.attr("property").is_empty() { n.attr("property") } else { n.attr("name") };
        match meta.iter_mut().find(|m| m.0 == key) { Some(m) => m.1 = n.attr("content").into(), None => meta.push((key.into(), n.attr("content").into())) }
    }
    let get = |k: &str| meta.iter().find(|m| m.0 == k).map(|m| m.1.clone()).unwrap_or_default();
    let name = get("og:title").split(" - Rocket League Mod").next().unwrap_or("").to_owned();
    let mut text = root.find_all("script").iter().map(|n| n.text.clone()).collect::<Vec<_>>().join("\n");
    text.push_str(&next_strings(&text.clone()));
    let title = name.to_lowercase();
    let category = if ["meter", "gauge", "hud"].iter().any(|v| title.contains(v)) { "Boostmeter" } else if title.contains("ball") { "Pelota" } else if title.contains("decal") { "Decal" } else { "Otros" };
    let download = file_path(&text).map(|p| join_url("https://uploads.videogamemods.com/", &p)).unwrap_or_default();
    Ok(make("vgm", &name, url, category, &download, &get("og:image"), ""))
}

fn sounds(html: &str) -> Vec<Value> {
    let root = parse_html(html);
    let mut result = Vec::new();
    for card in root.find_all("div") {
        if !card.classes().contains(&"instant") { continue; }
        let link = card.find_all("a").into_iter().find(|n| n.classes().contains(&"instant-link"));
        let button = card.find_all("button").into_iter().find(|n| n.attr("onclick").starts_with("play("));
        let (Some(link), Some(button)) = (link, button) else { continue };
        let onclick = button.attr("onclick");
        let audio = onclick.find("play('/media/sounds/").and_then(|i| { let s = &onclick[i + 6..]; s.find('\'').map(|e| &s[..e]) }).filter(|p| p.ends_with(".mp3") && p.len() > "/media/sounds/.mp3".len());
        if let Some(audio) = audio { result.push(make("sounds", &link.content(), &join_url(SOUNDS, link.attr("href")), "Sonido", &join_url(SOUNDS, audio), "", "")); }
    }
    result
}

pub fn catalog(root: &Path, source: &str, query: &str, refresh: bool, url: &str) -> R<Value> {
    let old = load(root, source)?;
    let mut items: Vec<Value> = old["items"].as_array().cloned().unwrap_or_default();
    if refresh {
        let mut incoming: Vec<Value> = if !url.is_empty() {
            allowed(url)?;
            if source == "vgm" && url.contains("/vgm/rocketleague/mods/") { vec![vgm_detail(url)?] }
            else if source == "alpha" && url.contains("/view/") {
                let html = String::from_utf8(fetch(url, 5 * 1024 * 1024)?).map_err(err)?;
                let root_node = parse_html(&html);
                let meta = |k: &str| root_node.find_all("meta").into_iter().filter(|n| n.attr("property") == k).last().map(|n| n.attr("content").to_owned());
                let download = root_node.find_all("form").into_iter().find(|n| n.attr("action").starts_with("https://s3.alphaconsole.io/uploads/")).map(|n| n.attr("action").to_owned()).unwrap_or_default();
                let name = meta("og:title").unwrap_or("Textura AlphaConsole".into()).split('|').next().unwrap_or("").trim().to_owned();
                let types = [("item-label-boostmeter", "Boostmeter"), ("item-label-ball", "Pelota"), ("item-label-decal", "Decal")];
                let mut all = Vec::new();
                root_node.all(None, &mut all);
                let category = all.iter().flat_map(|n| n.classes()).find_map(|c| types.iter().find(|t| t.0 == c).map(|t| t.1)).ok_or("La ficha no es una pelota, boostmeter o decal compatible.")?;
                vec![make("alpha", &name, url, category, &download, &meta("og:image").unwrap_or_default(), "")]
            } else { return Err("Pega la ficha de una textura de la fuente seleccionada.".into()); }
        } else {
            match source {
                "alpha" => alpha(&String::from_utf8_lossy(&fetch(ALPHA, 5 * 1024 * 1024)?)),
                "sounds" => sounds(&String::from_utf8_lossy(&fetch(&format!("{SOUNDS}?{}", url::form_urlencoded::Serializer::new(String::new()).append_pair("name", query).finish()), 5 * 1024 * 1024)?)),
                "vgm" => {
                    let found: Vec<Value> = VGM_SLUGS.iter().filter_map(|s| vgm_detail(&format!("https://videogamemods.com/vgm/rocketleague/mods/{s}")).ok()).collect();
                    if found.is_empty() { return Err("VGM no entregó fichas. Puedes abrir su web e importar un ZIP.".into()); }
                    found
                }
                _ => return Err("Fuente no válida.".into()),
            }
        };
        if incoming.is_empty() { return Err("No se encontraron resultados en esta fuente.".into()); }
        for item in incoming.iter_mut() {
            if let Some(prev) = items.iter().find(|m| m["key"] == item["key"]) {
                for field in ["local", "digests"] { if let Some(v) = prev.get(field) { item[field] = v.clone(); } }
            }
        }
        items = if !url.is_empty() {
            let mut merged: Vec<Value> = Vec::new();
            for m in items.into_iter().chain(incoming) { match merged.iter_mut().find(|x| x["key"] == m["key"]) { Some(x) => *x = m, None => merged.push(m) } }
            merged
        } else { incoming };
        let images = base(root)?.join("images");
        std::fs::create_dir_all(&images).map_err(err)?;
        let slots: Vec<std::sync::Mutex<&mut Value>> = items.iter_mut().map(std::sync::Mutex::new).collect();
        let next = std::sync::atomic::AtomicUsize::new(0);
        std::thread::scope(|scope| {
            for _ in 0..3 {
                scope.spawn(|| loop {
                    let i = next.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                    let Some(slot) = slots.get(i) else { break };
                    let mut item = slot.lock().unwrap();
                    let Some(url) = item["image"].as_str().filter(|u| !u.is_empty()).map(str::to_owned) else { continue };
                    let dest = images.join(format!("{}.png", item["key"].as_str().unwrap_or("")));
                    let ok = dest.exists() || (|| -> R<()> {
                        let raw = fetch(&url, 8 * 1024 * 1024)?;
                        let picture = image::load_from_memory(&raw).map_err(err)?;
                        if picture.width() as u64 * picture.height() as u64 > 25_000_000 { return Err(String::new()); }
                        let picture = if picture.width() > 420 || picture.height() > 260 { picture.resize(420, 260, image::imageops::FilterType::CatmullRom) } else { picture };
                        picture.to_rgb8().save_with_format(&dest, image::ImageFormat::Png).map_err(err)
                    })().is_ok();
                    if ok { item["preview"] = dest.display().to_string().into(); }
                });
            }
        });
    }
    let data = json!({"items": items, "source": source});
    write(root, source, &data)?;
    Ok(data)
}

pub fn audio_library(root: &Path) -> Vec<Value> {
    std::fs::read_dir(root.join("data").join("sounds")).into_iter().flatten().flatten().map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|x| x == "json"))
        .filter_map(|p| serde_json::from_slice::<Value>(&std::fs::read(p).ok()?).ok())
        .filter(|item| (item["kind"] == "post" || item["kind"] == "reset") && item["path"].as_str().is_some_and(|p| Path::new(p).is_file())).collect()
}

/// Fetch and play a short clip from the catalog; never imported or marked downloaded.
pub fn preview_sound(root: &Path, key: &str, kind: &str) -> R<Value> {
    let data = load(root, "sounds")?;
    let item = data["items"].as_array().into_iter().flatten().find(|m| m["key"] == key).filter(|m| m["download"].as_str().is_some_and(|d| !d.is_empty())).cloned().ok_or("Sonido no disponible para preescucha.")?;
    let specs = super::studio::manifest();
    let spec = specs.get(kind).ok_or("Evento de audio no válido.")?;
    let raw = fetch(item["download"].as_str().unwrap(), 8 * 1024 * 1024)?;
    let tmp = std::env::temp_dir().join(format!("sp-preview-{}.mp3", util::uuid_hex()));
    std::fs::write(&tmp, &raw).map_err(err)?;
    let pcm = super::studio::decode_mp3(&tmp);
    let _ = std::fs::remove_file(&tmp);
    let pcm = pcm?;
    let frames = pcm.len() / 2;
    if frames == 0 || frames > 300 * 48000 { return Err("El audio supera 5 minutos o no es válido.".into()); }
    let keep = ((spec["max_seconds"].as_f64().unwrap_or(0.) * 48000.).round() as usize).min(frames);
    if keep == 0 { return Err("El sonido está vacío.".into()); }
    super::studio::play_wave(&super::studio::wav(&pcm[..keep * 2]))?;
    Ok(json!({"previewed": true, "name": item["name"], "media": data}))
}

pub fn download(root: &Path, source: &str, key: &str, kind: &str, preview: bool) -> R<Value> {
    if source == "sounds" && preview { return preview_sound(root, key, kind); }
    let mut data = load(root, source)?;
    let index = data["items"].as_array().and_then(|items| items.iter().position(|m| m["key"] == key)).filter(|&i| data["items"][i]["download"].as_str().is_some_and(|d| !d.is_empty()));
    let Some(index) = index else { return Err("Esta ficha requiere descarga desde su web. Después usa Importar.".into()) };
    let item = data["items"][index].clone();
    let local = base(root)?.join("files");
    std::fs::create_dir_all(&local).map_err(err)?;
    if source == "sounds" {
        let path = local.join(format!("{key}.mp3"));
        if !path.exists() { std::fs::write(&path, fetch(item["download"].as_str().unwrap(), 8 * 1024 * 1024)?).map_err(err)?; }
        let mut audio = match super::studio::import_mp3(root, &path, kind) { Ok(a) => a, Err(e) => { let _ = std::fs::remove_file(&path); return Err(e); } };
        audio["name"] = item["name"].as_str().unwrap_or("").chars().take(64).collect::<String>().into();
        audio["catalog_key"] = key.into();
        std::fs::write(Path::new(audio["path"].as_str().unwrap()).with_extension("json"), util::dumps_compact(&audio)).map_err(err)?;
        data["items"][index]["local"] = true.into();
        write(root, source, &data)?;
        return Ok(json!({"media": data, "sounds": audio_library(root)}));
    }
    let raw = fetch(item["download"].as_str().unwrap(), 128 * 1024 * 1024)?;
    if !raw.starts_with(b"PK") { return Err("La fuente no entregó un ZIP. Abre la ficha e impórtalo manualmente.".into()); }
    let tmp = super::workshop::TempDir::new(&base(root)?, "texture-")?;
    let path = tmp.0.join("pack.zip");
    std::fs::write(&path, &raw).map_err(err)?;
    let (packs, errors) = super::library::scan(&path);
    if packs.is_empty() { return Err(format!("El pack no es compatible con este importador. {}", errors.first().cloned().unwrap_or_default())); }
    super::library::save_library(&root.join("data").join("texture_library"), &packs)?;
    drop(tmp);
    data["items"][index]["local"] = true.into();
    data["items"][index]["digests"] = packs.iter().map(|p| p.digest.clone()).collect::<Vec<_>>().into();
    write(root, source, &data)?;
    Ok(json!({"media": data}))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn html_tree_like_htmlparser() {
        let root = parse_html(r#"<div class="instant"><a class="instant-link" href="/es/instant/x/">Hola &amp; adiós</a><button onclick="play('/media/sounds/x.mp3', 'a')"></button><br><img src=a.png></div><script>if(a<b){}</script>"#);
        assert_eq!(root.find_all("a")[0].content(), "Hola & adiós");
        assert_eq!(root.find_all("img")[0].attr("src"), "a.png");
        assert_eq!(root.find_all("script")[0].text, "if(a<b){}");
        let found = sounds(r#"<div class="instant"><a class="instant-link" href="/es/instant/x/">Hola</a><button onclick="play('/media/sounds/x.mp3', 'a')"></button></div>"#);
        assert_eq!(found[0]["download"], "https://www.myinstants.com/media/sounds/x.mp3");
    }
    #[test]
    fn next_payload_strings() {
        let text = r#"self.__next_f.push([1,"{\"file_path\":\"mods/a b.zip\"}"])"#;
        assert_eq!(file_path(&next_strings(text)).as_deref(), Some("mods/a b.zip"));
    }
}
