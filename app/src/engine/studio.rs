//! Studio: car palette and flip reset colours, Wwise event sounds and the start screen. Port of engine/studio_backend.py,
//! post_isolation.py and start_logo.py. Presentation jobs only; no runtime game hooks.
use super::util::{self, err, R};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::path::Path;

pub fn manifest() -> Value { serde_json::from_str(include_str!("../../assets/audio_manifest.json")).unwrap_or(json!({})) }

pub fn load(root: &Path) -> R<Value> {
    let path = root.join("data").join("studio.json");
    if !path.exists() { return Ok(json!({})); }
    serde_json::from_slice(&std::fs::read(&path).map_err(err)?).map_err(err)
}

fn truthy(v: &Value) -> bool {
    match v { Value::Null => false, Value::Bool(b) => *b, Value::Object(m) => !m.is_empty(), Value::Array(a) => !a.is_empty(), Value::String(s) => !s.is_empty(), Value::Number(n) => n.as_f64() != Some(0.) }
}

/// Settings whose packages were restored are switched off.
pub fn after_restore(root: &Path, names: Option<&[String]>) -> R<Value> {
    let mut settings = load(root)?;
    if !truthy(&settings) { return Ok(settings); }
    match names {
        None => settings = json!({}),
        Some(names) => {
            let has = |n: &str| names.iter().any(|x| x == n);
            if has("TAGame.upk") {
                settings["reset_enabled"] = false.into();
                if settings["palette"].is_object() { settings["palette"]["enabled"] = false.into(); }
            }
            if has("GFX_StartMenu_SF.upk") { settings["season_enabled"] = false.into(); }
            if let Some(kinds) = manifest().as_object() {
                for (kind, spec) in kinds {
                    if spec["bank"].as_str().is_some_and(has) { if let Some(sounds) = settings.get_mut("sounds").and_then(Value::as_object_mut) { sounds.remove(kind); } }
                }
            }
        }
    }
    util::write_text(&root.join("data").join("studio.json"), &util::dumps(&settings, true))?;
    Ok(settings)
}


use super::colors::{export_properties, properties, repack};
use super::state::{load_state, pristine, sha};
use super::upk::{i32_at, i64_at, Package};
use sha2::{Digest, Sha256};
use std::path::PathBuf;

/// Offsets of the selectable car colour grids in TAGame.upk: (key, first RGBA, count).
fn palette_ranges(pkg: &Package) -> R<Vec<(&'static str, usize, usize)>> {
    let flat = pkg.flat()?;
    let mut result = Vec::new();
    for (key, path, count) in [("accent", "CarColors.Accent", 105usize), ("blue", "CarColors.BlueTeamV3", 70), ("orange", "CarColors.OrangeTeamV3", 70)] {
        let index = pkg.find_asset(path)?;
        let exp = &pkg.exports[index];
        if pkg.class_name(exp) != "CarColorSet_TA" { return Err("Paleta no reconocida.".into()); }
        let (data, props) = export_properties(pkg, flat, exp);
        let field = props.iter().find(|v| v.name == "Colors" && v.kind == "ArrayProperty").filter(|f| f.size == 4 + count * 16 && i32_at(data, f.value) as usize == count);
        let field = field.ok_or("La versión del juego tiene otra paleta.")?;
        result.push((key, i64_at(exp, 36) as usize + field.value + 4, count));
    }
    Ok(result)
}

/// The flip reset effect's two RGB samples per colour table.
fn reset_ranges(pkg: &Package) -> R<Vec<usize>> {
    let flat = pkg.flat()?;
    let mut result = Vec::new();
    for (index, exp) in pkg.exports.iter().enumerate() {
        let cls = pkg.class_name(exp);
        if cls != "ParticleModuleColor" && cls != "ParticleModuleColorScaleOverLife" { continue; }
        if !pkg.path(index)?.starts_with("FX_FlipReset.") { continue; }
        let (data, props) = export_properties(pkg, flat, exp);
        for p in &props {
            if (p.name != "StartColor" && p.name != "ColorScaleOverLife") || p.st != "RawDistributionVector" { continue; }
            let arr = &data[p.value..p.value + p.size];
            let Some((_, nested)) = properties(&pkg.names, arr, 0) else { continue };
            let table = nested.iter().find(|v| v.name == "LookupTable" && v.kind == "ArrayProperty").filter(|t| t.size == 36 && i32_at(arr, t.value) == 8);
            let table = table.ok_or("Tabla de color del flip reset desconocida.")?;
            // RawDistributionVector: first two floats are the time range, then two RGB samples.
            let start = i64_at(exp, 36) as usize + p.value + table.value + 4 + 8;
            result.extend([start, start + 12]);
        }
    }
    if result.is_empty() { return Err("No se localiza el color nativo del flip reset.".into()); }
    Ok(result)
}

fn rgb(value: &Value) -> R<[f64; 3]> {
    let text = value.as_str().filter(|s| s.len() == 7 && s.starts_with('#')).ok_or("Color RGB no válido.")?;
    let part = |i: usize| u8::from_str_radix(&text[i..i + 2], 16).map(|v| v as f64 / 255.).map_err(|_| "Color RGB no válido.".to_string());
    Ok([part(1)?, part(3)?, part(5)?])
}

fn put_rgb(flat: &mut [u8], pos: usize, values: &[f64]) { for (i, v) in values.iter().enumerate() { flat[pos + i * 4..pos + i * 4 + 4].copy_from_slice(&(*v as f32).to_le_bytes()); } }

/// Car palette and flip reset colours; nothing else in TAGame.upk may change.
fn patch_appearance(pkg: &Package, settings: &Value) -> R<(Package, Value)> {
    let original = pkg.flat()?;
    let mut flat = original.to_vec();
    let mut allowed = vec![false; flat.len()];
    let mut allowed_count = 0usize;
    let mut allow = |range: std::ops::Range<usize>, allowed: &mut Vec<bool>| { for p in range { if !allowed[p] { allowed[p] = true; allowed_count += 1; } } };
    let ranges = palette_ranges(pkg)?;
    let mut changed: Vec<&str> = Vec::new();
    let palette = &settings["palette"];
    if palette["enabled"].as_bool().unwrap_or(false) {
        // Full selectable palette: preserve the game grid dimensions and slot indices.
        for (key, start, count) in ranges {
            let values = palette[key].as_array().cloned().unwrap_or_default();
            if values.len() != count { return Err(format!("La paleta {key} requiere {count} colores.")); }
            for (n, value) in values.iter().enumerate() { let pos = start + n * 16; put_rgb(&mut flat, pos, &rgb(value)?); allow(pos..pos + 12, &mut allowed); }
        }
        changed.push("Rich color palette");
    }
    if settings["reset_enabled"].as_bool().unwrap_or(false) {
        let values = rgb(&settings["reset_color"])?;
        let starts = reset_ranges(pkg)?;
        for &start in &starts { put_rgb(&mut flat, start, &values); allow(start..start + 12, &mut allowed); }
        let (lo, hi) = (values.iter().cloned().fold(f64::INFINITY, f64::min), values.iter().cloned().fold(f64::NEG_INFINITY, f64::max));
        for &start in starts.iter().step_by(2) { put_rgb(&mut flat, start - 8, &[lo, hi]); allow(start - 8..start, &mut allowed); }
        changed.push("Color del flip reset");
    }
    if original.iter().zip(&flat).enumerate().any(|(i, (a, b))| a != b && !allowed[i]) { return Err("Cambio fuera de las capas de color.".into()); }
    let packed = repack(pkg, &flat)?;
    Ok((packed, json!({"only_presentation_color_values": true, "allowed_color_bytes": allowed_count, "features": changed, "physics_and_functions_unchanged": true})))
}

