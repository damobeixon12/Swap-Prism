//! Prepared change jobs: item swaps (with colour), body bridges, texture packs and the Utopia Retro map. Port of
//! engine/cosmetic_backend.py, the generate parts of engine/body_visual.py and engine/texture_engine.py, and
//! engine/workshop.prepare. Every job is a folder with informe.json and generado/<package>, installed separately.
use super::state::{pristine, sha};
use super::upk::{Package, rewire};
use super::util::{self, err, R};
use super::library::TexturePack;
use serde_json::{json, Value};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

pub const SLOTS: [&str; 10] = ["Body", "Decal", "Wheels", "Rocket Boost", "Trail", "Topper", "Antenna", "Goal Explosion", "Player Banner", "Avatar Border"];
const STANDARD_CARS: [&str; 17] = ["Octane", "Fennec", "Dominus", "Breakout", "Merc", "Backfire", "Gizmo", "Hotshot", "Paladin", "Road Hog", "Venom", "X-Devil", "Takumi", "Scarab", "Zippy", "Ripper", "Grog"];

/// Every catalogued item (items.json merged with the enrichment data), shared with the interface.
pub fn database() -> &'static Vec<Value> {
    static ROWS: std::sync::OnceLock<Vec<Value>> = std::sync::OnceLock::new();
    ROWS.get_or_init(|| serde_json::from_slice(include_bytes!("../../assets/catalog.json")).unwrap_or_default())
}

fn text<'a>(v: &'a Value, key: &str) -> &'a str { v[key].as_str().unwrap_or("") }

pub fn safe_package(name: &str) -> R<&str> {
    if name.contains('/') || name.contains('\\') || name.contains(':') || !name.to_lowercase().ends_with(".upk") || name == "." || name == ".." || name.len() <= 4 {
        return Err("Nombre de paquete no válido.".into());
    }
    Ok(name)
}

/// Items with a package present in this game, sorted by slot, name and id.
pub fn catalog(game: &Path) -> R<Vec<Value>> {
    let cooked = game.join("TAGame").join("CookedPCConsole");
    let mut rows = Vec::new();
    for row in database() {
        if !SLOTS.contains(&text(row, "Slot")) || text(row, "AssetPath").is_empty() { continue; }
        if cooked.join(safe_package(text(row, "AssetPackage"))?).is_file() { rows.push(row.clone()); }
    }
    rows.sort_by(|a, b| (text(a, "Slot"), text(a, "Product"), a["ID"].as_i64()).cmp(&(text(b, "Slot"), text(b, "Product"), b["ID"].as_i64())));
    Ok(rows)
}

/// Conservative filters, not a claim of in-game or online compatibility.
pub fn compatibility(item: &Value, car: &str) -> (bool, String) {
    let slot = text(item, "Slot");
    if slot == "Body" { return (!text(item, "AssetPath").is_empty() && !text(item, "AssetPackage").is_empty(), "Solo puente visual sobre los datos físicos originales; requiere validación de modelo.".into()); }
    if !SLOTS.contains(&slot) { return (false, "Esta categoría solo está disponible para consulta.".into()); }
    if !STANDARD_CARS.contains(&car) { return (false, "Este coche necesita reglas de compatibilidad verificadas.".into()); }
    let restricted = item["Attributes"].as_array().into_iter().flatten().any(|a| ["IsLicensed", "PreviewOnly", "InclusivePlatforms"].contains(&text(a, "Key")));
    if restricted { return (false, "Objeto con restricciones de licencia, plataforma o vista previa.".into()); }
    let product = text(item, "Product");
    if slot == "Decal" {
        if let Some((required, _)) = product.split_once(':') {
            let required = required.trim();
            if required.to_lowercase() != car.to_lowercase() { return (false, format!("Decal específico de {required}.")); }
            return (true, format!("Decal identificado para {car}; confirma que lo permite tu garaje."));
        }
    }
    (true, format!("Confirma en el garaje que este objeto puede equiparse en {car}."))
}

/// json.dumps(report, ensure_ascii=False, indent=2) as Python's text mode writes it.
fn write_report(path: &Path, report: &Value, ascii: bool) -> R<()> { util::write_text(path, &util::dumps(report, ascii)) }

fn new_job(output: &Path) -> R<PathBuf> {
    let job = output.join(util::uuid_hex());
    std::fs::create_dir_all(job.join("generado")).map_err(err)?;
    Ok(job)
}

