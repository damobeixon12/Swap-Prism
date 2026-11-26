//! Install/restore validated packages with persistent verified backups. Port of engine/swap_manager.py and the
//! install part of engine/change_queue.py.
use super::epic;
use super::util::{self, err, R};
use serde_json::{json, Map, Value};
use std::path::{Path, PathBuf};

pub fn sha(path: &Path) -> R<String> { util::sha256_file(path) }

pub fn load_state(folder: &Path) -> R<Map<String, Value>> {
    let path = folder.join("swaps.json");
    if !path.exists() { return Ok(Map::new()); }
    serde_json::from_slice::<Value>(&std::fs::read(&path).map_err(err)?).map_err(err)?.as_object().cloned().ok_or("swaps.json no válido.".into())
}

pub fn write_state(folder: &Path, state: &Map<String, Value>) -> R<()> {
    std::fs::create_dir_all(folder).map_err(err)?;
    let tmp = folder.join("swaps.json.tmp");
    util::write_text(&tmp, &util::dumps(&Value::Object(state.clone()), false))?;
    std::fs::rename(&tmp, folder.join("swaps.json")).map_err(err)
}

fn text<'a>(v: &'a Value, key: &str) -> &'a str { v[key].as_str().unwrap_or("") }

/// Current file hashes without rewriting backup/install metadata.
pub fn inspect_state(folder: &Path) -> R<Value> {
    let mut state = load_state(folder)?;
    let profile_path = folder.join("profile.json");
    let profile = if profile_path.exists() { serde_json::from_slice::<Value>(&std::fs::read(&profile_path).map_err(err)?).map_err(err)?["packages"].clone() } else { json!({}) };
    for (name, entry) in state.iter_mut() {
        let observed = match entry["target"].as_str().map(|t| sha(Path::new(t))) {
            Some(Ok(current)) => {
                let mut status = if entry["original_sha256"] == current.as_str() { "original" } else if entry["installed_sha256"] == current.as_str() { "installed" } else { "external" };
                let layer = &profile[name.as_str()];
                if status == "installed" && layer["installed_sha256"] == current.as_str() && layer["base_sha256"] == entry["original_sha256"] { status = "original"; }
                if status == "external" && is_epic_original(Path::new(text(entry, "target")).parent().unwrap_or(Path::new("")), name)? == Some(true) { status = "original"; }
                status
            }
            _ => "missing",
        };
        entry["observed_status"] = observed.into();
    }
    Ok(Value::Object(state))
}

/// Emergency check: heal records of packages the launcher already reset, list packages changed by others.
/// Never writes game files and never restores anything the user changed.
pub fn verify_files(game: &Path, folder: &Path) -> R<Value> {
    let cooked = game.join("TAGame").join("CookedPCConsole");
    if epic::find(game).is_none() { return Ok(json!({"epic": false, "healed": [], "modified": [], "state": inspect_state(folder)?})); }
    let mut healed = Vec::new();
    for (name, entry) in load_state(folder)? {
        if entry["status"] == "installed" && is_epic_original(&cooked, &name)? == Some(true) { rebase(&cooked, &name, folder)?; healed.push(name); }
    }
    let tracked: Vec<(String, Value)> = load_state(folder)?.into_iter().map(|(k, v)| (k.to_lowercase(), v)).collect();
    let mut files: Vec<PathBuf> = std::fs::read_dir(&cooked).map_err(err)?.flatten().map(|e| e.path()).collect();
    files.sort_by_key(|p| p.display().to_string().to_lowercase());
    let mut modified = Vec::new();
    for path in files {
        let ext = path.extension().map(|e| e.to_string_lossy().to_lowercase()).unwrap_or_default();
        if ext != "upk" && ext != "bnk" { continue; }
        let name = path.file_name().unwrap().to_string_lossy().into_owned();
        if is_epic_original(&cooked, &name)? != Some(false) { continue; }
        if let Some((_, entry)) = tracked.iter().find(|t| t.0 == name.to_lowercase()) {
            if entry["installed_sha256"] == sha(&path)?.as_str() { continue; } // our own change (a profile layer may say restored)
        }
        modified.push(name);
    }
    Ok(json!({"epic": true, "healed": healed, "modified": modified, "state": inspect_state(folder)?}))
}