/// Wwise bank sections (tag, payload); every tag appears once.
pub fn bank_chunks(raw: &[u8]) -> R<Vec<([u8; 4], Vec<u8>)>> {
    let mut pos = 0;
    let mut chunks: Vec<([u8; 4], Vec<u8>)> = Vec::new();
    while pos < raw.len() {
        if pos + 8 > raw.len() { return Err("Banco de audio truncado.".into()); }
        let tag: [u8; 4] = raw[pos..pos + 4].try_into().unwrap();
        let size = u32::from_le_bytes(raw[pos + 4..pos + 8].try_into().unwrap()) as usize;
        if pos + 8 + size > raw.len() { return Err("Banco de audio truncado.".into()); }
        if chunks.iter().any(|c| c.0 == tag) { return Err("Secciones de audio repetidas.".into()); }
        chunks.push((tag, raw[pos + 8..pos + 8 + size].to_vec()));
        pos += 8 + size;
    }
    Ok(chunks)
}

fn part<'a>(chunks: &'a [([u8; 4], Vec<u8>)], tag: &[u8; 4]) -> R<&'a Vec<u8>> { chunks.iter().find(|c| &c.0 == tag).map(|c| &c.1).ok_or(format!("Falta la sección {}.", String::from_utf8_lossy(tag))) }
fn u32_at(d: &[u8], o: usize) -> R<u32> { d.get(o..o + 4).map(|b| u32::from_le_bytes(b.try_into().unwrap())).ok_or("Audio truncado.".into()) }
fn put_u32(d: &mut [u8], o: usize, v: u32) { d[o..o + 4].copy_from_slice(&v.to_le_bytes()); }
fn index(raw: &[u8]) -> Vec<(u32, u32, u32)> { raw.chunks_exact(12).map(|c| (u32::from_le_bytes(c[0..4].try_into().unwrap()), u32::from_le_bytes(c[4..8].try_into().unwrap()), u32::from_le_bytes(c[8..12].try_into().unwrap()))).collect() }
fn join(chunks: &[([u8; 4], Vec<u8>)], updated: &[([u8; 4], Vec<u8>)]) -> Vec<u8> {
    let mut out = Vec::new();
    for (tag, data) in chunks {
        let data = updated.iter().find(|u| &u.0 == tag).map(|u| &u.1).unwrap_or(data);
        out.extend(tag);
        out.extend((data.len() as u32).to_le_bytes());
        out.extend(data);
    }
    out
}

/// Replace the event's embedded media with the prepared PCM; every other sound stays exact.
fn patch_bank(raw: &[u8], spec: &Value, pcm: &[u8]) -> R<Vec<u8>> {
    if let Some(reason) = spec["disabled_reason"].as_str().filter(|r| !r.is_empty()) { return Err(reason.into()); }
    if spec["isolation"].is_object() && !spec["isolation"].as_object().unwrap().is_empty() { return isolate(raw, spec, pcm); }
    if format!("{:x}", Sha256::digest(raw)) != spec["original_sha256"].as_str().unwrap_or("") { return Err("El banco de audio cambió con una actualización; requiere volver a identificarlo.".into()); }
    let chunks = bank_chunks(raw)?;
    let ids: Vec<u64> = spec["media"].as_array().into_iter().flatten().filter_map(|m| m["id"].as_u64()).collect();
    let mut hirc = part(&chunks, b"HIRC")?.clone();
    let (didx, data) = (part(&chunks, b"DIDX")?, part(&chunks, b"DATA")?);
    let (mut new_data, mut new_index) = (Vec::new(), Vec::new());
    for (mid, offset, size) in index(didx) {
        while new_data.len() % 16 != 0 { new_data.push(0); }
        let media: &[u8] = if ids.contains(&(mid as u64)) { pcm } else { &data[offset as usize..(offset + size) as usize] };
        new_index.extend(mid.to_le_bytes());
        new_index.extend((new_data.len() as u32).to_le_bytes());
        new_index.extend((media.len() as u32).to_le_bytes());
        new_data.extend(media);
    }
    for media in spec["media"].as_array().into_iter().flatten() {
        let params = &media["params"];
        if params["StreamType"].as_array().into_iter().flatten().any(|p| p["value"] != 0) { return Err("El sonido no es un recurso incorporado.".into()); }
        for (name, value) in [("ulPluginID", 0x10001u32), ("uInMemoryMediaSize", pcm.len() as u32)] {
            for prop in params[name].as_array().into_iter().flatten() {
                let at = prop["offset"].as_u64().unwrap_or(0) as usize;
                if u32_at(&hirc, at)? as u64 != prop["value"].as_u64().unwrap_or(u64::MAX) { return Err("Metadatos de audio incompatibles.".into()); }
                put_u32(&mut hirc, at, value);
            }
        }
    }
    let output = join(&chunks, &[(*b"DIDX", new_index), (*b"DATA", new_data), (*b"HIRC", hirc)]);
    // Every unselected media payload and every unrelated section must remain exact.
    let after = bank_chunks(&output)?;
    let (adidx, adata) = (part(&after, b"DIDX")?, part(&after, b"DATA")?);
    let new: Vec<(u32, &[u8])> = index(adidx).into_iter().map(|(m, o, s)| (m, &adata[o as usize..(o + s) as usize])).collect();
    for (mid, off, size) in index(didx) {
        if !ids.contains(&(mid as u64)) && new.iter().find(|n| n.0 == mid).map(|n| n.1) != Some(&data[off as usize..(off + size) as usize]) { return Err("Cambió un sonido no seleccionado.".into()); }
    }
    Ok(output)
}

/// Absolute alignment of every media payload in a bank.
fn validate_layout(raw: &[u8]) -> R<()> {
    let chunks = bank_chunks(raw)?;
    let header = part(&chunks, b"BKHD")?;
    if header.len() < 16 { return Err("Cabecera de banco incompleta.".into()); }
    let alignment = u32_at(header, 12)? & 0xffff;
    if alignment == 0 || alignment > 4096 || alignment & (alignment - 1) != 0 { return Err("Alineación del banco desconocida.".into()); }
    let didx = chunks.iter().find(|c| &c.0 == b"DIDX").map(|c| c.1.clone()).unwrap_or_default();
    if didx.len() % 12 != 0 { return Err("Índice de muestras truncado.".into()); }
    let entries = index(&didx);
    let ids: Vec<u32> = entries.iter().map(|e| e.0).collect();
    if ids.windows(2).any(|w| w[0] >= w[1]) { return Err("El índice de muestras no está ordenado o tiene IDs repetidos.".into()); }
    let mut pos = 0;
    while pos < raw.len() && &raw[pos..pos + 4] != b"DATA" { pos += 8 + u32_at(raw, pos + 4)? as usize; }
    if pos >= raw.len() { return Err("No hay audio DATA en el banco.".into()); }
    let (begin, size) = (pos + 8, part(&chunks, b"DATA")?.len());
    for (_, off, length) in entries {
        if (off + length) as usize > size { return Err("Muestra fuera del banco.".into()); }
        if (begin + off as usize) % alignment as usize != 0 { return Err("La dirección absoluta de audio no está alineada.".into()); }
    }
    Ok(())
}