/// An item swap (optionally recoloured) or a body bridge.
pub fn generate_item(root: &Path, game: &Path, output: &Path, target_id: &Value, donor_id: &Value, version: &str, car: &str, color: &str) -> R<PathBuf> {
    super::require_closed()?;
    let game = util::resolve(game);
    let rows = catalog(&game)?;
    let find = |id: &Value| rows.iter().filter(|r| r["ID"].to_string().trim_matches('"') == id.to_string().trim_matches('"')).cloned().collect::<Vec<_>>();
    let (targets, donors) = (find(target_id), find(donor_id));
    let ([target], [donor]) = (&targets[..], &donors[..]) else { return Err("Elige dos objetos del catálogo disponible.".into()) };
    if target["ID"] == donor["ID"] && color == "Original" { return Err("Elige una apariencia distinta.".into()); }
    if text(target, "Slot") != text(donor, "Slot") { return Err("Los dos objetos deben ser de la misma categoría.".into()); }
    for item in [target, donor] { let (ok, reason) = compatibility(item, car); if !ok { return Err(reason); } }
    let cooked = game.join("TAGame").join("CookedPCConsole");
    let data = root.join("data");
    if text(target, "Slot") == "Body" {
        if color != "Original" { return Err("El puente de carrocería conserva la pintura original. Selecciona Original.".into()); }
        let source = pristine(&cooked, text(target, "AssetPackage"), &data)?;
        let other = Package::open(&pristine(&cooked, text(donor, "AssetPackage"), &data)?)?;
        let original = Package::open(&source)?;
        let (raw, validation) = super::body::generate(&original, other, target, donor)?;
        let job = new_job(output)?;
        let file = job.join("generado").join(text(target, "AssetPackage"));
        std::fs::write(&file, &raw).map_err(err)?;
        let report = json!({"schema": 5, "kind": "body_visual", "target": target, "donor": donor, "game_version": version,
            "original_sha256": {text(target, "AssetPackage"): sha(&source)?}, "generated_sha256": sha(&file)?, "validation": validation});
        write_report(&job.join("informe.json"), &report, false)?;
        return Ok(job);
    }
    let uses = |package: &str| database().iter().filter(|r| text(r, "AssetPackage").to_lowercase() == package.to_lowercase()).count();
    if [target, donor].iter().any(|i| uses(text(i, "AssetPackage")) > 1) { return Err("El paquete es compartido y requiere soporte de edición por objeto.".into()); }
    let mut names: Vec<String> = [target, donor].iter().map(|i| safe_package(text(i, "AssetPackage")).map(str::to_owned)).collect::<R<_>>()?;
    names.sort();
    names.dedup();
    let originals_from: Vec<(String, PathBuf)> = names.iter().map(|n| Ok((n.clone(), pristine(&cooked, n, &data)?))).collect::<R<_>>()?;
    let live_before: Vec<String> = names.iter().map(|n| sha(&cooked.join(n))).collect::<R<_>>()?;
    let job = output.join(util::uuid_hex());
    let (originals, staged) = (job.join("originales"), job.join("generado"));
    std::fs::create_dir_all(&originals).map_err(err)?;
    std::fs::create_dir_all(&staged).map_err(err)?;
    let mut original_hash = serde_json::Map::new();
    for (name, source) in &originals_from {
        let source_hash = sha(source)?;
        std::fs::copy(source, originals.join(name)).map_err(err)?;
        if sha(&originals.join(name))? != source_hash || sha(source)? != source_hash { return Err("Un archivo cambió durante la copia.".into()); }
        original_hash.insert(name.clone(), source_hash.into());
    }
    let attempt = (|| -> R<()> {
        let mut donor_package = Package::open(&originals.join(text(donor, "AssetPackage")))?;
        let target_package = Package::open(&originals.join(text(target, "AssetPackage")))?;
        let mut color_report = Value::Null;
        if color != "Original" {
            let (painted, report) = super::textures::paint(donor_package, color, Some(&cooked))?;
            donor_package = painted;
            color_report = report;
        }
        let (result, mut validation) = rewire(&donor_package, &target_package, donor, target)?;
        validation["color"] = color_report;
        let generated = staged.join(text(target, "AssetPackage"));
        std::fs::write(&generated, &result).map_err(err)?;
        if names.iter().map(|n| sha(&cooked.join(n))).collect::<R<Vec<_>>>()? != live_before { return Err("Los paquetes del juego cambiaron durante la generación.".into()); }
        let report = json!({"schema": 2, "status": "generated", "game_version": version, "target": target, "donor": donor, "car": car,
            "original_sha256": original_hash, "generated_sha256": sha(&generated)?, "validation": validation, "online_verified": false,
            "training_tested_pair": target["ID"] == 5287 && donor["ID"] == 2044 && version == "++Prime+Update60.1-CL-533021",
            "note": "Cosmético local. Los assets cargados pueden requerir reiniciar el juego."});
        write_report(&job.join("informe.json"), &report, false)?;
        write_report(&job.join("motor.log"), &report["validation"], false)?;
        Ok(())
    })();
    if let Err(e) = attempt { let _ = std::fs::write(job.join("error.log"), &e); return Err(e); }
    Ok(job)
}