pub fn filename(name: &str) -> R<&str> {
    let lower = name.to_lowercase();
    if name.contains('/') || name.contains('\\') || name.contains(':') || !(lower.ends_with(".upk") || ["SFX_Car_Movements.bnk", "SFX_Impacts.bnk"].contains(&name)) {
        return Err("Paquete no válido.".into());
    }
    Ok(name)
}

pub fn modified(name: &str) -> String {
    format!("{name} está modificado por otro programa o por una versión anterior. Usa «Verificar archivos» (Epic → Biblioteca → Rocket League → ⋯ → Gestionar → Verificar) y vuelve a aplicar.")
}

static STATE_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

/// Some(true/false) against Epic's manifest; None when there is no manifest (Steam).
pub fn is_epic_original(cooked: &Path, name: &str) -> R<Option<bool>> {
    let game = cooked.parent().and_then(Path::parent).unwrap_or(Path::new(""));
    let want = epic::expected(game, name)?;
    let target = cooked.join(name);
    match want { Some(want) if target.exists() => Ok(Some(epic::sha1(&target)? == want)), _ => Ok(None) }
}

/// The game file is Epic's original again (verify/update): make it the backup and forget the old install.
pub fn rebase(cooked: &Path, name: &str, folder: &Path) -> R<()> {
    let _guard = STATE_LOCK.lock().unwrap();
    let mut state = load_state(folder)?;
    let Some(entry) = state.get_mut(name).filter(|e| e.as_object().is_some_and(|o| !o.is_empty())) else { return Ok(()) };
    let target = cooked.join(name);
    let digest = sha(&target)?;
    let backup = PathBuf::from(text(entry, "backup"));
    if backup.exists() && sha(&backup)? == digest && entry["original_sha256"] == digest.as_str() && entry["status"] != "installed" { return Ok(()); }
    std::fs::create_dir_all(backup.parent().unwrap()).map_err(err)?;
    std::fs::copy(&target, &backup).map_err(err)?;
    if sha(&backup)? != digest { return Err(format!("No se pudo renovar la copia original de {name}")); }
    entry["original_sha256"] = digest.into();
    entry["status"] = "restored".into();
    entry["reload_status"] = "reset-by-launcher".into();
    entry.as_object_mut().unwrap().remove("profile_layer");
    write_state(folder, &state)
}

/// The unmodified source for a package: the game file when it is original, else the verified backup.
pub fn pristine(cooked: &Path, name: &str, folder: &Path) -> R<PathBuf> {
    let name = filename(name)?;
    let target = util::resolve(&cooked.join(name));
    let entry = load_state(folder)?.get(name).cloned().filter(|e| e.as_object().is_some_and(|o| !o.is_empty()));
    if let Some(entry) = &entry {
        if !util::same_path(&target, Path::new(text(entry, "target"))) { return Err("La instalación no coincide con la copia de seguridad.".into()); }
    }
    let original = is_epic_original(cooked, name)?;
    if original == Some(true) { rebase(cooked, name, folder)?; return Ok(target); }
    let Some(entry) = entry else {
        if original == Some(false) { return Err(modified(name)); }
        return Ok(target);
    };
    let backup = PathBuf::from(text(&entry, "backup"));
    if sha(&backup)? != text(&entry, "original_sha256") { return Err("La copia original ha cambiado. No se usa.".into()); }
    if original == Some(false) {
        let game = cooked.parent().and_then(Path::parent).unwrap_or(Path::new(""));
        if Some(epic::sha1(&backup)?) != epic::expected(game, name)? { return Err(modified(name)); }
    }
    let current = sha(&target)?;
    if current != text(&entry, "original_sha256") && current != text(&entry, "installed_sha256") { return Err(modified(name)); }
    Ok(backup)
}