/// Rebuild a bank keeping DATA aligned (Wwise pads the end of BKHD).
fn pack_aligned(chunks: &[([u8; 4], Vec<u8>)], updated: &[([u8; 4], Vec<u8>)]) -> R<Vec<u8>> {
    let header = part(chunks, b"BKHD")?;
    let alignment = u32_at(header, 12)? & 0xffff;
    if alignment == 0 || alignment > 4096 || alignment & (alignment - 1) != 0 { return Err("Alineación del banco desconocida.".into()); }
    let (mut before, mut header_seen) = (0usize, false);
    for (tag, payload) in chunks {
        if tag == b"DATA" { break; }
        if tag == b"BKHD" { header_seen = true; }
        before += 8 + updated.iter().find(|u| &u.0 == tag).map(|u| u.1.len()).unwrap_or(payload.len());
    }
    if !header_seen { return Err("Orden de secciones desconocido.".into()); }
    let padding = (alignment as usize - (before + 8) % alignment as usize) % alignment as usize;
    let mut all: Vec<([u8; 4], Vec<u8>)> = updated.to_vec();
    let mut bkhd = updated.iter().find(|u| &u.0 == b"BKHD").map(|u| u.1.clone()).unwrap_or(header.clone());
    bkhd.extend(std::iter::repeat_n(0u8, padding));
    all.retain(|u| &u.0 != b"BKHD");
    all.push((*b"BKHD", bkhd));
    let result = join(chunks, &all);
    validate_layout(&result)?;
    Ok(result)
}

fn hirc_objects(hirc: &[u8]) -> R<Vec<(u32, usize, Vec<u8>)>> {
    if hirc.len() < 4 { return Err("HIRC truncado.".into()); }
    let (mut pos, mut rows, mut seen) = (4usize, Vec::new(), std::collections::HashSet::new());
    for _ in 0..u32_at(hirc, 0)? {
        if pos + 9 > hirc.len() { return Err("Objeto de audio truncado.".into()); }
        let size = u32_at(hirc, pos + 1)? as usize + 5;
        if size < 9 || pos + size > hirc.len() { return Err("Tamaño de audio no válido.".into()); }
        let ident = u32_at(hirc, pos + 5)?;
        if !seen.insert(ident) { return Err("Identificador de audio repetido.".into()); }
        rows.push((ident, pos, hirc[pos..pos + size].to_vec()));
        pos += size;
    }
    if pos != hirc.len() { return Err("Datos HIRC fuera de objetos.".into()); }
    Ok(rows)
}

/// Copy the ball-contact sound subtree and route ONLY the goal-frame switches to it (post/crossbar sound).
fn isolate(raw: &[u8], spec: &Value, pcm: &[u8]) -> R<Vec<u8>> {
    let original_sha = spec["original_sha256"].as_str().unwrap_or("");
    if format!("{:x}", Sha256::digest(raw)) != original_sha { return Err("El banco original ha cambiado.".into()); }
    let route = &spec["isolation"];
    let num = |v: &Value| v.as_u64().unwrap_or(0) as u32;
    let chunks = bank_chunks(raw)?;
    let hirc = part(&chunks, b"HIRC")?;
    let old = hirc_objects(hirc)?;
    let by_id: HashMap<u32, Vec<u8>> = old.iter().map(|o| (o.0, o.2.clone())).collect();
    let mut used: std::collections::HashSet<u32> = route["reserved_ids"].as_array().into_iter().flatten().map(num).collect();
    used.extend(by_id.keys().copied());
    let mut allocate = |label: String| -> u32 {
        let digest = Sha256::digest(format!("{original_sha}{label}").as_bytes());
        let mut value = u32::from_le_bytes(digest[..4].try_into().unwrap());
        while value == 0 || used.contains(&value) { value = value.wrapping_add(1); }
        used.insert(value);
        value
    };
    let nodes: Vec<Value> = route["nodes"].as_array().cloned().unwrap_or_default();
    let mut ids: HashMap<u32, u32> = HashMap::new();
    for n in &nodes { let id = num(&n["id"]); let new = allocate(format!("post-node-{id}")); ids.insert(id, new); }
    let mut media_ids: Vec<u32> = nodes.iter().filter(|n| n.get("media").is_some()).map(|n| num(&n["media"]["sourceID"]["value"])).collect();
    media_ids.sort();
    media_ids.dedup();
    let mut new_media: HashMap<u32, u32> = HashMap::new();
    for &i in &media_ids { let new = allocate(format!("post-media-{i}")); new_media.insert(i, new); }
    validate_layout(raw)?;
    let mut data = part(&chunks, b"DATA")?.clone();
    let mut entries = index(part(&chunks, b"DIDX")?);
    let original_entries: HashMap<u32, (u32, u32)> = entries.iter().map(|e| (e.0, (e.1, e.2))).collect();
    if !media_ids.iter().all(|m| original_entries.contains_key(m)) { return Err("Falta una muestra original del poste.".into()); }
    for &ident in &media_ids {
        while data.len() % 16 != 0 { data.push(0); }
        entries.push((new_media[&ident], data.len() as u32, pcm.len() as u32));
        data.extend(pcm);
    }
    entries.sort_by_key(|e| e.0);
    let index_bytes: Vec<u8> = entries.iter().flat_map(|e| [e.0, e.1, e.2].into_iter().flat_map(u32::to_le_bytes)).collect();
    let mut clones = Vec::new();
    // Preserve the native serialized child-before-parent order, not numeric IDs.
    let mut ordered = nodes.clone();
    ordered.sort_by_key(|n| n["start"].as_u64().unwrap_or(0));
    for node in &ordered {
        let serial = by_id.get(&num(&node["id"])).ok_or("La rama de audio cambió.")?;
        if serial.len() as u64 != node["size"].as_u64().unwrap_or(0) || format!("{:x}", Sha256::digest(serial)) != node["sha256"].as_str().unwrap_or("") { return Err("La rama de audio cambió.".into()); }
        let mut new = serial.clone();
        for r in node["refs"].as_array().into_iter().flatten() {
            let at = r["offset"].as_u64().unwrap_or(0) as usize;
            if u32_at(&new, at)? != num(&r["value"]) { return Err("Referencia de audio no verificada.".into()); }
            put_u32(&mut new, at, *ids.get(&num(&r["value"])).ok_or("Referencia de audio no verificada.")?);
        }
        if let Some(params) = node.get("media") {
            if params["StreamType"]["value"] != 0 { return Err("La muestra del poste no está incorporada.".into()); }
            for (key, value) in [("sourceID", new_media[&num(&params["sourceID"]["value"])]), ("ulPluginID", 0x10001), ("uInMemoryMediaSize", pcm.len() as u32)] {
                let r = &params[key];
                let at = r["offset"].as_u64().unwrap_or(0) as usize;
                if u32_at(&new, at)? != num(&r["value"]) { return Err("Formato de muestra no verificado.".into()); }
                put_u32(&mut new, at, value);
            }
        }
        clones.push(new);
    }
    let start = route["parent_start"].as_u64().unwrap_or(0) as usize;
    let parent = by_id.get(&num(&route["parent"])).ok_or("El selector de superficies cambió.")?;
    if parent.len() as u64 != route["parent_size"].as_u64().unwrap_or(0) || format!("{:x}", Sha256::digest(parent)) != route["parent_sha256"].as_str().unwrap_or("") { return Err("El selector de superficies cambió.".into()); }
    let root_id = ids[&num(&route["root"])];
    let mut new = parent.clone();
    for r in route["post_refs"].as_array().into_iter().flatten() {
        let at = r["offset"].as_u64().unwrap_or(0) as usize - start;
        if u32_at(&new, at)? != num(&route["root"]) { return Err("El destino de poste cambió.".into()); }
        put_u32(&mut new, at, root_id);
    }
    for key in ["child_count", "param_count"] {
        let field = &route[key];
        let at = field["offset"].as_u64().unwrap_or(0) as usize - start;
        if u32_at(&new, at)? != num(&field["value"]) { return Err("El selector de sonido tiene otra estructura.".into()); }
        put_u32(&mut new, at, num(&field["value"]) + 1);
    }
    let (a, b) = (route["param_template"][0].as_u64().unwrap_or(0) as usize, route["param_template"][1].as_u64().unwrap_or(0) as usize);
    let mut param = hirc[a..b].to_vec();
    put_u32(&mut param, 0, root_id);
    if param.len() != 14 { return Err("Parámetros de cambio de sonido desconocidos.".into()); }
    let mut inserts = [(route["param_insert"].as_u64().unwrap_or(0) as usize - start, param), (route["child_insert"].as_u64().unwrap_or(0) as usize - start, root_id.to_le_bytes().to_vec())];
    inserts.sort_by(|x, y| y.0.cmp(&x.0).then(y.1.cmp(&x.1)));
    for (at, payload) in inserts { new.splice(at..at, payload); }
    let size = new.len() as u32 - 5;
    put_u32(&mut new, 1, size);
    let mut serialized: Vec<u8> = Vec::new();
    for (ident, _, serial) in &old {
        if *ident == num(&route["parent"]) { for c in &clones { serialized.extend(c); } serialized.extend(&new); } else { serialized.extend(serial); }
    }
    let mut updated_hirc = ((old.len() + clones.len()) as u32).to_le_bytes().to_vec();
    updated_hirc.extend(serialized);
    // Verify original sound objects and every non-post switch remain byte exact.
    let parsed: HashMap<u32, Vec<u8>> = hirc_objects(&updated_hirc)?.into_iter().map(|o| (o.0, o.2)).collect();
    if old.iter().any(|(i, _, s)| *i != num(&route["parent"]) && parsed.get(i) != Some(s)) { return Err("Cambió un sonido original.".into()); }
    let wanted: Vec<u64> = route["post_refs"].as_array().into_iter().flatten().filter_map(|r| r["offset"].as_u64()).collect();
    let child_insert = route["child_insert"].as_u64().unwrap_or(0);
    for switch in route["switches"].as_array().into_iter().flatten() {
        for r in switch["refs"].as_array().into_iter().flatten() {
            let offset = r["offset"].as_u64().unwrap_or(0);
            let at = offset as usize - start + if offset >= child_insert { 4 } else { 0 };
            let expected = if wanted.contains(&offset) { root_id } else { num(&r["value"]) };
            if u32_at(&new, at)? != expected { return Err("Cambió otra superficie de pelota.".into()); }
        }
    }
    let original_data = part(&chunks, b"DATA")?;
    let final_entries: HashMap<u32, (u32, u32)> = entries.iter().map(|e| (e.0, (e.1, e.2))).collect();
    if data[..original_data.len()] != original_data[..] || original_entries.iter().any(|(i, e)| final_entries.get(i) != Some(e)) { return Err("Cambió una muestra original.".into()); }
    pack_aligned(&chunks, &[(*b"HIRC", updated_hirc), (*b"DIDX", index_bytes), (*b"DATA", data)])
}