pub const BALL_MAP: [(&str, &str); 3] = [("Diffuse", "Ball_Default00_D"), ("Normal", "Ball_Default00_N"), ("Masks", "Ball_Default00_RGB")];
pub const METER_MAP: [(&str, &str); 4] = [("Background", "BoostMeter_Background"), ("Fill", "BoostMeter_Fill"), ("Tint", "BoostMeter_FillTintablePortion"), ("Glow", "BoostMeter_Glow")];

/// A texture pack applied to its package (ball, boost meter, decal or boost).
pub fn generate_texture(root: &Path, game: &Path, pack: &TexturePack, output: &Path, target: Option<&Value>, boost_texture: Option<&str>, package: Option<&str>) -> R<PathBuf> {
    use super::colors::export_data;
    super::require_closed()?;
    let mut target = target.cloned();
    let mut base: Option<Value> = None;
    if pack.category == "Decal" {
        let found = database().iter().find(|r| r["ID"] == pack.skin_id && text(r, "Slot") == "Decal" && !text(r, "AssetPath").is_empty()).cloned();
        let found = found.ok_or("El SkinID del pack no tiene un paquete verificado.")?;
        if target.is_none() { target = Some(found.clone()); }
        let uses = |p: &str| database().iter().filter(|r| text(r, "AssetPackage") == p).count();
        if uses(text(target.as_ref().unwrap(), "AssetPackage")) > 1 || uses(text(&found, "AssetPackage")) > 1 { return Err("El decal usa un paquete compartido. Elige otro destino.".into()); }
        base = Some(found);
    }
    let (mut name, mapping): (String, Option<&[(&str, &str)]>) = match pack.category.as_str() {
        "Pelota" => (package.unwrap_or("GameInfo_Soccar_SF.upk").into(), Some(&BALL_MAP)),
        "Boostmeter" => ("GFX_Hud_SF.upk".into(), Some(&METER_MAP)),
        _ => {
            let t = target.as_ref().filter(|t| ["Decal", "Rocket Boost"].contains(&text(t, "Slot"))).ok_or("Selecciona un decal o boost como destino.")?;
            (text(base.as_ref().unwrap_or(t), "AssetPackage").into(), None)
        }
    };
    let cooked = game.join("TAGame").join("CookedPCConsole");
    let data = root.join("data");
    let source = pristine(&cooked, &name, &data)?;
    let pkg = Package::open(&source)?;
    let flat = pkg.flat()?;
    let mut replacements: BTreeMap<usize, Vec<u8>> = BTreeMap::new();
    let mut used: Vec<String> = Vec::new();
    if pack.category == "Boost" {
        let wanted = boost_texture.unwrap_or("");
        let matches: Vec<usize> = (0..pkg.exports.len()).filter(|&i| pkg.class_name(&pkg.exports[i]) == "Texture2D" && pkg.path(i).is_ok_and(|p| p == wanted)).collect();
        let [index] = matches[..] else { return Err("Selecciona la textura exacta del boost.".into()) };
        replacements.insert(index, super::textures::inline_serial(&pkg, export_data(flat, &pkg.exports[index]), pack.layer("Diffuse").ok_or("Capa no compatible: Diffuse")?, false)?);
        used.push(wanted.into());
    } else if let Some(mapping) = mapping {
        let mut layers = pack.layers.clone();
        if pack.category == "Boostmeter" {
            // Empty AlphaConsole Tint means no team-tinted overlay, not "keep vanilla".
            if let Some(i) = layers.iter().position(|l| l.0 == "Tintable") { let (_, raw) = layers.remove(i); layers.retain(|l| l.0 != "Tint"); layers.push(("Tint".into(), raw)); }
            if !layers.iter().any(|l| l.0 == "Tint") { layers.push(("Tint".into(), super::dxt::png(&image::RgbaImage::new(256, 256))?)); }
        }
        for (role, raw) in &layers {
            let texture = mapping.iter().find(|m| m.0 == role).ok_or(format!("Capa no compatible: {role}"))?.1;
            let matches: Vec<usize> = (0..pkg.exports.len()).filter(|&i| pkg.class_name(&pkg.exports[i]) == "Texture2D" && pkg.export_name(&pkg.exports[i]) == texture).collect();
            let [index] = matches[..] else { return Err(format!("No se localiza la textura {texture}")) };
            replacements.insert(index, super::textures::inline_serial(&pkg, export_data(flat, &pkg.exports[index]), raw, false)?);
            used.push(role.clone());
        }
    } else {
        let params = super::textures::bindings(&pkg, flat);
        for (role, raw) in &pack.layers {
            let indexes: Vec<usize> = params.iter().find(|p| p.0 == *role).map(|p| p.1.clone()).unwrap_or_default();
            // AlphaConsole packs sometimes duplicate Diffuse under the actual shader slot.
            if indexes.is_empty() && role == "Diffuse" && pack.layer("1_Diffuse_Skin") == Some(raw) && params.iter().any(|p| p.0 == "1_Diffuse_Skin" && !p.1.is_empty()) { continue; }
            if indexes.is_empty() && role == "2_Diffuse_Skin_Mask" && super::textures::add_mask_binding(&pkg, flat, raw, &mut replacements)? { used.push(role.clone()); continue; }
            if indexes.is_empty() { return Err(format!("El material del destino no admite la capa {role}. Selecciona el decal base del pack.")); }
            for index in indexes { replacements.insert(index, super::textures::inline_serial(&pkg, export_data(flat, &pkg.exports[index]), raw, false)?); }
            used.push(role.clone());
        }
    }
    if replacements.is_empty() { return Err("No hay texturas compatibles para aplicar.".into()); }
    let mut hud_color = Value::Null;
    if pack.category == "Boostmeter" {
        let (index, serial, report) = super::hud::patch_boostmeter_movie(&pkg, flat)?;
        replacements.insert(index, serial);
        hud_color = report;
    }
    let mut result = super::textures::append_exports(&pkg, &replacements, None, None)?;
    let mut original_hash = serde_json::Map::new();
    original_hash.insert(name.clone(), sha(&source)?.into());
    if let (Some(b), Some(t)) = (&base, &target) {
        if b["ID"] != t["ID"] {
            let target_source = pristine(&cooked, text(t, "AssetPackage"), &data)?;
            let target_pkg = Package::open(&target_source)?;
            let painted = Package::from_raw(&source, result)?;
            result = rewire(&painted, &target_pkg, b, t)?.0;
            name = text(t, "AssetPackage").into();
            original_hash.insert(name.clone(), sha(&target_source)?.into());
        }
    }
    let job = new_job(output)?;
    let generated = job.join("generado").join(&name);
    std::fs::write(&generated, &result).map_err(err)?;
    let target = target.unwrap_or_else(|| json!({"ID": null, "Product": pack.category, "Slot": "Texture", "AssetPackage": name}));
    let mut report = json!({"schema": 3, "kind": "texture", "target": target, "donor": {"ID": null, "Product": pack.name, "Slot": "Texture"},
        "original_sha256": original_hash, "generated_sha256": sha(&generated)?,
        "validation": {"texture_and_material_bindings_only": hud_color.is_null(), "texture_and_hud_display_only": !hud_color.is_null(), "other_payloads_unchanged": true,
            "layers": used, "exports": replacements.len(), "visual_verified": false,
            "vanilla_tint_removed": pack.category == "Boostmeter" && pack.layer("Tint").is_none() && pack.layer("Tintable").is_none()},
        "source_pack_sha256": pack.digest});
    report["validation"]["hud_display_color"] = hud_color;
    write_report(&job.join("informe.json"), &report, false)?;
    Ok(job)
}