fn is_true(v: &Value) -> bool { *v == Value::Bool(true) }

/// Only package reports with the validation their kind requires may be installed.
fn validated(report: &Value, name: &str) -> bool {
    let validation = &report["validation"];
    let hud = &validation["hud_display_color"];
    let methods: Vec<&str> = hud["methods"].as_array().map(|m| m.iter().filter_map(Value::as_str).collect()).unwrap_or_default();
    let mut method_set = methods.clone();
    method_set.sort();
    method_set.dedup();
    let verified_hud = is_true(&hud["only_display_color_references_changed"])
        || (["gfx_hud_sf.upk", "gfxshell_sf.upk", "gfx_replayhud_sf.upk", "gfx_spectatorhud_sf.upk"].contains(&name.to_lowercase().as_str())
            && is_true(&hud["only_meter_display_methods_changed"]) && is_true(&hud["label_and_number_textColor_explicit"])
            && method_set == ["onBoostChanged", "onTeamColorChanged", "updateBoost"]);
    let kind = report["kind"].as_str().unwrap_or("");
    is_true(&validation["serialized_payloads_unchanged"])
        || (kind == "texture" && is_true(&validation["other_payloads_unchanged"])
            && (is_true(&validation["texture_and_material_bindings_only"])
                || (is_true(&validation["texture_and_hud_display_only"]) && verified_hud && is_true(&hud["boost_amount_calculation_instructions_unchanged"]))
                || (name == "GFX_StartMenu_SF.upk" && is_true(&validation["only_start_menu_logo_presentation"]))))
        || (kind == "body_visual" && is_true(&validation["original_body_physics_data_unchanged"]) && is_true(&validation["only_visual_body_refs_changed"]))
        || (kind == "body_visual" && is_true(&validation["original_body_physics_fields_unchanged"]) && is_true(&validation["self_contained_visual_dependencies"]))
        || (kind == "profile" && is_true(&validation["only_client_presentation"]) && is_true(&validation["other_payloads_unchanged"]))
        || (kind == "studio" && name == "TAGame.upk" && is_true(&validation["only_presentation_color_values"]) && is_true(&validation["physics_and_functions_unchanged"]))
        || (kind == "sound" && ["SFX_Car_Movements.bnk", "SFX_Impacts.bnk"].contains(&name) && is_true(&validation["only_verified_audio_media"]))
        || (kind == "workshop" && name == "Labs_Utopia_P.upk" && is_true(&validation["workshop_map_validated"]) && is_true(&validation["fixed_utopia_retro_only"]))
}

pub fn read_report(job: &Path) -> R<Value> { serde_json::from_slice(&std::fs::read(job.join("informe.json")).map_err(err)?).map_err(err) }