/// 16-bit mono 48 kHz WAV (Python's wave module layout).
pub fn wav(pcm: &[u8]) -> Vec<u8> {
    let mut out = b"RIFF".to_vec();
    out.extend((36 + pcm.len() as u32).to_le_bytes());
    out.extend(b"WAVEfmt ");
    out.extend(16u32.to_le_bytes());
    out.extend([1u16, 1].iter().flat_map(|v| v.to_le_bytes()));
    out.extend(48000u32.to_le_bytes());
    out.extend(96000u32.to_le_bytes());
    out.extend([2u16, 16].iter().flat_map(|v| v.to_le_bytes()));
    out.extend(b"data");
    out.extend((pcm.len() as u32).to_le_bytes());
    out.extend(pcm);
    out
}

/// PCM frames of a 16-bit mono 48 kHz WAV.
fn wav_pcm(raw: &[u8], error: &str) -> R<Vec<u8>> {
    if raw.len() < 12 || &raw[..4] != b"RIFF" || &raw[8..12] != b"WAVE" { return Err(error.into()); }
    let (mut pos, mut format, mut data) = (12usize, None, None);
    while pos + 8 <= raw.len() {
        let size = u32::from_le_bytes(raw[pos + 4..pos + 8].try_into().unwrap()) as usize;
        let body = &raw[pos + 8..(pos + 8 + size).min(raw.len())];
        match &raw[pos..pos + 4] { b"fmt " => format = Some(body.to_vec()), b"data" => data = Some(body.to_vec()), _ => {} }
        pos += 8 + size + size % 2;
    }
    let (Some(f), Some(d)) = (format, data) else { return Err(error.into()) };
    if f.len() < 16 || u16::from_le_bytes([f[2], f[3]]) != 1 || u32::from_le_bytes(f[4..8].try_into().unwrap()) != 48000 || u16::from_le_bytes([f[14], f[15]]) != 16 { return Err(error.into()); }
    Ok(d[..d.len() / 2 * 2].to_vec())
}

fn round_half_even(x: f64) -> i64 { let r = x.round(); if (x - x.trunc()).abs() == 0.5 { (2. * (x / 2.).round()) as i64 } else { r as i64 } }

fn number(v: &Value) -> Option<f64> { if v.is_boolean() { None } else { v.as_f64().filter(|x| x.is_finite()) } }

pub fn clip_wave(raw: &[u8], start: &Value, end: &Value, limit: f64) -> R<Vec<u8>> {
    let (Some(start), Some(end)) = (number(start), number(end)) else { return Err("Recorte no válido.".into()) };
    if !limit.is_finite() { return Err("Recorte no válido.".into()); }
    let pcm = wav_pcm(raw, "Audio no válido.")?;
    let frames = (pcm.len() / 2) as i64;
    let (a, b) = (round_half_even(start * 48000.), round_half_even(end * 48000.));
    if a < 0 || b <= a || b > frames + 2 || b - a > round_half_even(limit * 48000.) + 1 { return Err("El recorte supera la duración permitida.".into()); }
    let b = b.min(frames);
    Ok(wav(&pcm[(a * 2) as usize..(b * 2).max(a * 2) as usize]))
}

pub fn with_volume(raw: &[u8], volume: &Value) -> R<Vec<u8>> {
    let volume = number(volume).filter(|v| (0. ..=1.).contains(v)).ok_or("Volumen no válido.")?;
    let pcm = wav_pcm(raw, "Audio preparado no válido.")?;
    if volume == 1. { return Ok(raw.to_vec()); }
    let scaled: Vec<u8> = pcm.chunks_exact(2).flat_map(|c| (round_half_even(i16::from_le_bytes([c[0], c[1]]) as f64 * volume) as i16).to_le_bytes()).collect();
    Ok(wav(&scaled))
}

