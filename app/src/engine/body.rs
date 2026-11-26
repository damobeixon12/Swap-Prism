//! Original-body visual bridge: the donor's model inside the owned body package, with the original handling (hitbox)
//! transplanted and verified. Never installs a donor body package as the target. Port of engine/body_visual.py.
use super::colors::{export_properties, properties, Prop};
use super::upk::{crypt, encode_cp1252, i32_at, i64_at, patch_generation_counts, put32, put64, summary_offsets, Package};
use super::util::R;
use serde_json::{json, Value};
use std::collections::{BTreeMap, HashMap};

pub fn import_path(pkg: &Package, mut reference: i32) -> R<String> {
    if reference > 0 { return pkg.path(reference as usize - 1); }
    let (mut parts, mut seen) = (Vec::new(), std::collections::HashSet::new());
    while reference < 0 {
        if !seen.insert(reference) { return Err("Import cíclico.".into()); }
        let entry = pkg.imports.get((-reference - 1) as usize).ok_or("Import fuera de rango.")?;
        parts.push(pkg.fname(i32_at(entry, 20), i32_at(entry, 24)));
        reference = i32_at(entry, 16);
    }
    parts.reverse();
    Ok(parts.join("."))
}

/// Add the imports (and names) the transplanted fields reference; every original export stays byte exact.
pub fn bridge_imports(pkg: Package, specs: &[(String, String)], extra_names: &[String]) -> R<(Package, HashMap<String, i32>)> {
    let mut names = pkg.names.clone();
    let mut name_bytes = Vec::new();
    let mut imports = pkg.imports.clone();
    let mut name = |text: &str, names: &mut Vec<String>, name_bytes: &mut Vec<u8>| -> R<i32> {
        let lower = text.to_lowercase();
        if let Some(i) = names.iter().position(|n| n.to_lowercase() == lower) { return Ok(i as i32); }
        let mut raw = encode_cp1252(text)?;
        raw.push(0);
        name_bytes.extend((raw.len() as i32).to_le_bytes());
        name_bytes.extend(&raw);
        name_bytes.extend(pkg.name_flags[0].to_le_bytes());
        names.push(text.to_owned());
        Ok(names.len() as i32 - 1)
    };
    for value in extra_names { name(value, &mut names, &mut name_bytes)?; }
    let mut refs = HashMap::new();
    for (path, cls) in specs {
        let parts: Vec<&str> = path.split('.').collect();
        if parts.len() < 2 { return Err("Referencia visual incompleta.".into()); }
        let mut outer = 0i32;
        for (depth, part) in parts.iter().enumerate() {
            let found = imports.iter().position(|e| i32_at(e, 16) == outer && names[i32_at(e, 20) as usize].to_lowercase() == part.to_lowercase());
            outer = match found {
                Some(ix) => -(ix as i32) - 1,
                None => {
                    let klass = if depth == parts.len() - 1 { cls.as_str() } else { "Package" };
                    let package = name(if klass != "Package" { "Engine" } else { "Core" }, &mut names, &mut name_bytes)?;
                    let class = name(klass, &mut names, &mut name_bytes)?;
                    let object = name(part, &mut names, &mut name_bytes)?;
                    imports.push([package, 0, class, 0, outer, object, 0].iter().flat_map(|v| v.to_le_bytes()).collect());
                    -(imports.len() as i32)
                }
            };
        }
        refs.insert(path.clone(), outer);
    }
    let ndelta = name_bytes.len();
    let new_imports: Vec<u8> = imports[pkg.imports.len()..].concat();
    let delta = ndelta + new_imports.len();
    if delta == 0 { return Ok((pkg, refs)); }
    let s = &pkg.summary;
    let off = s.name_offset as usize;
    let (irel, erel) = ((s.import_offset - s.name_offset) as usize, (s.export_offset - s.name_offset) as usize);
    let iend = irel + pkg.imports.len() * 28;
    if !(pkg.names_end <= irel && irel <= iend && iend <= erel) { return Err("Tablas de cabecera no compatibles con el puente visual.".into()); }
    let mut exports = pkg.exports.clone();
    for exp in exports.iter_mut() { if i64_at(exp, 36) > 0 { let v = i64_at(exp, 36) + delta as i64; put64(exp, 36, v); } }
    let mut header = pkg.header[..pkg.names_end].to_vec();
    header.extend(&name_bytes);
    header.extend(&pkg.header[pkg.names_end..iend]);
    header.extend(&new_imports);
    header.extend(&pkg.header[iend..erel]);
    for e in &exports { header.extend(e); }
    header.extend(&pkg.header[pkg.exports_end..]);
    let chunkoffset = pkg.chunks_offset as usize + delta;
    let aligned = (header.len() + 15) & !15;
    let growth = aligned as i64 - pkg.enc_aligned as i64;
    let gap = s.total_header_size as i64 - off as i64 - pkg.enc_aligned as i64;
    let garbage = pkg.garbage_size as i64;
    let extra = if growth > gap || delta as i64 > garbage { ((growth - gap).max(delta as i64 - garbage).max(0) + 255) & !15 } else { 0 };
    for ix in 0..pkg.chunks.len() {
        let pos = chunkoffset + 4 + ix * pkg.stride;
        let (a, b) = (i64_at(&header, pos) + delta as i64, i64_at(&header, pos + 12) + extra);
        put64(&mut header, pos, a);
        put64(&mut header, pos + 12, b);
    }
    header.resize(aligned, 0);
    let mut prefix = pkg.raw[..off].to_vec();
    let offsets = summary_offsets(&pkg.raw)?;
    put32(&mut prefix, offsets.total_header_size, (s.total_header_size as i64 + extra) as i32);
    put32(&mut prefix, offsets.name_count, names.len() as i32);
    put32(&mut prefix, offsets.import_count, imports.len() as i32);
    put32(&mut prefix, offsets.import_offset, s.import_offset + ndelta as i32);
    put32(&mut prefix, offsets.export_offset, s.export_offset + delta as i32);
    put32(&mut prefix, offsets.depends_offset, s.depends_offset + delta as i32);
    for at in [offsets.import_export_guids_offset, offsets.thumbnail_table_offset] {
        if i32_at(&prefix, at) > 0 { let v = i32_at(&prefix, at) + delta as i32; put32(&mut prefix, at, v); }
    }
    patch_generation_counts(&mut prefix, &offsets, s.export_count, names.len() as i32);
    put32(&mut prefix, pkg.meta_offset, (garbage + extra - delta as i64) as i32);
    put32(&mut prefix, pkg.meta_offset + 4, chunkoffset as i32);
    let oldgap = &pkg.raw[off + pkg.enc_aligned..s.total_header_size as usize];
    let newgap = (gap + extra - growth).max(0) as usize;
    let mut output = prefix;
    output.extend(crypt(&pkg.key, &header, pkg.nonce.as_deref(), false)?);
    output.extend(&oldgap[..newgap.min(oldgap.len())]);
    output.extend(std::iter::repeat_n(0u8, newgap.saturating_sub(oldgap.len())));
    output.extend(&pkg.raw[s.total_header_size as usize..]);
    let checked = Package::from_raw(&pkg.file, output)?;
    for index in 0..pkg.exports.len() {
        if pkg.payload(index)? != checked.payload(index)? { return Err("El puente alteró un export original.".into()); }
    }
    Ok((checked, refs))
}

