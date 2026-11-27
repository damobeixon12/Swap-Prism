//! Local profile (name, title, rank, avatar, banner): settings, package layers and their bookkeeping.
//! Port of engine/profile_manager.py.
use super::util::{self, err, R};
use serde_json::{json, Value};
use std::path::Path;

pub const SCHEMA: i64 = 1;
/// Bump when generated profile packages change, so installed profiles are rebuilt.
pub const CODEC_REVISION: i64 = 17;

pub fn load(root: &Path) -> R<Value> {
    let path = root.join("data").join("profile.json");
    if !path.exists() { return Ok(json!({"schema": SCHEMA, "settings": {"name": "", "avatar": "", "banner": ""}, "packages": {}})); }
    let value: Value = serde_json::from_slice(&std::fs::read(&path).map_err(err)?).map_err(err)?;
    if value["schema"] != SCHEMA { return Err("Versión de perfil no reconocida.".into()); }
    Ok(value)
}

pub fn save(root: &Path, value: &Value) -> R<()> {
    let path = root.join("data").join("profile.json");
    std::fs::create_dir_all(path.parent().unwrap()).map_err(err)?;
    let tmp = path.with_extension("tmp");
    util::write_text(&tmp, &util::dumps(value, true))?;
    std::fs::rename(&tmp, &path).map_err(err)
}

/// Packages restored outside the profile no longer belong to it.
pub fn after_restore(root: &Path, names: Option<&[String]>) -> R<()> {
    let mut profile = load(root)?;
    let packages = profile["packages"].as_object_mut().ok_or("profile.json no válido.")?;
    let selected: Vec<String> = packages.keys().filter(|k| names.is_none_or(|n| n.contains(k))).cloned().collect();
    for name in &selected { packages.remove(name); }
    let remaining = !packages.is_empty();
    // A partial package restore cannot represent a complete profile installation.
    profile["partial"] = (remaining && !selected.is_empty()).into();
    if !remaining { profile["settings"] = json!({"name": "", "avatar": "", "banner": ""}); profile["partial"] = false.into(); }
    let folder = root.join("data");
    let mut state = super::state::load_state(&folder)?;
    for name in &selected { if let Some(entry) = state.get_mut(name).and_then(Value::as_object_mut) { entry.remove("profile_layer"); } }
    super::state::write_state(&folder, &state)?;
    save(root, &profile)
}

use super::display::{normalize_rank, normalize_title, patch_package, validate_name};
use super::state::{filename, is_epic_original, load_state, modified, pristine, sha, write_state};
use super::upk::Package;
use std::path::PathBuf;

/// A picture for the avatar (512²) or banner (840x200), stored by content hash. `crop` ([x, y, width, height] as fractions
/// of the picture) is the area chosen in the viewer; without it the centre is cropped to fit.
pub fn import_image(root: &Path, source: &Path, kind: &str, crop: Option<[f64; 4]>) -> R<String> {
    if kind != "avatar" && kind != "banner" { return Err("Tipo de imagen no válido.".into()); }
    if std::fs::metadata(source).map_err(err)?.len() > 20 * 1024 * 1024 { return Err("La imagen supera los 20 MB.".into()); }
    let reader = image::ImageReader::open(source).map_err(err)?.with_guessed_format().map_err(err)?;
    let mut decoder = reader.into_decoder().map_err(err)?;
    let orientation = image::ImageDecoder::orientation(&mut decoder).ok();
    let mut picture = image::DynamicImage::from_decoder(decoder).map_err(err)?;
    if picture.width() as u64 * picture.height() as u64 > 40_000_000 { return Err("Imagen demasiado grande.".into()); }
    if let Some(orientation) = orientation { picture.apply_orientation(orientation); }
    let (w, h) = if kind == "avatar" { (512, 512) } else { (840, 200) };
    let rgba = picture.to_rgba8();
    let sized = match crop {
        Some([x, y, cw, ch]) => {
            if !(x >= 0. && y >= 0. && cw > 0. && ch > 0. && x + cw <= 1.0001 && y + ch <= 1.0001) { return Err("Recorte no válido.".into()); }
            let (pw, ph) = (rgba.width() as f64, rgba.height() as f64);
            let (left, top) = ((x * pw).round() as u32, (y * ph).round() as u32);
            let width = ((cw * pw).round() as u32).clamp(1, rgba.width() - left.min(rgba.width() - 1));
            let height = ((ch * ph).round() as u32).clamp(1, rgba.height() - top.min(rgba.height() - 1));
            super::dxt::resize(&image::imageops::crop_imm(&rgba, left.min(rgba.width() - 1), top.min(rgba.height() - 1), width, height).to_image(), w, h)
        }
        None => super::dxt::fit(&rgba, w, h),
    };
    let raw = super::dxt::png(&sized)?;
    let folder = root.join("data").join("profile").join("images");
    std::fs::create_dir_all(&folder).map_err(err)?;
    use sha2::{Digest, Sha256};
    let target = folder.join(format!("{:x}.png", Sha256::digest(&raw)));
    std::fs::write(&target, &raw).map_err(err)?;
    Ok(util::resolve(&target).display().to_string())
}