/// Wwise PCM with an explicit channel layout and sample data aligned at byte 80.
pub fn wem_pcm(raw: &[u8]) -> R<Vec<u8>> {
    let pcm = wav_pcm(raw, "Formato PCM preparado no válido.")?;
    let mut fmt = Vec::new();
    for v in [0xfffeu16, 1] { fmt.extend(v.to_le_bytes()); }
    fmt.extend(48000u32.to_le_bytes());
    fmt.extend(96000u32.to_le_bytes());
    for v in [2u16, 16, 22, 16] { fmt.extend(v.to_le_bytes()); }
    fmt.extend(4u32.to_le_bytes());
    fmt.extend([0x01, 0, 0, 0, 0, 0, 0x10, 0, 0x80, 0, 0, 0xaa, 0, 0x38, 0x9b, 0x71]);
    let chunk = |tag: &[u8], data: &[u8]| { let mut c = tag.to_vec(); c.extend((data.len() as u32).to_le_bytes()); c.extend(data); c };
    let mut payload = b"WAVE".to_vec();
    payload.extend(chunk(b"fmt ", &fmt));
    payload.extend(chunk(b"JUNK", &[0; 4]));
    payload.extend(chunk(b"data", &pcm));
    let mut result = b"RIFF".to_vec();
    result.extend((payload.len() as u32).to_le_bytes());
    result.extend(payload);
    if result.windows(4).position(|w| w == b"data").map(|p| p + 8) != Some(80) { return Err("Alineación PCM incorrecta.".into()); }
    Ok(result)
}

/// A presentation job (studio colours, sounds or the start screen).
fn make_job(root: &Path, name: &str, raw: &[u8], source: &Path, label: &str, validation: Value, kind: &str) -> R<PathBuf> {
    let job = root.join("paquetes").join(util::uuid_hex());
    std::fs::create_dir_all(job.join("generado")).map_err(err)?;
    std::fs::write(job.join("generado").join(name), raw).map_err(err)?;
    let item = json!({"Product": label, "AssetPackage": name, "Slot": "Personalización"});
    let report = json!({"kind": kind, "target": item, "donor": item, "original_sha256": {name: sha(source)?}, "generated_sha256": format!("{:x}", Sha256::digest(raw)), "validation": validation});
    util::write_text(&job.join("informe.json"), &util::dumps(&report, true))?;
    Ok(job)
}

fn flag(v: &Value) -> bool { match v { Value::Null => false, Value::Bool(b) => *b, Value::Object(o) => !o.is_empty(), Value::String(s) => !s.is_empty(), Value::Number(n) => n.as_f64() != Some(0.), Value::Array(a) => !a.is_empty() } }

pub fn prepare_jobs(root: &Path, game: &Path, settings: &Value, progress: &dyn Fn(Value)) -> R<Vec<PathBuf>> {
    let report = |n: u32, message: &str| progress(json!({"phase": "studio", "current": n, "total": 5, "message": message}));
    report(0, "Validando personalización");
    super::require_closed()?;
    let cooked = game.join("TAGame").join("CookedPCConsole");
    let data = root.join("data");
    let specs = manifest();
    let old = load(root)?;
    let mut jobs = Vec::new();
    for kind in ["reset", "post"] {
        if flag(&settings["sounds"][kind]) { if let Some(reason) = specs[kind]["disabled_reason"].as_str().filter(|r| !r.is_empty()) { return Err(reason.into()); } }
    }
    let appearance = flag(&settings["palette"]["enabled"]) || flag(&settings["reset_enabled"]);
    if appearance || flag(&old["palette"]["enabled"]) || flag(&old["reset_enabled"]) {
        report(1, "Preparando paleta y efecto de flip reset");
        let source = pristine(&cooked, "TAGame.upk", &data)?;
        let (result, validation) = patch_appearance(&Package::open(&source)?, settings)?;
        jobs.push(make_job(root, "TAGame.upk", &result.raw, &source, "Colores y flip reset", validation, "studio")?);
    }
    for kind in ["reset", "post"] {
        let (chosen, was) = (&settings["sounds"][kind], &old["sounds"][kind]);
        if !flag(chosen) && !flag(was) { continue; }
        let spec = &specs[kind];
        let name = spec["bank"].as_str().unwrap_or("");
        let source = pristine(&cooked, name, &data)?;
        let mut raw = std::fs::read(&source).map_err(err)?;
        report(2, &format!("Preparando sonido {kind}"));
        if flag(chosen) {
            let folder = util::resolve(&data.join("sounds"));
            let path = util::resolve(Path::new(chosen["path"].as_str().unwrap_or("")));
            if !path.parent().is_some_and(|p| util::same_path(p, &folder)) || sha(&path)? != chosen["sha256"].as_str().unwrap_or("") { return Err("El sonido preparado ha cambiado. Impórtalo otra vez.".into()); }
            let info: Value = serde_json::from_slice(&std::fs::read(path.with_extension("json")).map_err(err)?).map_err(err)?;
            if info["kind"] != kind { return Err("Evento de audio no válido.".into()); }
            let max = spec["max_seconds"].as_f64().unwrap_or(0.);
            let end = chosen.get("end").cloned().unwrap_or(json!(info["seconds"].as_f64().unwrap_or(0.).min(max)));
            let clip = clip_wave(&std::fs::read(&path).map_err(err)?, chosen.get("start").unwrap_or(&json!(0.)), &end, max)?;
            raw = patch_bank(&raw, spec, &wem_pcm(&with_volume(&clip, chosen.get("volume").unwrap_or(&json!(1.0)))?)?)?;
        }
        let label = format!("Sonido {}", if kind == "reset" { "flip reset" } else { "poste/travesaño" });
        jobs.push(make_job(root, name, &raw, &source, &label, json!({"only_verified_audio_media": true, "event": kind}), "sound")?);
    }
    if flag(&settings["season_enabled"]) || flag(&old["season_enabled"]) {
        report(3, "Preparando pantalla de temporada");
        let source = pristine(&cooked, "GFX_StartMenu_SF.upk", &data)?;
        let pkg = Package::open(&source)?;
        let raw = if flag(&settings["season_enabled"]) { season_logo(&pkg, settings.get("season").unwrap_or(&json!(24)))? } else { pkg.raw.clone() };
        jobs.push(make_job(root, "GFX_StartMenu_SF.upk", &raw, &source, "Inicio de temporada Swap Prism", json!({"other_payloads_unchanged": true, "only_start_menu_logo_presentation": true}), "texture")?);
    }
    Ok(jobs)
}

pub fn commit_settings(root: &Path, settings: &Value) -> R<()> {
    let path = root.join("data").join("studio.json");
    std::fs::create_dir_all(path.parent().unwrap()).map_err(err)?;
    let tmp = path.with_extension("tmp");
    util::write_text(&tmp, &util::dumps(settings, true))?;
    std::fs::rename(&tmp, &path).map_err(err)
}

pub fn apply(root: &Path, game: &Path, settings: &Value, progress: &dyn Fn(Value)) -> R<Value> {
    let jobs = prepare_jobs(root, game, settings, progress)?;
    let profile = if jobs.is_empty() { None } else { Some(super::profile::compose_jobs(root, &jobs, None)?) };
    progress(json!({"phase": "studio", "current": 4, "total": 5, "message": "Instalando cambios verificados"}));
    let count = if jobs.is_empty() { 0 } else { super::state::install_all(root, &jobs, game, progress)? };
    let mut profile = profile.unwrap_or(json!({}));
    if profile.as_object().is_some_and(|o| !o.is_empty()) { super::profile::commit_composed(root, &mut profile)?; }
    commit_settings(root, settings)?;
    progress(json!({"phase": "studio", "current": 5, "total": 5, "message": "Personalización aplicada"}));
    Ok(json!({"count": count, "state": load_state(&root.join("data"))?, "studio": settings, "profile": profile}))
}

