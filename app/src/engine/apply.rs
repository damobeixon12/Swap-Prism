//! The "apply" action: validate the queue against the saved inventory, prepare every job (skins, textures, map,
//! shared companions, replaced originals, studio, profile) and install them as one transaction.
//! Port of engine/swapprism_engine.py (apply) and engine/change_queue.py (prepare_all, superseded_jobs).
use super::library::{load_library, TexturePack};
use super::state::{filename, load_state, pristine, read_report, sha};
use super::upk::Package;
use super::util::{self, err, R};
use serde_json::{json, Value};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

enum Plan { Workshop(Value), Item { target: Value, donor: Value, car: String, color: String }, Texture { pack: TexturePack, target: Option<Value> } }

impl Plan {
    fn category(&self) -> String {
        match self { Plan::Workshop(_) => "Workshop".into(), Plan::Item { target, .. } => target["Slot"].as_str().unwrap_or("").into(), Plan::Texture { pack, .. } => pack.category.clone() }
    }
}

fn package_of(job: &Path) -> R<String> { Ok(read_report(job)?["target"]["AssetPackage"].as_str().unwrap_or("").to_owned()) }

/// The shared boost meter code library copies (GFxShell, replay and spectator HUD) get the same display-only change.
fn generate_meter_companions(root: &Path, game: &Path, pack: &TexturePack, output: &Path) -> R<Vec<PathBuf>> {
    use super::colors::export_properties;
    use super::hud::{abc_tag, neutral_boostmeter_gfx};
    super::require_closed()?;
    let cooked = game.join("TAGame").join("CookedPCConsole");
    let data = root.join("data");
    let profile = super::profile::load(root)?;
    let state = load_state(&data)?;
    let mut jobs = Vec::new();
    for name in super::METER_COMPANIONS {
        let source = pristine(&cooked, name, &data)?;
        let entry = &profile["packages"][name];
        let underlying = if entry.is_object() && !entry.as_object().unwrap().is_empty() { entry["base_state"].clone() } else { state.get(name).cloned().unwrap_or(Value::Null) };
        if underlying["status"] == "installed" && underlying["hud_display_companion"] != true { return Err(format!("Otro cambio usa la biblioteca del medidor: {name}")); }
        let pkg = Package::open(&source)?;
        let flat = pkg.flat()?;
        let mut replacements: BTreeMap<usize, Vec<u8>> = BTreeMap::new();
        let mut reports = Vec::new();
        for (i, e) in pkg.exports.iter().enumerate() {
            if pkg.class_name(e) != "SwfMovie" { continue; }
            let (data, props) = export_properties(&pkg, flat, e);
            let Some(raw) = props.iter().find(|p| p.name == "RawData" && p.kind == "ArrayProperty") else { continue };
            let at = raw.value + 4;
            let old = &data[at..at + raw.size - 4];
            let Ok((.., abc)) = abc_tag(old) else { continue };
            if abc.class("tagame.hud.BoostMeterView").is_none() { continue; }
            let (new, report) = neutral_boostmeter_gfx(old)?;
            reports.push(report);
            let mut serial = [&data[..at], &new[..], &data[at + old.len()..]].concat();
            serial[raw.value - 8..raw.value - 4].copy_from_slice(&(4 + new.len() as i32).to_le_bytes());
            serial[raw.value..raw.value + 4].copy_from_slice(&(new.len() as i32).to_le_bytes());
            if let Some(crc) = props.iter().find(|p| (p.name == "SourceFileCRC" || p.name == "RawDataCRC") && p.size == 4) {
                let pos = (crc.value as i64 + if crc.value > at { new.len() as i64 - old.len() as i64 } else { 0 }) as usize;
                serial[pos..pos + 4].copy_from_slice(&crc32fast::hash(&new).to_le_bytes());
            }
            replacements.insert(i, serial);
        }
        if reports.len() != 1 { return Err(format!("La copia compartida del medidor cambió: {name}")); }
        let check = replacements.clone();
        let validator = move |i: usize, serial: &[u8]| check.get(&i).is_some_and(|r| r == serial);
        let result = super::textures::append_exports(&pkg, &replacements, Some(&validator), None)?;
        let job = output.join(util::uuid_hex());
        std::fs::create_dir_all(job.join("generado")).map_err(err)?;
        let generated = job.join("generado").join(name);
        std::fs::write(&generated, &result).map_err(err)?;
        let report = json!({"schema": 3, "kind": "texture", "hud_display_companion": true,
            "target": {"ID": null, "Product": "Boostmeter · biblioteca compartida", "Slot": "Texture", "AssetPackage": name},
            "donor": {"ID": null, "Product": pack.name, "Slot": "Texture"}, "original_sha256": {name: sha(&source)?}, "generated_sha256": sha(&generated)?,
            "validation": {"texture_and_hud_display_only": true, "other_payloads_unchanged": true, "hud_display_color": reports[0], "visual_verified": false}});
        util::write_text(&job.join("informe.json"), &util::dumps(&report, true))?;
        jobs.push(job);
    }
    Ok(jobs)
}