/// Install one prepared job: verified original backup first, then the copy, rolled back on any failure.
pub fn apply_job(job: &Path, cooked: &Path, folder: &Path) -> R<Value> {
    super::require_closed()?;
    let cooked = util::resolve(cooked);
    let report = read_report(job)?;
    let name = filename(text(&report["target"], "AssetPackage"))?.to_owned();
    if !validated(&report, &name) { return Err("El paquete no tiene la validación requerida.".into()); }
    let staged = job.join("generado").join(&name);
    let target = util::resolve(&cooked.join(&name));
    if !target.parent().is_some_and(|p| util::same_path(p, &cooked)) { return Err("Destino fuera de la carpeta del juego.".into()); }
    if sha(&staged)? != text(&report, "generated_sha256") { return Err("El paquete generado ha cambiado.".into()); }
    let source = pristine(&cooked, &name, folder)?;
    let mut state = load_state(folder)?;
    let existing = state.get(&name).is_some_and(|e| e.as_object().is_some_and(|o| !o.is_empty()));
    let original_sha = report["original_sha256"][&name].as_str().ok_or("Informe sin SHA original.")?.to_owned();
    if sha(&source)? != original_sha { return Err("El original ya no coincide con el usado al generar.".into()); }
    std::fs::create_dir_all(folder.join("backups")).map_err(err)?;
    let backup = folder.join("backups").join(&name);
    if !existing {
        if backup.exists() && sha(&backup)? != original_sha {
            // A leftover copy from an older game version; replace it only with a verified original.
            if is_epic_original(&cooked, &name)? != Some(true) { return Err("Ya existe otra copia original distinta; no se sobrescribe.".into()); }
            std::fs::remove_file(&backup).map_err(err)?;
        }
        if !backup.exists() { std::fs::copy(&target, &backup).map_err(err)?; }
        if sha(&backup)? != original_sha { return Err("La copia de seguridad no coincide.".into()); }
    }
    let mut entry = json!({"target": target.display().to_string(), "backup": util::resolve(&backup).display().to_string(), "original_sha256": original_sha,
        "installed_sha256": report["generated_sha256"], "target_item": report["target"], "donor_item": report["donor"],
        "game_version": report.get("game_version").cloned().unwrap_or(Value::Null), "status": "pending"});
    entry["color"] = report["validation"].get("color").cloned().unwrap_or(Value::Null);
    if is_true(&report["hud_display_companion"]) { entry["hud_display_companion"] = true.into(); }
    entry["reload_status"] = "not-running".into();
    state.insert(name.clone(), entry);
    write_state(folder, &state)?;
    let mut copying = false;
    let attempt = (|| -> R<()> {
        super::require_closed()?;
        copying = true;
        std::fs::copy(&staged, &target).map_err(err)?;
        if sha(&target)? != text(&report, "generated_sha256") { return Err("La copia instalada no coincide.".into()); }
        Ok(())
    })();
    match attempt {
        Ok(()) => {
            state[&name]["status"] = if is_true(&report["restore_selection"]) { "restored" } else { "installed" }.into();
            write_state(folder, &state)?;
            Ok(state[&name].clone())
        }
        Err(e) => {
            // Roll back only after confirming we still possess a valid original.
            if copying && sha(&backup)? == original_sha {
                super::require_closed()?;
                std::fs::copy(&backup, &target).map_err(err)?;
                state[&name]["status"] = "restored-after-error".into();
                write_state(folder, &state)?;
            }
            Err(e)
        }
    }
}

pub fn restore_all(folder: &Path, names: Option<&[String]>) -> R<usize> {
    super::require_closed()?;
    let mut state = load_state(folder)?;
    if let Some(names) = names {
        for n in names { filename(n)?; }
        if !names.iter().all(|n| state.contains_key(n)) { return Err("El cambio seleccionado no tiene copia registrada.".into()); }
    }
    let mut count = 0;
    let keys: Vec<String> = state.keys().cloned().collect();
    for name in keys {
        if names.is_some_and(|n| !n.contains(&name)) { continue; }
        let entry = state[&name].clone();
        let (target, backup) = (PathBuf::from(text(&entry, "target")), PathBuf::from(text(&entry, "backup")));
        if sha(&backup)? != text(&entry, "original_sha256") { return Err(format!("Copia de seguridad no válida: {}", filename(&name)?)); }
        let current = sha(&target)?;
        if current == text(&entry, "original_sha256") {
            state[&name]["status"] = "restored".into();
            state[&name]["reload_status"] = "not-running".into();
            continue;
        }
        if current != text(&entry, "installed_sha256") {
            if is_epic_original(target.parent().unwrap_or(Path::new("")), &name)? == Some(true) {
                // Already reset by the launcher: nothing to restore.
                state[&name]["status"] = "restored".into();
                state[&name]["reload_status"] = "reset-by-launcher".into();
                continue;
            }
            return Err(modified(&name));
        }
        let game = target.parent().and_then(Path::parent).and_then(Path::parent).unwrap_or(Path::new(""));
        if let Some(want) = epic::expected(game, &name)? {
            if !want.is_empty() && epic::sha1(&backup)? != want { return Err(modified(&name)); } // never write a bad old backup as "original"
        }
        super::require_closed()?;
        std::fs::copy(&backup, &target).map_err(err)?;
        if sha(&target)? != text(&entry, "original_sha256") { return Err(format!("No se pudo verificar la restauración: {name}")); }
        state[&name]["status"] = "restored".into();
        state[&name]["reload_status"] = "not-running".into();
        count += 1;
        write_state(folder, &state)?;
    }
    write_state(folder, &state)?;
    Ok(count)
}