const LOGO_PATH: &str = "UI_MainMenu_Logo.Event_Logo_RadSummer";

/// Pin only the start-screen logo to the embedded texture (projectx ImageLoader reads img://PATH).
fn patch_start_abc(data: &[u8]) -> R<(Vec<u8>, Value)> {
    use super::hud::{encode_u30, instructions, Abc, Reader};
    let abc = Abc::parse(data)?;
    let sid = abc.strings.len() as u32;
    let mut patches: Vec<(usize, usize, Vec<u8>)> = Vec::new();
    let mut changed: HashMap<u32, Vec<u8>> = HashMap::new();
    let mut hits: Vec<String> = Vec::new();
    let handlers: Vec<u32> = abc.class("menus.StartMenu").map(|c| c.traits.iter().filter(|t| t.kind == 1 && t.name.ends_with(".onDynamicLogoLoaded")).filter_map(|t| t.method).collect()).unwrap_or_default();
    for (mid, body) in &abc.bodies {
        let code = &body.code;
        let ops = instructions(code)?;
        let mut new = code.clone();
        if handlers.contains(mid) {
            // The online callback can arrive before START is active; the embedded image is ready synchronously.
            let gates: Vec<_> = ops.iter().filter(|o| o.2 == 0x12 && o.0 < 34).collect();
            if gates.len() != 2 || gates[1].1 - gates[1].0 != 4 { return Err("La condición visual del logo cambió.".into()); }
            new[gates[1].0..gates[1].1].copy_from_slice(&[0x29, 0x02, 0x02, 0x02]);
            hits.push("embedded_logo_ready".into());
        }
        for w in ops.windows(2) {
            let ((at, _, opcode, args), (_, next_end, next_opcode, next_args)) = (&w[0], &w[1]);
            if *opcode != 0x60 || abc.name(args[0]) != "tagame.api.DynamicLogos" || *next_opcode != 0x46 { continue; }
            let method = abc.name(next_args[0]);
            if next_args[1] != 0 || (method != "GetLogoImage" && method != "IsDynamicLogoLoaded") { continue; }
            let replacement = if method == "GetLogoImage" { let mut r = vec![0x2c]; r.extend(encode_u30(sid)); r } else { vec![0x26] };
            if replacement.len() > next_end - at { return Err("La instrucción de logo no cabe.".into()); }
            new[*at..*at + replacement.len()].copy_from_slice(&replacement);
            new[*at + replacement.len()..*next_end].fill(0x02);
            hits.push(method);
        }
        if new != *code { patches.push((body.codepos, body.codepos + code.len(), new.clone())); changed.insert(*mid, new); }
    }
    if !hits.iter().any(|h| h == "GetLogoImage") || !hits.iter().any(|h| h == "IsDynamicLogoLoaded") { return Err("No se reconoce el renderizador de inicio.".into()); }
    let mut pooled = encode_u30(LOGO_PATH.len() as u32);
    pooled.extend(LOGO_PATH.as_bytes());
    patches.push((abc.string_pool_end, abc.string_pool_end, pooled));
    let mut r = Reader { data, pos: abc.string_count_pos };
    r.u30()?;
    patches.push((abc.string_count_pos, r.pos, encode_u30(abc.strings.len() as u32 + 1)));
    patches.sort_by(|a, b| b.0.cmp(&a.0).then(b.1.cmp(&a.1)));
    let mut result = data.to_vec();
    for (a, b, v) in patches { result.splice(a..b, v); }
    let checked = Abc::parse(&result)?;
    if checked.classes != abc.classes || checked.methods != abc.methods || checked.strings[..checked.strings.len() - 1] != abc.strings[..] { return Err("Cambió una definición ajena al logo.".into()); }
    for (mid, body) in &abc.bodies {
        let new = checked.body(*mid).ok_or("Cambio fuera de las lecturas de logo.")?;
        if new.code != *changed.get(mid).unwrap_or(&body.code) || (new.stack, new.locals, new.initial_scope, new.max_scope, new.exceptions) != (body.stack, body.locals, body.initial_scope, body.max_scope, body.exceptions) {
            return Err("Cambio fuera de las lecturas de logo.".into());
        }
    }
    Ok((result, json!({"only_start_menu_logo_presentation": true, "methods": changed.len(), "reads": hits})))
}

pub fn patch_start_movie(movie: &[u8]) -> R<(Vec<u8>, Value)> {
    let (start, body, end, pos, _) = super::hud::abc_tag(movie)?;
    let (new, proof) = patch_start_abc(&movie[pos..end])?;
    let mut payload = movie[body..pos].to_vec();
    payload.extend(&new);
    let mut result = movie[..start].to_vec();
    result.extend(((82u16 << 6) | 63).to_le_bytes());
    result.extend((payload.len() as u32).to_le_bytes());
    result.extend(&payload);
    result.extend(&movie[end..]);
    let total = result.len() as u32;
    result[4..8].copy_from_slice(&total.to_le_bytes());
    super::hud::abc_tag(&result)?;
    Ok((result, proof))
}

/// Start screen picture: the Swap Prism logo, a caption and "SEASON n" in Segoe UI Bold.
pub fn season_picture(season: i64) -> R<Vec<u8>> {
    use ab_glyph::{Font, FontRef, PxScale, ScaleFont};
    let font_data = std::fs::read(r"C:\Windows\Fonts\segoeuib.ttf").map_err(err)?;
    let font = FontRef::try_from_slice(&font_data).map_err(err)?;
    let mut image = image::RgbaImage::new(1024, 512);
    // Keep the supplied logo intact; remove only its black matte in this composition.
    let mut logo = image::load_from_memory(include_bytes!("../../assets/logo.png")).map_err(err)?.to_rgba8();
    for p in logo.pixels_mut() { p[3] = ((p[0] as u32 * 299 + p[1] as u32 * 587 + p[2] as u32 * 114) / 1000) as u8; }
    let (lw, lh) = (logo.width() as f64, logo.height() as f64);
    let scale = (220. / lw).min(220. / lh).min(1.);
    let logo = super::dxt::resize(&logo, (lw * scale).round() as u32, (lh * scale).round() as u32);
    image::imageops::overlay(&mut image, &logo, ((1024 - logo.width()) / 2) as i64, 0);
    let blend = |img: &mut image::RgbaImage, x: i32, y: i32, color: [u8; 4], coverage: f32| {
        if x < 0 || y < 0 || x >= img.width() as i32 || y >= img.height() as i32 || coverage <= 0. { return; }
        let p = img.get_pixel_mut(x as u32, y as u32);
        let a = color[3] as f32 / 255. * coverage.min(1.);
        let out_a = a + p[3] as f32 / 255. * (1. - a);
        for i in 0..3 { p[i] = if out_a > 0. { ((color[i] as f32 * a + p[i] as f32 * p[3] as f32 / 255. * (1. - a)) / out_a).round() as u8 } else { 0 }; }
        p[3] = (out_a * 255.).round() as u8;
    };
    // Rounded panel with a 2 px outline (PIL rounded_rectangle 135,307 → 889,431, radius 22).
    let (x0, y0, x1, y1, radius) = (135f32, 307f32, 889f32, 431f32, 22f32);
    for y in 307..=431 { for x in 135..=889 {
        let (fx, fy) = (x as f32 + 0.5, y as f32 + 0.5);
        let cx = fx.clamp(x0 + radius, x1 + 1. - radius);
        let cy = fy.clamp(y0 + radius, y1 + 1. - radius);
        let d = ((fx - cx).powi(2) + (fy - cy).powi(2)).sqrt();
        if d > radius { continue; }
        let inside = d <= radius - 2. && fx >= x0 + 2. && fx <= x1 - 1. && fy >= y0 + 2. && fy <= y1 - 1.;
        blend(&mut image, x, y, if inside { [20, 25, 32, 230] } else { [0x68, 0xDA, 0xF9, 255] }, 1.);
    } }
    let mut draw_text = |text: &str, size: f32, cx: f32, cy: f32, color: [u8; 4]| {
        let scale = font.pt_to_px_scale(size).unwrap_or(PxScale::from(size));
        let scaled = font.as_scaled(scale);
        let width: f32 = text.chars().map(|c| scaled.h_advance(scaled.glyph_id(c))).sum();
        // Anchor "mm": horizontal centre, vertical middle between ascender and descender.
        let baseline = cy + (scaled.ascent() + scaled.descent()) / 2.;
        let mut x = cx - width / 2.;
        for c in text.chars() {
            let glyph = scaled.scaled_glyph(c);
            let advance = scaled.h_advance(glyph.id);
            let mut g = glyph;
            g.position = ab_glyph::point(x, baseline);
            if let Some(outline) = font.outline_glyph(g) {
                let bounds = outline.px_bounds();
                outline.draw(|gx, gy, v| blend(&mut image, bounds.min.x as i32 + gx as i32, bounds.min.y as i32 + gy as i32, color, v));
            }
            x += advance;
        }
    };
    draw_text("SWAP PRISM × ROCKET LEAGUE", 31., 512., 251., [0x68, 0xDA, 0xF9, 255]);
    draw_text(&format!("SEASON {season}"), 72., 512., 365., [255, 255, 255, 255]);
    super::dxt::png(&image)
}