/// A previously installed change in a category being replaced goes back to its original in the same transaction.
fn superseded_jobs(root: &Path, plans: &[Plan], jobs: &[PathBuf], game: &Path) -> R<Vec<PathBuf>> {
    let categories: Vec<String> = plans.iter().map(Plan::category).collect();
    let occupied: Vec<String> = jobs.iter().map(|j| package_of(j)).collect::<R<_>>()?;
    let mut extra = Vec::new();
    for (name, entry) in load_state(&root.join("data"))? {
        if occupied.contains(&name) || entry["status"] != "installed" || entry["hud_display_companion"] == true { continue; }
        let item = entry["target_item"].clone();
        let mut category = item["Slot"].as_str().unwrap_or("").to_owned();
        if category == "Texture" { category = item["Product"].as_str().unwrap_or("").to_owned(); }
        if !categories.contains(&category) { continue; }
        super::require_closed()?;
        let source = pristine(&game.join("TAGame").join("CookedPCConsole"), &name, &root.join("data"))?;
        let job = root.join("paquetes").join(format!("replace-{}", util::uuid_hex()));
        std::fs::create_dir_all(job.join("generado")).map_err(err)?;
        std::fs::copy(&source, job.join("generado").join(&name)).map_err(err)?;
        let digest = sha(&source)?;
        let report = json!({"kind": "restore", "restore_selection": true, "target": item, "donor": {"Product": "Original", "Slot": item.get("Slot").cloned().unwrap_or(Value::Null)},
            "original_sha256": {&name: digest}, "generated_sha256": digest, "validation": {"serialized_payloads_unchanged": true}});
        // json.dumps(report) with default separators, written in text mode.
        util::write_text(&job.join("informe.json"), &ascii(&util::dumps_compact(&report)))?;
        extra.push(job);
    }
    Ok(extra)
}

fn ascii(text: &str) -> String {
    text.chars().map(|c| if c.is_ascii() { c.to_string() } else { let mut b = [0u16; 2]; c.encode_utf16(&mut b).iter().map(|u| format!("\\u{u:04x}")).collect() }).collect()
}