/// Same ball texture in every mode's package; skips packages another change already uses.
/// The ball in every other package that carries it (`taken`: packages already used by other changes are left alone).
pub fn generate_ball_companions(root: &Path, game: &Path, pack: &TexturePack, output: &Path, taken: &[String]) -> R<Vec<PathBuf>> {
    let cooked = game.join("TAGame").join("CookedPCConsole");
    let state = super::state::load_state(&root.join("data"))?;
    let names: Vec<String> = super::ball_companions(&cooked, &root.join("data"))?.into_iter().filter(|name| {
        let entry = state.get(name);
        cooked.join(name).exists() && !taken.contains(&name.to_lowercase())
            && !entry.is_some_and(|e| e["status"] == "installed" && e["hud_display_companion"] != true)
    }).collect();
    // Packages are independent; a few at a time (maps are large, memory stays bounded).
    let threads = std::thread::available_parallelism().map_or(2, |n| n.get()).clamp(1, 4);
    let next = std::sync::atomic::AtomicUsize::new(0);
    let results: Vec<R<PathBuf>> = std::thread::scope(|s| {
        let workers: Vec<_> = (0..threads).map(|_| s.spawn(|| {
            let mut done = Vec::new();
            loop {
                let i = next.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                let Some(name) = names.get(i) else { break };
                done.push((i, (|| {
                    let job = generate_texture(root, game, pack, output, None, None, Some(name))?;
                    let mut report = super::state::read_report(&job)?;
                    report["hud_display_companion"] = true.into();
                    let label = name.trim_end_matches(".upk").trim_end_matches("_SF").trim_end_matches("_sf");
                    let label = if label.to_lowercase().starts_with("gameinfo_") { &label[9..] } else { label };
                    report["target"]["Product"] = format!("Pelota · {label}").into();
                    write_report(&job.join("informe.json"), &report, false)?;
                    Ok(job)
                })()));
            }
            done
        })).collect();
        let mut all: Vec<(usize, R<PathBuf>)> = workers.into_iter().flat_map(|w| w.join().unwrap_or_default()).collect();
        all.sort_by_key(|r| r.0);
        all.into_iter().map(|r| r.1).collect()
    });
    results.into_iter().collect()
}