/// The start screen shows the season picture instead of the online event logo.
fn season_logo(pkg: &Package, season: &Value) -> R<Vec<u8>> {
    let season = season.as_i64().filter(|s| !season.is_boolean() && (1..=999).contains(s)).ok_or("Temporada del diseño no válida.")?;
    let picture = season_picture(season)?;
    let flat = pkg.flat()?;
    let index = pkg.find_asset(LOGO_PATH)?;
    let exp = &pkg.exports[index];
    if pkg.class_name(exp) != "Texture2D" { return Err("El inicio del juego ha cambiado.".into()); }
    let serial = super::colors::export_data(flat, exp);
    let mut replacements = std::collections::BTreeMap::from([(index, super::textures::inline_serial(pkg, serial, &picture, false)?)]);
    let movie_index = pkg.find_asset("GFX_StartMenu.StartMenu")?;
    let (data, props) = export_properties(pkg, flat, &pkg.exports[movie_index]);
    let raw = props.iter().find(|v| v.name == "RawData" && v.kind == "ArrayProperty").ok_or("No se reconoce la película del inicio.")?;
    let (movie, _) = patch_start_movie(&data[raw.value + 4..raw.value + raw.size])?;
    let mut serial = data[..raw.value].to_vec();
    serial.extend((movie.len() as u32).to_le_bytes());
    serial.extend(&movie);
    serial.extend(&data[raw.value + raw.size..]);
    serial[raw.value - 8..raw.value - 4].copy_from_slice(&(movie.len() as i32 + 4).to_le_bytes());
    if let Some(crc) = props.iter().find(|v| (v.name == "RawDataCRC" || v.name == "SourceFileCRC") && v.size == 4) {
        let position = (crc.value as i64 + if crc.value > raw.value { movie.len() as i64 + 4 - raw.size as i64 } else { 0 }) as usize;
        serial[position..position + 4].copy_from_slice(&crc32fast::hash(&movie).to_le_bytes());
    }
    replacements.insert(movie_index, serial);
    let check = replacements[&movie_index].clone();
    let validator = move |i: usize, b: &[u8]| i == movie_index && b == check.as_slice();
    super::textures::append_exports(pkg, &replacements, Some(&validator), None)
}

/// MP3 → 16-bit mono 48 kHz PCM through Windows Media Foundation (the Python engine used miniaudio).
pub fn decode_mp3(path: &Path) -> R<Vec<u8>> {
    use windows::core::HSTRING;
    use windows::Win32::Media::MediaFoundation::*;
    use windows::Win32::System::Com::{CoInitializeEx, COINIT_MULTITHREADED};
    unsafe {
        let _ = CoInitializeEx(None, COINIT_MULTITHREADED);
        MFStartup(MF_VERSION, MFSTARTUP_FULL).map_err(err)?;
        let result = (|| -> windows::core::Result<Vec<u8>> {
            // The source reader inserts the decoder and the audio resampler for the requested PCM type.
            let reader = MFCreateSourceReaderFromURL(&HSTRING::from(path.as_os_str()), None)?;
            let media = MFCreateMediaType()?;
            media.SetGUID(&MF_MT_MAJOR_TYPE, &MFMediaType_Audio)?;
            media.SetGUID(&MF_MT_SUBTYPE, &MFAudioFormat_PCM)?;
            media.SetUINT32(&MF_MT_AUDIO_NUM_CHANNELS, 1)?;
            media.SetUINT32(&MF_MT_AUDIO_SAMPLES_PER_SECOND, 48000)?;
            media.SetUINT32(&MF_MT_AUDIO_BITS_PER_SAMPLE, 16)?;
            media.SetUINT32(&MF_MT_AUDIO_BLOCK_ALIGNMENT, 2)?;
            media.SetUINT32(&MF_MT_AUDIO_AVG_BYTES_PER_SECOND, 96000)?;
            let stream = MF_SOURCE_READER_FIRST_AUDIO_STREAM.0 as u32;
            reader.SetCurrentMediaType(stream, None, &media)?;
            let mut out = Vec::new();
            loop {
                let mut flags = 0u32;
                let mut sample: Option<IMFSample> = None;
                reader.ReadSample(stream, 0, None, Some(&mut flags), None, Some(&mut sample))?;
                if let Some(sample) = sample {
                    let buffer = sample.ConvertToContiguousBuffer()?;
                    let mut ptr = std::ptr::null_mut();
                    let mut len = 0u32;
                    buffer.Lock(&mut ptr, None, Some(&mut len))?;
                    out.extend_from_slice(std::slice::from_raw_parts(ptr, len as usize));
                    buffer.Unlock()?;
                }
                if flags & MF_SOURCE_READERF_ENDOFSTREAM.0 as u32 != 0 { break; }
                if out.len() > 300 * 96000 + 96000 { break; }
            }
            Ok(out)
        })();
        let _ = MFShutdown();
        result.map_err(|e| format!("No se pudo decodificar el MP3: {e}"))
    }
}