fn physics_field(name: &str) -> bool {
    let low = name.to_lowercase();
    ["handling", "physics", "physasset", "axle", "chassisspring", "suspension", "collision", "mass", "inertia"].iter().any(|v| low.contains(v))
}

fn header_start(p: &Prop) -> usize { p.value - 24 - if p.kind == "StructProperty" || p.kind == "ByteProperty" { 8 } else { 0 } }

/// Remap only typed FNames and object references, never opaque mesh data.
fn remap_property_blob(pkg: &Package, dest: &Package, data: &[u8], field: &Prop, refs: &HashMap<String, i32>) -> R<Vec<u8>> {
    let start = header_start(field);
    let stop = field.value + field.size;
    let mut blob = data[start..stop].to_vec();
    // Later duplicates win, as in a Python dict comprehension.
    let lookup: HashMap<String, usize> = dest.names.iter().enumerate().map(|(i, n)| (n.to_lowercase(), i)).collect();
    fn walk(pkg: &Package, data: &[u8], start: usize, blob: &mut Vec<u8>, items: &[Prop], base: usize, lookup: &HashMap<String, usize>, refs: &HashMap<String, i32>) -> R<()> {
        let fname = |blob: &mut Vec<u8>, pos: usize| -> R<()> {
            let old = i32_at(blob, pos);
            if old < 0 || old as usize >= pkg.names.len() { return Err("FName físico no válido.".into()); }
            let new = *lookup.get(&pkg.names[old as usize].to_lowercase()).ok_or("FName físico no válido.")?;
            put32(blob, pos, new as i32);
            Ok(())
        };
        for p in items {
            let at = base + p.value;
            let header = at - 24 - if p.kind == "StructProperty" || p.kind == "ByteProperty" { 8 } else { 0 };
            fname(blob, header)?;
            fname(blob, header + 8)?;
            if p.kind == "StructProperty" || p.kind == "ByteProperty" { fname(blob, header + 24)?; }
            if p.kind == "NameProperty" { fname(blob, at)?; }
            else if p.kind == "ObjectProperty" || p.kind == "ComponentProperty" {
                let r = i32_at(blob, at);
                if r != 0 { let new = *refs.get(&import_path(pkg, r)?).ok_or("Referencia física sin puente.")?; put32(blob, at, new); }
            } else if p.kind == "StructProperty" {
                let pos = start + at;
                if let Some((end, parsed)) = properties(&pkg.names, &data[pos..pos + p.size], 0) {
                    if end == p.size {
                        walk(pkg, data, start, blob, &parsed, at, lookup, refs)?;
                        fname(blob, at + p.size - 8)?;
                    }
                }
            }
        }
        Ok(())
    }
    let mut shifted = field.clone();
    shifted.value -= start;
    walk(pkg, data, start, &mut blob, &[shifted], 0, &lookup, refs)?;
    Ok(blob)
}