fn prepare_all(root: &Path, plans: Vec<Plan>, game: &Path, version: &str, progress: &dyn Fn(Value)) -> R<Vec<PathBuf>> {
    super::require_closed()?;
    // Latest plan per category (first position, last value).
    let mut latest: Vec<Plan> = Vec::new();
    for plan in plans { match latest.iter().position(|p| p.category() == plan.category()) { Some(i) => latest[i] = plan, None => latest.push(plan) } }
    let output = root.join("paquetes");
    let mut jobs: Vec<PathBuf> = Vec::new();
    let mut packages: Vec<String> = Vec::new();
    let total = latest.len();
    for (n, plan) in latest.iter().enumerate() {
        super::require_closed()?;
        progress(json!({"phase": "prepare", "current": n + 1, "total": total, "message": format!("Preparando {}/{}", n + 1, total)}));
        let job = match plan {
            Plan::Workshop(selection) => super::jobs::prepare_workshop(root, game, selection)?,
            Plan::Item { target, donor, car, color } => super::jobs::generate_item(root, game, &output, &target["ID"], &donor["ID"], version, car, color)?,
            Plan::Texture { pack, target } => super::jobs::generate_texture(root, game, pack, &output, target.as_ref(), None, None)?,
        };
        let name = package_of(&job)?;
        if packages.contains(&name.to_lowercase()) { return Err(format!("Dos cambios usan el mismo paquete: {name}. Conserva uno en la cola.")); }
        packages.push(name.to_lowercase());
        jobs.push(job);
        let companions = match plan {
            Plan::Texture { pack, .. } if pack.category == "Boostmeter" => generate_meter_companions(root, game, pack, &output)?,
            Plan::Texture { pack, .. } if pack.category == "Pelota" => super::jobs::generate_ball_companions(root, game, pack, &output, &packages)?,
            _ => Vec::new(),
        };
        for companion in companions {
            let name = package_of(&companion)?;
            if packages.contains(&name.to_lowercase()) { return Err(format!("Dos cambios usan la biblioteca {name}")); }
            packages.push(name.to_lowercase());
            jobs.push(companion);
        }
    }
    let extra = superseded_jobs(root, &latest, &jobs, game)?;
    jobs.extend(extra);
    Ok(jobs)
}