/// A MP3 for an audio event, converted and stored with its waveform peaks for the editor.
pub fn import_mp3(root: &Path, path: &Path, kind: &str) -> R<Value> {
    let specs = manifest();
    let spec = specs.get(kind).filter(|s| s["media"].as_array().is_some_and(|m| !m.is_empty())).ok_or("Evento de audio no identificado.")?;
    if path.extension().is_none_or(|e| e.to_string_lossy().to_lowercase() != "mp3") || !path.is_file() { return Err("Selecciona un archivo MP3.".into()); }
    if std::fs::metadata(path).map_err(err)?.len() > 8 * 1024 * 1024 { return Err("El MP3 supera 8 MB.".into()); }
    let head = std::fs::read(path).map_err(err)?;
    let mp3 = head.starts_with(b"ID3") || (head.len() > 1 && head[0] == 0xFF && head[1] & 0xE0 == 0xE0);
    if !mp3 { return Err("El contenido no es un MP3 válido.".into()); }
    let pcm = decode_mp3(path)?;
    let samples: Vec<i16> = pcm.chunks_exact(2).map(|c| i16::from_le_bytes([c[0], c[1]])).collect();
    if samples.is_empty() || samples.len() > 300 * 48000 { return Err("Selecciona un MP3 de hasta 5 minutos para recortarlo.".into()); }
    let raw = wav(&pcm[..samples.len() * 2]);
    let digest = format!("{:x}", Sha256::digest(&raw));
    let folder = root.join("data").join("sounds");
    std::fs::create_dir_all(&folder).map_err(err)?;
    let file = folder.join(format!("{digest}-{kind}.wav"));
    std::fs::write(&file, &raw).map_err(err)?;
    let peaks: Vec<f64> = (0..96).map(|n| {
        let segment = &samples[samples.len() * n / 96..samples.len() * (n + 1) / 96];
        segment.iter().map(|v| (*v as i32).abs()).max().unwrap_or(0) as f64 / 32768.
    }).collect();
    let duration = samples.len() as f64 / 48000.;
    let stem: String = path.file_stem().unwrap_or_default().to_string_lossy().chars().take(64).collect();
    let result = json!({"kind": kind, "name": stem, "path": file.display().to_string(), "sha256": digest, "seconds": duration, "peaks": peaks,
        "volume": 1.0, "start": 0.0, "end": duration.min(spec["max_seconds"].as_f64().unwrap_or(duration))});
    std::fs::write(file.with_extension("json"), util::dumps_compact(&result)).map_err(err)?;
    Ok(result)
}

/// Play a prepared clip (in memory, synchronous).
pub fn play_wave(raw: &[u8]) -> R<()> {
    use windows_sys::Win32::Media::Audio::{PlaySoundW, SND_MEMORY, SND_SYNC};
    let ok = unsafe { PlaySoundW(raw.as_ptr() as *const u16, std::ptr::null_mut(), SND_MEMORY | SND_SYNC) };
    if ok == 0 { Err("No se pudo reproducir el sonido.".into()) } else { Ok(()) }
}

/// Listen to an imported sound with its trim and volume.
pub fn preview(root: &Path, request: &Value) -> R<Value> {
    let path = util::resolve(Path::new(request["path"].as_str().unwrap_or("")));
    let folder = util::resolve(&root.join("data").join("sounds"));
    if !path.parent().is_some_and(|p| util::same_path(p, &folder)) || path.extension().is_none_or(|e| e != "wav") { return Err("Preescucha no válida.".into()); }
    let info: Value = serde_json::from_slice(&std::fs::read(path.with_extension("json")).map_err(err)?).map_err(err)?;
    let limit = manifest()[info["kind"].as_str().unwrap_or("")]["max_seconds"].as_f64().unwrap_or(0.);
    let end = request.get("end").cloned().unwrap_or(json!(info["seconds"].as_f64().unwrap_or(0.).min(limit)));
    let clip = clip_wave(&std::fs::read(&path).map_err(err)?, request.get("start").unwrap_or(&json!(0)), &end, limit)?;
    play_wave(&with_volume(&clip, request.get("volume").unwrap_or(&json!(1.0)))?)?;
    Ok(json!({"previewed": true}))
}

#[cfg(test)]
mod tests {
    #[test]
    fn manifest_parses() {
        let parsed: Result<serde_json::Value, _> = serde_json::from_str(include_str!("../../assets/audio_manifest.json"));
        assert!(parsed.is_ok(), "{:?}", parsed.err());
        assert!(super::manifest()["post"]["bank"].is_string());
    }
}

/// SP_STUDIO_CASES=<folder from the Python side> cargo test -- --ignored real_studio
#[cfg(test)]
mod parity {
    use super::*;
    fn h(b: &[u8]) -> String { format!("{:x}", Sha256::digest(b)) }
    #[test]
    #[ignore]
    fn real_studio_matches_python() {
        let Some(dir) = std::env::var_os("SP_STUDIO_CASES") else { return };
        let dir = PathBuf::from(dir);
        let cooked = Path::new(r"C:\Program Files\Epic Games\rocketleague\TAGame\CookedPCConsole");
        let want: Value = serde_json::from_slice(&std::fs::read(dir.join("studio.json")).unwrap()).unwrap();
        let specs = manifest();
        let tone = std::fs::read(dir.join("tone.wav")).unwrap();
        let clip = clip_wave(&tone, &json!(0.25), &json!(1.25), specs["post"]["max_seconds"].as_f64().unwrap()).unwrap();
        let vol = with_volume(&clip, &json!(0.6)).unwrap();
        let wem = wem_pcm(&vol).unwrap();
        assert_eq!(h(&clip), want["clip"].as_str().unwrap(), "recorte");
        assert_eq!(h(&vol), want["volume"].as_str().unwrap(), "volumen");
        assert_eq!(h(&wem), want["wem"].as_str().unwrap(), "formato Wwise");
        for kind in ["reset", "post"] {
            let spec = &specs[kind];
            let raw = std::fs::read(cooked.join(spec["bank"].as_str().unwrap())).unwrap();
            let got = match patch_bank(&raw, spec, &wem) { Ok(b) => h(&b), Err(e) => format!("error: {e}") };
            assert_eq!(got, want["banks"][kind].as_str().unwrap(), "banco {kind}");
        }
        let pkg = Package::open(&cooked.join("TAGame.upk")).unwrap();
        let (packed, validation) = patch_appearance(&pkg, &want["settings"]).unwrap();
        assert_eq!(h(packed.flat().unwrap()), want["appearance"]["flat"].as_str().unwrap(), "paleta");
        assert_eq!(validation, want["appearance"]["validation"], "validación de paleta");
        let start = Package::open(&cooked.join("GFX_StartMenu_SF.upk")).unwrap();
        let i = start.find_asset("GFX_StartMenu.StartMenu").unwrap();
        let (data, props) = export_properties(&start, start.flat().unwrap(), &start.exports[i]);
        let rd = props.iter().find(|v| v.name == "RawData").unwrap();
        let (movie, proof) = patch_start_movie(&data[rd.value + 4..rd.value + rd.size]).unwrap();
        assert_eq!(h(&movie), want["start"]["movie"].as_str().unwrap(), "logo de inicio");
        assert_eq!(proof, want["start"]["proof"]);
        // The season picture renders and the whole start screen package validates.
        let picture = season_picture(24).unwrap();
        std::fs::write(dir.join("season_rust.png"), &picture).unwrap();
        season_logo(&start, &json!(24)).unwrap();
        println!("estudio comparado");
    }
}

/// SP_MP3=<file> SP_MP3_OUT=<folder> cargo test -- --ignored real_mp3
#[cfg(test)]
mod mp3 {
    #[test]
    #[ignore]
    fn real_mp3_decodes() {
        let (Some(file), Some(out)) = (std::env::var_os("SP_MP3"), std::env::var_os("SP_MP3_OUT")) else { return };
        let root = std::path::PathBuf::from(out);
        let result = super::import_mp3(&root, std::path::Path::new(&file), "post").unwrap();
        println!("{} s, {} picos, wav {}", result["seconds"], result["peaks"].as_array().unwrap().len(), result["path"]);
    }
}