/// Build the job's package bytes and validation for a body swap.
pub fn generate(original: &Package, other: Package, target: &Value, donor: &Value) -> R<(Vec<u8>, Value)> {
    let opos = original.find_asset(target["AssetPath"].as_str().unwrap_or(""))?;
    let dpos = other.find_asset(donor["AssetPath"].as_str().unwrap_or(""))?;
    if original.class_name(&original.exports[opos]) != "ProductAsset_Body_TA" || other.class_name(&other.exports[dpos]) != "ProductAsset_Body_TA" { return Err("Carrocería no verificada.".into()); }
    let oflat = original.flat()?;
    let (odata, oprops) = export_properties(original, oflat, &original.exports[opos]);
    let physical: Vec<Prop> = oprops.iter().filter(|p| physics_field(&p.name)).cloned().collect();
    let handling = physical.iter().find(|p| p.name == "HandlingPreset" && p.kind == "ObjectProperty").ok_or("No se ha identificado la hitbox original. Este par no se aplica.")?.clone();
    let mut specs: Vec<(String, String)> = Vec::new();
    fn collect(original: &Package, data: &[u8], props: &[Prop], specs: &mut Vec<(String, String)>) -> R<()> {
        for p in props {
            if (p.kind == "ObjectProperty" || p.kind == "ComponentProperty") && i32_at(data, p.value) != 0 {
                let r = i32_at(data, p.value);
                let path = import_path(original, r)?;
                let cls = if r > 0 { original.class_name(&original.exports[r as usize - 1]) } else { original.names[i32_at(&original.imports[(-r - 1) as usize], 8) as usize].clone() };
                if !path.starts_with("HandlingPresets.") { return Err("Los datos físicos originales dependen de otro paquete. No se instala un modelo invisible.".into()); }
                specs.push((path, cls));
            } else if p.kind == "StructProperty" {
                let sub = &data[p.value..p.value + p.size];
                if let Some((end, parsed)) = properties(&original.names, sub, 0) { if end == sub.len() { collect(original, sub, &parsed, specs)?; } }
            }
        }
        Ok(())
    }
    collect(original, odata, &physical, &mut specs)?;
    let (bridged, refs) = bridge_imports(other, &specs, &original.names)?;
    let flat = bridged.flat()?;
    let (data, props) = export_properties(&bridged, flat, &bridged.exports[dpos]);
    if !(0..64).step_by(4).any(|n| properties(&bridged.names, data, n).is_some()) || props.is_empty() { return Err("No se reconoce el objeto de carrocería.".into()); }
    let stream_start = header_start(&props[0]);
    // Remove donor physics overrides (including offsets absent from the original).
    let mut kept = Vec::new();
    let mut cursor = stream_start;
    for p in &props {
        let stop = p.value + p.size;
        if !physics_field(&p.name) { kept.extend(&data[cursor..stop]); }
        cursor = stop;
    }
    for p in &physical { kept.extend(remap_property_blob(original, &bridged, odata, p, &refs)?); }
    let mut blob = data[..stream_start].to_vec();
    blob.extend(kept);
    blob.extend(&data[cursor..]);
    let replacements = BTreeMap::from([(dpos, blob.clone())]);
    let validator = |i: usize, b: &[u8]| i == dpos && b == blob.as_slice();
    let appended = super::textures::append_exports(&bridged, &replacements, None, Some(&validator))?;
    let result = Package::from_raw(&bridged.file, appended)?;
    let (raw, _) = super::upk::rewire(&result, original, donor, target)?;
    let checked = Package::from_raw(&original.file, raw)?;
    let body = &checked.exports[checked.find_asset(target["AssetPath"].as_str().unwrap_or(""))?];
    let (cdata, cprops) = export_properties(&checked, checked.flat()?, body);
    let actual: Vec<Prop> = cprops.iter().filter(|p| physics_field(&p.name)).cloned().collect();
    if actual.iter().map(|p| &p.name).ne(physical.iter().map(|p| &p.name)) { return Err("No se conservaron todos los campos físicos originales.".into()); }
    // Reverse-normalize the typed field blobs into the original name table.
    let mut backrefs = HashMap::new();
    for p in &physical {
        let r = i32_at(odata, p.value);
        if p.kind == "ObjectProperty" && r != 0 { backrefs.insert(import_path(original, r)?, r); }
    }
    for (before, after) in physical.iter().zip(&actual) {
        let expected = remap_property_blob(original, original, odata, before, &backrefs)?;
        let restored = remap_property_blob(&checked, original, cdata, after, &backrefs)?;
        if expected != restored { return Err(format!("Un campo físico no coincide: {}", before.name)); }
    }
    let mesh = cprops.iter().find(|p| p.name == "Mesh").filter(|p| i32_at(cdata, p.value) > 0).ok_or("El modelo visual no está incorporado en su paquete.")?;
    let mesh_index = i32_at(cdata, mesh.value) as usize - 1;
    if checked.class_name(&checked.exports[mesh_index]) != "SkeletalMesh" { return Err("El modelo visual no es válido.".into()); }
    let validation = json!({"original_body_physics_fields_unchanged": true, "original_handling_preset": import_path(original, i32_at(odata, handling.value))?,
        "physics_fields": physical.iter().map(|p| p.name.clone()).collect::<Vec<_>>(), "self_contained_visual_dependencies": true, "visual_verified": false});
    Ok((checked.raw, validation))
}