/// Install every prepared job as one transaction: on failure the previous files and records come back.
pub fn install_all(root: &Path, jobs: &[PathBuf], game: &Path, progress: &dyn Fn(Value)) -> R<usize> {
    super::require_closed()?;
    let folder = root.join("data");
    let state = load_state(&folder)?;
    let cooked = game.join("TAGame").join("CookedPCConsole");
    let reports: Vec<Value> = jobs.iter().map(|j| read_report(j)).collect::<R<_>>()?;
    let tmp = super::workshop::TempDir::new(&std::env::temp_dir(), "localskins-transaction-")?;
    let mut saved: Vec<(PathBuf, PathBuf, String)> = Vec::new();
    for report in &reports {
        let name = filename(text(&report["target"], "AssetPackage"))?;
        let target = cooked.join(name);
        let old = tmp.0.join(name);
        std::fs::copy(&target, &old).map_err(err)?;
        saved.push((target.clone(), old, sha(&target)?));
    }
    let mut attempted: Vec<&Value> = Vec::new();
    let result = (|| -> R<()> {
        for (job, report) in jobs.iter().zip(&reports) {
            attempted.push(report);
            progress(json!({"phase": "install", "current": attempted.len(), "total": jobs.len(), "message": format!("Aplicando {}…", text(&report["target"], "Product"))}));
            apply_job(job, &cooked, &folder)?;
        }
        Ok(())
    })();
    let Err(exc) = result else { return Ok(jobs.len()) };
    let rollback = (|| -> R<()> {
        super::require_closed()?;
        let mut current_state = load_state(&folder)?;
        for report in &attempted {
            let name = text(&report["target"], "AssetPackage");
            let (target, old, digest) = saved.iter().find(|s| s.0.file_name().is_some_and(|f| f == name)).ok_or("Falta la copia de transacción.")?;
            super::require_closed()?;
            if sha(old)? != *digest { return Err("La copia de transacción cambió.".into()); }
            let current = sha(target)?;
            if current != *digest && current != text(report, "generated_sha256") && current != report["original_sha256"][name].as_str().unwrap_or("") {
                return Err(format!("Otro programa cambió {name}; no se sobrescribe."));
            }
            if current != *digest {
                std::fs::copy(old, target).map_err(err)?;
                if sha(target)? != *digest { return Err(format!("No se verificó la reversión de {name}")); }
            }
            match state.get(name) { Some(v) => { current_state.insert(name.to_owned(), v.clone()); } None => { current_state.remove(name); } }
            write_state(&folder, &current_state)?;
        }
        write_state(&folder, &state)
    })();
    match rollback {
        Err(rollback) => Err(format!("Aplicación incompleta: {exc}. Restauración de transacción pendiente: {rollback}. Conserva data y usa Restaurar con el juego cerrado.")),
        Ok(()) => Err(format!("No se aplicó la cola; se recuperó el estado anterior. {exc}")),
    }
}

/// Discard disposable generated payloads only after a successful commit.
pub fn discard(root: &Path, jobs: &[PathBuf]) -> usize {
    let base = util::resolve(&root.join("paquetes"));
    let mut removed = 0;
    for job in jobs {
        let job = util::resolve(job);
        if !job.starts_with(&base) || job == base { continue; }
        for name in ["generado", "validacion", "AssetSwapper_Decrypted"] {
            let folder = job.join(name);
            let Ok(meta) = std::fs::symlink_metadata(&folder) else { continue };
            if !meta.is_dir() || meta.file_type().is_symlink() { continue; }
            if util::resolve(&folder).parent() != Some(job.as_path()) { continue; } // also rejects junctions
            if std::fs::remove_dir_all(&folder).is_ok() { removed += 1; } // a successful installation must stay successful
        }
    }
    removed
}