/// Utopia Retro replaced by a library map, or restored.
pub fn prepare_workshop(root: &Path, game: &Path, selection: &Value) -> R<PathBuf> {
    use super::workshop::{folder, read, validate_map, TARGET};
    super::require_closed()?;
    if selection["target"].as_str().unwrap_or(TARGET) != TARGET { return Err("El destino es exclusivamente Utopia Retro.".into()); }
    let source = pristine(&game.join("TAGame").join("CookedPCConsole"), TARGET, &root.join("data"))?;
    let (raw, label, restore) = if selection["kind"] == "workshop_restore" { (source.clone(), "Original".to_owned(), true) } else {
        let library = read(&folder(root)?.join("library.json"), json!({}))?;
        let entry = library.get(selection["map_key"].as_str().unwrap_or("")).filter(|e| !e.is_null()).ok_or("Descarga o importa el mapa primero.")?;
        let raw = util::resolve(Path::new(text(entry, "path")));
        if !raw.parent().and_then(Path::parent).is_some_and(|p| util::same_path(p, &folder(root).unwrap().join("maps"))) || validate_map(&raw)? != text(entry, "sha256") {
            return Err("El mapa de biblioteca ha cambiado.".into());
        }
        (raw, text(entry, "name").to_owned(), false)
    };
    let job = root.join("paquetes").join(format!("workshop-{}", util::uuid_hex()));
    std::fs::create_dir_all(job.join("generado")).map_err(err)?;
    std::fs::copy(&raw, job.join("generado").join(TARGET)).map_err(err)?;
    let report = json!({"kind": "workshop", "target": {"Product": "Workshop · Utopia Retro", "Slot": "Workshop", "AssetPackage": TARGET},
        "donor": {"Product": label, "Slot": "Workshop"}, "original_sha256": {TARGET: sha(&source)?}, "generated_sha256": sha(&raw)?,
        "validation": {"workshop_map_validated": true, "fixed_utopia_retro_only": true}, "restore_selection": restore});
    super::workshop::write(&job.join("informe.json"), &report)?;
    Ok(job)
}