/// SP_BODY_EXPECTED=<json from the Python side> cargo test -- --ignored real_bodies
#[cfg(test)]
mod parity {
    use super::*;
    use sha2::{Digest, Sha256};
    #[test]
    #[ignore]
    fn real_bodies_match_python() {
        let Some(path) = std::env::var_os("SP_BODY_EXPECTED") else { return };
        let cooked = std::path::Path::new(r"C:\Program Files\Epic Games\rocketleague\TAGame\CookedPCConsole");
        let expected: Vec<Value> = serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
        let mut failures = Vec::new();
        for want in &expected {
            let (target, donor) = (&want["target"], &want["donor"]);
            let got = (|| {
                let original = Package::open(&cooked.join(target["AssetPackage"].as_str().unwrap()))?;
                let other = Package::open(&cooked.join(donor["AssetPackage"].as_str().unwrap()))?;
                let (raw, validation) = generate(&original, other, target, donor)?;
                let flat = format!("{:x}", Sha256::digest(Package::from_raw(&original.file, raw)?.flat()?));
                Ok::<_, String>((flat, validation))
            })();
            let ok = match (&got, want.get("flat")) {
                (Ok((flat, v)), Some(f)) => flat == f.as_str().unwrap() && *v == want["validation"],
                (Err(e), None) => Some(e.as_str()) == want["error"].as_str(),
                _ => false,
            };
            if !ok { failures.push(format!("{} <- {}: rust {:?} python {}", target["Product"], donor["Product"], got.as_ref().map(|g| &g.1), want.get("error").or(want.get("validation")).unwrap())); }
        }
        println!("{} pares comparados", expected.len());
        assert!(failures.is_empty(), "{}", failures.join("\n"));
    }
}