/// Validated profile settings: name, title, rank and the imported pictures.
pub fn normalize(root: &Path, settings: &Value) -> R<Value> {
    let mut result = json!({"name": validate_name(settings.get("name").unwrap_or(&json!("")))?});
    result["title"] = normalize_title(settings.get("title").unwrap_or(&Value::Null))?.unwrap_or(Value::Null);
    result["rank"] = normalize_rank(settings.get("rank").unwrap_or(&Value::Null))?.unwrap_or(Value::Null);
    let image_root = util::resolve(&root.join("data").join("profile").join("images"));
    for key in ["avatar", "banner"] {
        let value = settings[key].as_str().unwrap_or("");
        if value.is_empty() { result[key] = "".into(); continue; }
        let p = util::resolve(Path::new(value));
        if !p.parent().is_some_and(|d| util::same_path(d, &image_root)) || !p.is_file() || p.extension().is_none_or(|e| e != "png") { return Err("Importa la imagen desde el editor de perfil.".into()); }
        super::dxt::load(&std::fs::read(&p).map_err(err)?)?;
        result[key] = p.display().to_string().into();
    }
    // Which owned banner gets the picture (its package); absent = the equipped one.
    if let Some(item) = settings["banner_item"].as_str().filter(|i| !i.is_empty() && !result["banner"].as_str().unwrap_or("").is_empty()) { result["banner_item"] = item.into(); }
    Ok(result)
}

/// The package a profile layer is built on: the game file, or the archived base under an installed layer.
fn base(cooked: &Path, name: &str, profile: &Value) -> R<PathBuf> {
    let entry = &profile["packages"][name];
    if entry.is_null() || entry.as_object().is_some_and(|o| o.is_empty()) { return Ok(cooked.join(name)); }
    if sha(&cooked.join(name))? != entry["installed_sha256"].as_str().unwrap_or("") {
        if is_epic_original(cooked, name)? == Some(true) { return Ok(cooked.join(name)); } // reset by verify/update: rebuild on the original
        return Err(modified(name));
    }
    let path = PathBuf::from(entry["base"].as_str().unwrap_or(""));
    if sha(&path)? != entry["base_sha256"].as_str().unwrap_or("") { return Err(format!("La base del perfil no coincide: {name}")); }
    Ok(path)
}

fn archive(root: &Path, source: &Path) -> R<(PathBuf, String)> {
    let digest = sha(source)?;
    let folder = root.join("data").join("profile").join("bases").join(&digest);
    std::fs::create_dir_all(&folder).map_err(err)?;
    let target = folder.join(source.file_name().unwrap());
    if !target.exists() { std::fs::copy(source, &target).map_err(err)?; }
    if sha(&target)? != digest { return Err("No se verificó la base del perfil.".into()); }
    Ok((target, digest))
}

const SECTIONS: [&str; 5] = ["name", "title", "rank", "avatar", "banner"];

fn truthy(v: &Value) -> bool {
    match v { Value::Null => false, Value::Bool(b) => *b, Value::Number(n) => n.as_f64() != Some(0.), Value::String(s) => !s.is_empty(), Value::Array(a) => !a.is_empty(), Value::Object(o) => !o.is_empty() }
}