#[cfg(test)]
mod tests {
    use super::*;
    /// A fake game (no Epic manifest) with one package, and a prepared job that replaces it.
    fn setup(tag: &str) -> (PathBuf, PathBuf, PathBuf) {
        let dir = std::env::temp_dir().join(format!("sp-state-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let cooked = dir.join("game").join("TAGame").join("CookedPCConsole");
        std::fs::create_dir_all(&cooked).unwrap();
        std::fs::write(cooked.join("Test_SF.upk"), b"original").unwrap();
        let job = dir.join("root").join("paquetes").join("job1");
        std::fs::create_dir_all(job.join("generado")).unwrap();
        std::fs::write(job.join("generado").join("Test_SF.upk"), b"modified").unwrap();
        let report = json!({"kind": "item", "target": {"AssetPackage": "Test_SF.upk", "Product": "Test", "Slot": "Body"}, "donor": {"Product": "Other"},
            "original_sha256": {"Test_SF.upk": sha(&cooked.join("Test_SF.upk")).unwrap()}, "generated_sha256": sha(&job.join("generado").join("Test_SF.upk")).unwrap(),
            "validation": {"serialized_payloads_unchanged": true}});
        std::fs::write(job.join("informe.json"), report.to_string()).unwrap();
        (dir.join("game"), dir.join("root"), job)
    }
    #[test]
    fn install_inspect_restore() {
        let (game, root, job) = setup("ok");
        let cooked = game.join("TAGame").join("CookedPCConsole");
        assert_eq!(install_all(&root, &[job.clone()], &game, &|_| {}).unwrap(), 1);
        assert_eq!(std::fs::read(cooked.join("Test_SF.upk")).unwrap(), b"modified");
        let data = root.join("data");
        assert_eq!(inspect_state(&data).unwrap()["Test_SF.upk"]["observed_status"], "installed");
        assert_eq!(std::fs::read(data.join("backups").join("Test_SF.upk")).unwrap(), b"original");
        assert_eq!(restore_all(&data, None).unwrap(), 1);
        assert_eq!(std::fs::read(cooked.join("Test_SF.upk")).unwrap(), b"original");
        assert_eq!(inspect_state(&data).unwrap()["Test_SF.upk"]["observed_status"], "original");
        assert_eq!(discard(&root, &[job.clone()]), 1);
        assert!(!job.join("generado").exists());
        let _ = std::fs::remove_dir_all(game.parent().unwrap());
    }
    #[test]
    fn failed_install_rolls_back() {
        let (game, root, job) = setup("fail");
        let cooked = game.join("TAGame").join("CookedPCConsole");
        // The staged package no longer matches its report: nothing may change.
        std::fs::write(job.join("generado").join("Test_SF.upk"), b"tampered").unwrap();
        let error = install_all(&root, &[job], &game, &|_| {}).unwrap_err();
        assert!(error.contains("El paquete generado ha cambiado"), "{error}");
        assert_eq!(std::fs::read(cooked.join("Test_SF.upk")).unwrap(), b"original");
        assert!(load_state(&root.join("data")).unwrap().is_empty());
        let _ = std::fs::remove_dir_all(game.parent().unwrap());
    }
    #[test]
    fn unvalidated_report_is_refused() {
        let (game, root, job) = setup("unvalidated");
        let mut report = read_report(&job).unwrap();
        report["validation"] = json!({});
        std::fs::write(job.join("informe.json"), report.to_string()).unwrap();
        let cooked = game.join("TAGame").join("CookedPCConsole");
        assert!(apply_job(&job, &cooked, &root.join("data")).unwrap_err().contains("validación"));
        let _ = std::fs::remove_dir_all(game.parent().unwrap());
    }
    #[test]
    fn package_names_are_checked() {
        assert!(filename("Test_SF.upk").is_ok());
        assert!(filename("SFX_Impacts.bnk").is_ok());
        assert!(filename(r"..\x.upk").is_err());
        assert!(filename("other.bnk").is_err());
    }
}