pub fn apply(root: &Path, request: &Value, progress: &dyn Fn(Value)) -> R<Value> {
    let game = util::resolve(Path::new(request["game"].as_str().unwrap_or("")));
    if !game.join("TAGame").join("CookedPCConsole").is_dir() || !game.join("Binaries").join("Win64").join("RocketLeague.exe").is_file() {
        return Err("Selecciona la carpeta rocketleague que contiene Binaries y TAGame.".into());
    }
    let database = super::jobs::database();
    let snapshot = super::inventory::load_snapshot(&database)?;
    let owned: Vec<Value> = snapshot["inventory"].as_array().into_iter().flatten().filter(|r| r["blueprint"] != true).map(|r| r["ID"].clone()).collect();
    let active = if snapshot["active"].is_object() { snapshot["active"].clone() } else { json!({}) };
    let team = request["team"].as_str().unwrap_or("Blue");
    let team_index = if team == "Blue" { 0 } else { 1 };
    let teams = active["teams"].as_array().cloned().unwrap_or_default();
    let equipped: Vec<Value> = teams.get(team_index).map(|t| match &t["items"] { Value::Object(o) => o.values().cloned().collect(), Value::Array(a) => a.clone(), _ => Vec::new() }).unwrap_or_default();
    let body = |items: &[Value]| items.iter().find(|r| r["Slot"] == "Body").and_then(|r| r["Product"].as_str()).unwrap_or("").to_owned();
    let mut car = body(&equipped);
    // Presets use lowercase loadout keys in some save versions.
    if car.is_empty() {
        let loadout = &active[if team == "Blue" { "blue" } else { "orange" }];
        let items: Vec<Value> = match loadout { Value::Object(o) => o.get("items").and_then(Value::as_array).cloned().unwrap_or_default(), Value::Array(a) => a.clone(), _ => Vec::new() };
        car = body(&items);
    }
    let by_id = |id: &Value| database.iter().find(|r| !id.is_null() && r["ID"] == *id).cloned();
    let library = load_library(&root.join("data").join("texture_library"));
    let mut plans: Vec<Plan> = Vec::new();
    for selected in request["plans"].as_array().into_iter().flatten() {
        let kind = selected["kind"].as_str().unwrap_or("");
        if kind.starts_with("workshop") {
            if kind != "workshop" && kind != "workshop_restore" { return Err("Cambio de mapa no válido.".into()); }
            plans.push(Plan::Workshop(selected.clone()));
            continue;
        }
        let target = by_id(&selected["target_id"]);
        if let Some(t) = &target { if !owned.contains(&t["ID"]) { return Err("El objeto original ya no aparece en tu inventario.".into()); } }
        if kind == "item" {
            let donor = by_id(&selected["donor_id"]).ok_or(format!("{}", selected["donor_id"]))?;
            let target = target.ok_or("Elige el objeto original.")?;
            if car.is_empty() { return Err("No se ha identificado el coche del preset guardado. Abre el juego, equipa tu coche y vuelve a cerrar.".into()); }
            plans.push(Plan::Item { target, donor, car: car.clone(), color: selected["color"].as_str().unwrap_or("Original").to_owned() });
        } else {
            let digest = selected["digest"].as_str().unwrap_or("");
            let pack = library.iter().find(|p| p.digest == digest).cloned().ok_or(format!("'{digest}'"))?;
            if pack.category == "Decal" {
                // BodyID -1 marks a universal decal (any car).
                let template = if pack.body_id.as_i64().is_some_and(|b| b < 0) { Some(car.clone()) }
                    else if !pack.body_id.is_null() && pack.body_id != 0 { by_id(&pack.body_id).and_then(|r| r["Product"].as_str().map(str::to_owned)) }
                    else if pack.source.to_lowercase().contains("fenec") { Some("Fennec".into()) } else { Some(car.clone()) };
                if target.as_ref().is_none_or(|t| t["Slot"] != "Decal") || template.as_deref() != Some(car.as_str()) { return Err("La plantilla de decal debe corresponder al coche guardado.".into()); }
            } else if pack.category != "Boostmeter" && pack.category != "Pelota" { return Err("Tipo de textura no disponible.".into()); }
            plans.push(Plan::Texture { pack, target });
        }
    }
    let studio_settings = request.get("studio_settings").filter(|s| s.is_object()).cloned();
    let profile_settings = request.get("profile_settings").filter(|s| s.is_object()).cloned();
    if plans.is_empty() && studio_settings.is_none() && profile_settings.is_none() { return Err("La cola está vacía.".into()); }
    let mut jobs = prepare_all(root, plans, &game, request["version"].as_str().unwrap_or(""), progress)?;
    if let Some(studio) = &studio_settings {
        jobs.extend(super::studio::prepare_jobs(root, &game, studio, progress)?);
        let names: Vec<String> = jobs.iter().map(|j| package_of(j)).collect::<R<_>>()?;
        let mut unique = names.clone();
        unique.sort();
        unique.dedup();
        if unique.len() != names.len() { return Err("Dos cambios afectan al mismo paquete. Aplica uno cada vez.".into()); }
    }
    let (mut profile_jobs, prepared_profile) = match &profile_settings { Some(s) => { let (j, p) = super::profile::prepare(root, &game, s, progress)?; (j, Some(p)) } None => (Vec::new(), None) };
    let mut profile = super::profile::compose_jobs(root, &jobs, prepared_profile.as_ref())?;
    if !profile_jobs.is_empty() {
        let occupied: Vec<String> = jobs.iter().map(|j| package_of(j)).collect::<R<_>>()?;
        let mut kept = Vec::new();
        for j in profile_jobs { if !occupied.contains(&package_of(&j)?) { kept.push(j); } }
        profile_jobs = kept;
    }
    let all: Vec<PathBuf> = profile_jobs.iter().cloned().chain(jobs.iter().cloned()).collect();
    for job in &all { filename(&package_of(job)?)?; }
    let count = super::state::install_all(root, &all, &game, progress)?;
    if prepared_profile.is_some() { super::profile::commit_jobs(root, &profile_jobs, &profile)?; }
    super::profile::commit_composed(root, &mut profile)?;
    if let Some(studio) = &studio_settings { super::studio::commit_settings(root, studio)?; }
    let mut result = json!({"count": count, "state": load_state(&root.join("data"))?, "profile": profile});
    if let Some(studio) = studio_settings { result["studio"] = studio; }
    super::state::discard(root, &all);
    Ok(result)
}