/// Packages that need no rebuild when only some sections changed; None means rebuild all.
/// A section switched on for the first time is unknown territory, so it forces a full scan.
fn unaffected(cooked: &Path, settings: &Value, profile: &Value) -> R<Option<Vec<String>>> {
    let old = &profile["settings"];
    let get = |v: &Value, k: &str| v.get(k).cloned().unwrap_or(Value::Null);
    let changed: Vec<&str> = SECTIONS.iter().copied().filter(|k| get(settings, k) != get(old, k)).collect();
    let packages = profile["packages"].as_object().cloned().unwrap_or_default();
    if changed.is_empty() || profile["codec_revision"] != CODEC_REVISION || truthy(&profile["partial"]) || packages.is_empty()
        || changed.iter().any(|k| *k != "banner" && !truthy(&get(old, k))) || packages.values().any(|e| e.get("features").is_none()) {
        return Ok(None);
    }
    let mut keep = Vec::new();
    for (n, e) in &packages {
        let features: Vec<&str> = e["features"].as_array().into_iter().flatten().filter_map(Value::as_str).collect();
        if !features.iter().any(|f| changed.contains(f)) && sha(&cooked.join(n))? == e["installed_sha256"].as_str().unwrap_or("") { keep.push(n.clone()); }
    }
    Ok(Some(keep))
}

/// Package of the banner that gets the picture: the one chosen from the inventory, or the one equipped (local save).
fn equipped_banner(settings: &Value) -> R<Option<String>> {
    let snap = super::inventory::load_snapshot(&super::jobs::database())?;
    if let Some(item) = settings["banner_item"].as_str() {
        let owned = ["inventory", "account_equipped"].iter().flat_map(|k| snap[*k].as_array().into_iter().flatten()).any(|r| r["Slot"] == "Player Banner" && r["AssetPackage"] == item);
        return if owned { Ok(Some(item.to_owned())) } else { Err("El banner elegido ya no está en tu inventario guardado.".into()) };
    }
    Ok(snap["account_equipped"].as_array().into_iter().flatten().find(|r| r["Slot"] == "Player Banner" && r["AssetPackage"].as_str().is_some_and(|p| !p.is_empty())).and_then(|r| r["AssetPackage"].as_str().map(str::to_owned)))
}

type Prepared = Option<(PathBuf, Option<Value>)>;

fn generate(root: &Path, game: &Path, settings: &Value, profile: &Value, banner_package: Option<String>, progress: &dyn Fn(Value)) -> R<(Vec<PathBuf>, Value)> {
    let cooked = game.join("TAGame").join("CookedPCConsole");
    let mut keep = unaffected(&cooked, settings, profile)?;
    let old_banner = profile["banner_package"].as_str().map(str::to_owned);
    if let Some(k) = keep.as_mut() { if banner_package != old_banner { if let Some(b) = &old_banner { k.retain(|n| n != b); } } }
    let packages = profile["packages"].as_object().cloned().unwrap_or_default();
    let mut names: std::collections::BTreeSet<String> = packages.keys().cloned().collect();
    let (has_name, has_title, has_avatar, has_rank, has_banner) = (truthy(&settings["name"]), truthy(&settings["title"]), truthy(&settings["avatar"]), truthy(&settings["rank"]), truthy(&settings["banner"]));
    if has_name || has_title || has_avatar || has_rank {
        for e in std::fs::read_dir(&cooked).map_err(err)?.flatten() {
            let n = e.file_name().to_string_lossy().into_owned();
            let l = n.to_lowercase();
            if l.ends_with(".upk") && l.starts_with("gfx") && !l.contains("sounds") { names.insert(n); }
        }
    }
    // Startup contains the shared components/code loaded by the other menus.
    if has_avatar || has_banner || has_name || has_title || has_rank { names.insert("Startup.upk".into()); }
    if let Some(k) = &keep { names = packages.keys().filter(|n| !k.contains(n)).cloned().collect(); }
    if has_banner {
        names.insert("Startup.upk".into());
        if let Some(b) = &banner_package { if cooked.join(b).is_file() { names.insert(b.clone()); } }
    }
    let mut images: Vec<(String, Vec<u8>)> = Vec::new();
    if has_avatar { images.push(("avatar".into(), std::fs::read(settings["avatar"].as_str().unwrap()).map_err(err)?)); }
    if has_banner { images.push(("banner".into(), std::fs::read(settings["banner"].as_str().unwrap()).map_err(err)?)); }
    let folder = root.join("paquetes").join(format!("perfil-{}", util::uuid_hex()));
    std::fs::create_dir_all(&folder).map_err(err)?;
    let mut new = json!({"schema": SCHEMA, "codec_revision": CODEC_REVISION, "settings": settings, "banner_package": if has_banner { json!(banner_package) } else { Value::Null }, "packages": {}});
    for n in keep.iter().flatten() { new["packages"][n] = packages[n].clone(); }
    let names: Vec<String> = names.into_iter().collect();
    let total = names.len();
    let data = root.join("data");
    let avatar = images.iter().find(|i| i.0 == "avatar").map(|i| i.1.clone());
    let banner = images.iter().find(|i| i.0 == "banner").cloned();
    let prepare_one = |name: &str| -> R<Prepared> {
        super::require_closed()?;
        filename(name)?;
        let original = pristine(&cooked, name, &data)?;
        let source = base(&cooked, name, profile)?;
        // Check current tracked state even when no previous profile layer exists.
        let state = load_state(&data)?.get(name).cloned();
        if let Some(s) = &state {
            let now = sha(&cooked.join(name))?;
            if now != s["original_sha256"].as_str().unwrap_or("") && now != s["installed_sha256"].as_str().unwrap_or("") { return Err(modified(name)); }
        }
        // Archive by the original package name; never feed a renamed UPK to codecs.
        let (archived, digest) = archive(root, &source)?;
        let pkg = Package::open(&source)?;
        let pkg_images: Vec<(String, Vec<u8>)> = if name == "Startup.upk" { images.clone() } else if Some(name) == banner_package.as_deref() { banner.iter().cloned().collect() } else { Vec::new() };
        let patched = patch_package(&pkg, settings["name"].as_str().unwrap_or(""), &pkg_images, &settings["title"], avatar.as_deref(), &settings["rank"])?;
        let (raw, validation, entry) = match patched {
            None => {
                if !packages.contains_key(name) { return Ok(None); }
                (std::fs::read(&source).map_err(err)?, json!({"only_client_presentation": true, "other_payloads_unchanged": true, "exports": []}), None)
            }
            Some((raw, v)) => {
                use sha2::{Digest, Sha256};
                let entry = json!({"base": util::resolve(&archived).display().to_string(), "base_sha256": digest, "installed_sha256": format!("{:x}", Sha256::digest(&raw)),
                    "exports": v["exports"], "features": v.get("features").cloned().unwrap_or(json!([]))});
                (raw, v, Some(entry))
            }
        };
        let job = folder.join(name);
        std::fs::create_dir_all(job.join("generado")).map_err(err)?;
        let generated = job.join("generado").join(name);
        std::fs::write(&generated, &raw).map_err(err)?;
        let state_value = state.clone().unwrap_or(Value::Null);
        let base_state = if util::same_path(&source, &cooked.join(name)) { state_value.clone() } else { packages[name].get("base_state").cloned().unwrap_or(state_value) };
        let donor = settings["name"].as_str().filter(|n| !n.is_empty()).unwrap_or("Avatar / banner personalizado");
        let report = json!({"schema": 3, "kind": "profile", "target": {"Product": "Perfil local", "AssetPackage": name, "Slot": "Profile"},
            "donor": {"Product": donor, "Slot": "Profile"}, "original_sha256": {name: sha(&original)?}, "generated_sha256": sha(&generated)?, "validation": validation,
            "profile_base_state": base_state});
        let entry = entry.map(|mut e| { e["base_state"] = report["profile_base_state"].clone(); e });
        util::write_text(&job.join("informe.json"), &util::dumps(&report, true))?;
        Ok(Some((job, entry)))
    };
    let queue = std::sync::Mutex::new(names.iter().collect::<Vec<_>>());
    let results: std::sync::Mutex<Vec<(String, R<Prepared>)>> = std::sync::Mutex::new(Vec::new());
    // Four packages at a time; workers only signal, progress is reported from this thread.
    let (tx, rx) = std::sync::mpsc::channel::<()>();
    let (queue_ref, results_ref, work) = (&queue, &results, &prepare_one);
    std::thread::scope(|scope| {
        for _ in 0..4 {
            let tx = tx.clone();
            scope.spawn(move || loop {
                let Some(name) = queue_ref.lock().unwrap().pop() else { break };
                let result = work(name);
                let stop = result.is_err();
                results_ref.lock().unwrap().push((name.clone(), result));
                let _ = tx.send(());
                if stop { queue_ref.lock().unwrap().clear(); }
            });
        }
        drop(tx);
        for (done, ()) in rx.iter().enumerate() {
            progress(json!({"phase": "prepare", "current": done + 1, "total": total, "message": "Preparando perfil y título"}));
        }
    });
    let mut jobs = Vec::new();
    for (name, result) in results.into_inner().unwrap() {
        if let Some((job, entry)) = result? {
            if let Some(entry) = entry { new["packages"][&name] = entry; }
            jobs.push(job);
        }
    }
    jobs.sort_by_key(|j| j.file_name().map(|f| f.to_os_string()));
    Ok((jobs, new))
}

/// Jobs for the requested profile (none when it is already installed exactly).
pub fn prepare(root: &Path, game: &Path, settings: &Value, progress: &dyn Fn(Value)) -> R<(Vec<PathBuf>, Value)> {
    super::require_closed()?;
    let profile = load(root)?;
    let mut settings = settings.clone();
    if settings.get("title").is_none() { settings["title"] = profile["settings"].get("title").cloned().unwrap_or(Value::Null); }
    if settings.get("rank").is_none() { settings["rank"] = profile["settings"].get("rank").cloned().unwrap_or(Value::Null); }
    let settings = normalize(root, &settings)?;
    if truthy(&settings["title"]) {
        let snap = super::inventory::load_snapshot(&super::jobs::database())?;
        let mut owned: Vec<Value> = snap["inventory"].as_array().into_iter().flatten().filter(|r| r["blueprint"] != true).map(|r| r.get("title_id").cloned().unwrap_or(Value::Null)).collect();
        owned.extend(snap["account_equipped"].as_array().into_iter().flatten().filter(|r| r["Slot"] == "Player Title").map(|r| r["Product"].clone()));
        if !owned.contains(&settings["title"]["target"]) { return Err("El título original no aparece en tu inventario guardado.".into()); }
        let equipped = snap["account_equipped"].as_array().into_iter().flatten().find(|r| r["Slot"] == "Player Title").map(|r| r["Product"].clone()).unwrap_or(json!(""));
        if settings["title"]["target"] != equipped { return Err("Equipa el título original elegido en Rocket League y ciérralo antes de aplicar.".into()); }
    }
    let banner_package = if truthy(&settings["banner"]) { equipped_banner(&settings)? } else { None };
    if profile["codec_revision"] == CODEC_REVISION && !truthy(&profile["partial"]) && settings == profile["settings"] && json!(banner_package) == profile.get("banner_package").cloned().unwrap_or(Value::Null) {
        let cooked = game.join("TAGame").join("CookedPCConsole");
        let mut all = true;
        for (name, entry) in profile["packages"].as_object().cloned().unwrap_or_default() {
            if sha(&cooked.join(&name))? != entry["installed_sha256"].as_str().unwrap_or("") { all = false; break; }
        }
        if all { progress(json!({"phase": "done", "current": 1, "total": 1, "message": "El perfil ya estaba aplicado."})); return Ok((Vec::new(), profile)); }
    }
    generate(root, game, &settings, &profile, banner_package, progress)
}

/// Reconcile the generic package registry with its underlying cosmetic state.
pub fn commit_jobs(root: &Path, jobs: &[PathBuf], new: &Value) -> R<Value> {
    let data = root.join("data");
    let mut state = load_state(&data)?;
    for job in jobs {
        let report = super::state::read_report(job)?;
        let name = report["target"]["AssetPackage"].as_str().unwrap_or("").to_owned();
        let base = &report["profile_base_state"];
        let has_base = truthy(base);
        if new["packages"].get(&name).is_some() {
            if has_base {
                let mut entry = base.clone();
                entry["installed_sha256"] = report["generated_sha256"].clone();
                entry["status"] = "installed".into();
                entry["profile_layer"] = true.into();
                state.insert(name, entry);
            } else if let Some(e) = state.get_mut(&name) { e["profile_layer"] = true.into(); }
        } else if has_base { state.insert(name, base.clone()); }
        else if let Some(e) = state.get_mut(&name) { e["status"] = "restored".into(); e.as_object_mut().unwrap().remove("profile_layer"); }
    }
    write_state(&data, &state)?;
    save(root, new)?;
    Ok(json!({"count": jobs.len(), "state": state, "profile": new}))
}

pub fn apply(root: &Path, game: &Path, settings: &Value, progress: &dyn Fn(Value)) -> R<Value> {
    let (jobs, new) = prepare(root, game, settings, progress)?;
    let count = super::state::install_all(root, &jobs, game, progress)?;
    let mut result = commit_jobs(root, &jobs, &new)?;
    result["count"] = count.into();
    result["cached"] = jobs.is_empty().into();
    super::state::discard(root, &jobs);
    Ok(result)
}

/// Put the profile over newly selected skins; metadata is committed after the install succeeds.
pub fn compose_jobs(root: &Path, jobs: &[PathBuf], profile_override: Option<&Value>) -> R<Value> {
    let profile = match profile_override { Some(p) => p.clone(), None => load(root)? };
    let mut updated = profile.clone();
    let packages = profile["packages"].as_object().cloned().unwrap_or_default();
    if packages.is_empty() { return Ok(updated); }
    updated["_composed"] = json!([]);
    let settings = &profile["settings"];
    for job in jobs {
        let report_file = job.join("informe.json");
        let mut report = super::state::read_report(job)?;
        let name = report["target"]["AssetPackage"].as_str().unwrap_or("").to_owned();
        if !packages.contains_key(&name) { continue; }
        let source = job.join("generado").join(&name);
        let (archived, digest) = archive(root, &source)?;
        let mut all_images: Vec<(String, Vec<u8>)> = Vec::new();
        for k in ["avatar", "banner"] { if let Some(p) = settings[k].as_str().filter(|p| !p.is_empty()) { all_images.push((k.into(), std::fs::read(p).map_err(err)?)); } }
        let images: Vec<(String, Vec<u8>)> = if name == "Startup.upk" { all_images.clone() }
            else if Some(name.as_str()) == profile["banner_package"].as_str() { all_images.iter().filter(|i| i.0 == "banner").cloned().collect() } else { Vec::new() };
        let avatar = all_images.iter().find(|i| i.0 == "avatar").map(|i| i.1.as_slice());
        let (raw, validation) = patch_package(&Package::open(&source)?, settings["name"].as_str().unwrap_or(""), &images, &settings["title"], avatar, &settings["rank"])?
            .ok_or(format!("No se pudo conservar el perfil en {name}"))?;
        std::fs::write(&source, &raw).map_err(err)?;
        report["generated_sha256"] = sha(&source)?.into();
        report["profile_composition"] = validation.clone();
        util::write_text(&report_file, &util::dumps(&report, true))?;
        updated["packages"][&name] = json!({"base": util::resolve(&archived).display().to_string(), "base_sha256": digest, "installed_sha256": sha(&source)?,
            "exports": validation["exports"], "features": validation.get("features").cloned().unwrap_or(json!([])), "base_state": null});
        updated["_composed"].as_array_mut().unwrap().push(name.into());
    }
    Ok(updated)
}

pub fn commit_composed(root: &Path, profile: &mut Value) -> R<()> {
    let data = root.join("data");
    let mut state = load_state(&data)?;
    let composed: Vec<String> = profile.as_object_mut().and_then(|o| o.remove("_composed")).and_then(|v| serde_json::from_value(v).ok()).unwrap_or_default();
    for name in composed {
        if let Some(s) = state.get_mut(&name) {
            let entry = &mut profile["packages"][&name];
            let mut base_state = s.clone();
            base_state["installed_sha256"] = entry["base_sha256"].clone();
            if entry["base_sha256"] == s["original_sha256"] { base_state["status"] = "restored".into(); s["status"] = "restored".into(); }
            entry["base_state"] = base_state;
            s["profile_layer"] = true.into();
        }
    }
    save(root, profile)?;
    write_state(&data, &state)
}
