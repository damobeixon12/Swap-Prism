//! Conservative baked material colours. Never interprets arbitrary vectors as RGB. Port of engine/cosmetic_colors.py.
//! UE3 property-stream research: c1yq/RL-Swapper (MIT). Only named colour parameters; effects may override these.
use super::upk::{crypt, i32_at, i64_at, put32, put64, Package, PACKAGE_FILE_TAG};
use super::util::{err, R};
use serde_json::{json, Value};

pub const PAINTS: [(&str, Option<[f64; 3]>); 14] = [
    ("Original", None), ("Blanco", Some([0.880, 0.880, 0.885])), ("Negro", Some([0.012, 0.012, 0.013])),
    ("Rojo", Some([0.680, 0.018, 0.018])), ("Azul cielo", Some([0.035, 0.450, 0.880])),
    ("Cobalto", Some([0.025, 0.100, 0.720])), ("Morado", Some([0.260, 0.022, 0.580])),
    ("Rosa", Some([0.850, 0.160, 0.380])), ("Verde bosque", Some([0.018, 0.300, 0.035])),
    ("Lima", Some([0.240, 0.820, 0.015])), ("Azafrán", Some([0.880, 0.620, 0.015])),
    ("Naranja", Some([0.880, 0.200, 0.008])), ("Siena", Some([0.180, 0.032, 0.006])), ("Gris", Some([0.190, 0.195, 0.205])),
];
const SPATIAL: [&str; 26] = ["uv", "tiling", "position", "offset", "scale", "rotation", "speed", "direction", "parallax", "bounds", "size", "coord", "width", "height", "radius", "velocity",
    "accel", "gravity", "pivot", "normal", "panning", "distort", "brightness", "intensity", "opacity", "alpha"];

pub fn paint_rgb(color: &str) -> Option<[f64; 3]> { PAINTS.iter().find(|p| p.0 == color).and_then(|p| p.1) }

pub fn color_name(name: &str) -> bool {
    let low = name.to_lowercase();
    ["color", "colour", "tint"].iter().any(|w| low.contains(w)) && !SPATIAL.iter().any(|w| low.contains(w))
}

#[derive(Clone, Debug)]
pub struct Prop { pub name: String, pub kind: String, pub st: String, pub size: usize, pub value: usize }

/// A complete bounded tagged property stream, or None.
pub fn properties(names: &[String], data: &[u8], start: usize) -> Option<(usize, Vec<Prop>)> {
    let name = |pos: usize| -> Option<&String> {
        let (idx, num) = (i32::from_le_bytes(data.get(pos..pos + 4)?.try_into().ok()?), i32::from_le_bytes(data.get(pos + 4..pos + 8)?.try_into().ok()?));
        if idx < 0 || idx as usize >= names.len() || !(0..=2048).contains(&num) { return None; }
        Some(&names[idx as usize])
    };
    let mut result = Vec::new();
    let mut pos = start;
    for _ in 0..256 {
        let key = name(pos)?;
        if key == "None" { return Some((pos + 8, result)); }
        let kind = name(pos + 8)?.clone();
        let size = i32::from_le_bytes(data.get(pos + 16..pos + 20)?.try_into().ok()?);
        let array = i32::from_le_bytes(data.get(pos + 20..pos + 24)?.try_into().ok()?);
        if i32::from_le_bytes(data.get(pos + 12..pos + 16)?.try_into().ok()?) != 0 || size < 0 || !(0..=1024).contains(&array) { return None; }
        let mut value = pos + 24;
        let mut size = size as usize;
        let mut st = String::new();
        match kind.as_str() {
            "StructProperty" => { st = name(value)?.clone(); value += 8; }
            "ByteProperty" => { name(value)?; value += 8; }
            "BoolProperty" => size = 1,
            "IntProperty" | "FloatProperty" | "NameProperty" | "ObjectProperty" | "ComponentProperty" | "InterfaceProperty" | "ArrayProperty" | "StrProperty" | "DelegateProperty" => {}
            _ => return None,
        }
        if value + size > data.len() { return None; }
        result.push(Prop { name: key.clone(), kind, st, size, value });
        pos = value + size;
    }
    None
}

pub fn export_data<'a>(flat: &'a [u8], exp: &[u8]) -> &'a [u8] {
    let (offset, size) = (i64_at(exp, 36).max(0) as usize, i32_at(exp, 32).max(0) as usize);
    &flat[offset.min(flat.len())..(offset + size).min(flat.len())]
}

/// The export's serialized data and its first parseable property stream.
pub fn export_properties<'a>(pkg: &Package, flat: &'a [u8], exp: &[u8]) -> (&'a [u8], Vec<Prop>) {
    let data = export_data(flat, exp);
    let limit = data.len().min(64);
    for start in (0..limit).step_by(4).chain((0..limit).filter(|i| i % 4 != 0)) {
        if let Some((_, props)) = properties(&pkg.names, data, start) { if !props.is_empty() { return (data, props); } }
    }
    (data, Vec::new())
}

/// Every field inside structs and arrays, with its offset relative to `data`'s origin plus `base`.
pub fn nested_fields(pkg: &Package, data: &[u8], fields: &[Prop], base: usize, depth: usize, out: &mut Vec<(Prop, usize)>) {
    if depth > 6 { return; }
    for p in fields {
        out.push((p.clone(), base + p.value));
        if p.kind == "StructProperty" {
            let sub = &data[p.value..p.value + p.size];
            if let Some((_, parsed)) = properties(&pkg.names, sub, 0) { if !parsed.is_empty() { nested_fields(pkg, sub, &parsed, base + p.value, depth + 1, out); } }
        } else if p.kind == "ArrayProperty" && p.size >= 4 {
            let arr = &data[p.value..p.value + p.size];
            let count = i32_at(arr, 0);
            if !(0..=512).contains(&count) { continue; }
            let mut cursor = 4;
            for _ in 0..count {
                let Some((next, sub)) = properties(&pkg.names, arr, cursor) else { break };
                if sub.is_empty() { break; }
                cursor = next;
                nested_fields(pkg, arr, &sub, base + p.value, depth + 1, out);
            }
        }
    }
}

/// Insertion-ordered map keyed by absolute offset (Python dict semantics: update keeps the first position).
#[derive(Default)]
pub struct Found(pub Vec<(usize, Value)>);
impl Found {
    pub fn insert(&mut self, at: usize, record: Value) { match self.0.iter_mut().find(|e| e.0 == at) { Some(e) => e.1 = record, None => self.0.push((at, record)) } }
    pub fn update(&mut self, other: Found) { for (k, v) in other.0 { self.insert(k, v); } }
}

fn rgb_at(flat: &[u8], at: usize) -> [f64; 3] {
    [0, 4, 8].map(|o| f32::from_le_bytes(flat[at + o..at + o + 4].try_into().unwrap()) as f64)
}
fn sane(rgb: [f64; 3]) -> bool { rgb.iter().all(|x| x.is_finite() && x.abs() <= 10000.) }

pub fn material_color_ranges(pkg: &Package, flat: &[u8]) -> R<Found> {
    let mut found = Found::default();
    for (index, exp) in pkg.exports.iter().enumerate() {
        if pkg.class_name(exp) != "MaterialInstanceConstant" { continue; }
        let offset = i64_at(exp, 36) as usize;
        let data = export_data(flat, exp);
        let mut parsed = None;
        for start in (0..data.len().min(64)).step_by(4) {
            parsed = properties(&pkg.names, data, start);
            if parsed.as_ref().is_some_and(|p| !p.1.is_empty()) { break; }
        }
        let Some((_, props)) = parsed else { continue };
        for prop in &props {
            if prop.name != "VectorParameterValues" || prop.kind != "ArrayProperty" || prop.size < 4 { continue; }
            let base = prop.value;
            let arr = &data[base..base + prop.size];
            let count = i32_at(arr, 0);
            if !(0..=128).contains(&count) { continue; }
            let mut cursor = 4;
            let mut candidates = Found::default();
            for _ in 0..count {
                let Some((next, fields)) = properties(&pkg.names, arr, cursor) else { candidates = Found::default(); break };
                cursor = next;
                let pn = fields.iter().find(|p| p.name == "ParameterName" && p.kind == "NameProperty" && p.size == 8);
                let pv = fields.iter().find(|p| p.name == "ParameterValue" && p.st == "LinearColor" && p.size == 16);
                let (Some(pn), Some(pv)) = (pn, pv) else { continue };
                let ni = i32_at(arr, pn.value);
                if ni < 0 || ni as usize >= pkg.names.len() || !color_name(&pkg.names[ni as usize]) { continue; }
                let absolute = offset + base + pv.value;
                if sane(rgb_at(flat, absolute)) { candidates.insert(absolute, json!({"export": pkg.path(index)?, "parameter": pkg.names[ni as usize]})); }
            }
            if cursor <= arr.len() { found.update(candidates); }
        }
    }
    Ok(found)
}

/// Particle colour lookup tables and the distributions they reference ({index: neutral multiplier}).
pub fn particle_color_ranges(pkg: &Package, flat: &[u8]) -> R<(Found, Vec<(usize, bool)>)> {
    let mut found = Found::default();
    let mut dependencies: Vec<(usize, bool)> = Vec::new();
    for (index, exp) in pkg.exports.iter().enumerate() {
        let cls = pkg.class_name(exp);
        if !cls.starts_with("ParticleModuleColor") { continue; }
        let scale = cls == "ParticleModuleColorScaleOverLife";
        let (data, props) = export_properties(pkg, flat, exp);
        let base = i64_at(exp, 36) as usize;
        for prop in &props {
            if prop.st != "RawDistributionVector" { continue; }
            let raw = &data[prop.value..prop.value + prop.size];
            let Some((_, sub)) = properties(&pkg.names, raw, 0) else { continue };
            for field in &sub {
                let pos = field.value;
                if field.name == "Distribution" && field.size == 4 && i32_at(raw, pos) > 0 {
                    let r = i32_at(raw, pos) as usize - 1;
                    match dependencies.iter_mut().find(|d| d.0 == r) { Some(d) => d.1 = d.1 && scale, None => dependencies.push((r, scale)) }
                }
                if field.name == "LookupTable" && field.kind == "ArrayProperty" && field.size >= 24 {
                    let count = i32_at(raw, pos);
                    if count >= 5 && (count - 2) % 3 == 0 && 4 + count as usize * 4 == field.size {
                        for n in 0..((count - 2) / 3) as usize {
                            let at = base + prop.value + pos + 12 + n * 12;
                            found.insert(at, json!({"export": pkg.path(index)?, "parameter": format!("{}.LookupTable", prop.name), "particle": true, "neutral_multiplier": scale, "bounds": base + prop.value + pos + 4}));
                        }
                    }
                }
            }
        }
    }
    for &(index, neutral) in &dependencies {
        let exp = &pkg.exports[index];
        if !pkg.class_name(exp).starts_with("DistributionVector") { continue; }
        let (data, props) = export_properties(pkg, flat, exp);
        let base = i64_at(exp, 36) as usize;
        for p in &props {
            if p.st == "Vector" && p.size == 12 && ["Constant", "Min", "Max", "MaxOutput"].contains(&p.name.as_str()) {
                found.insert(base + p.value, json!({"export": pkg.path(index)?, "parameter": p.name, "particle": true, "distribution": index, "neutral_multiplier": neutral}));
            }
        }
        let mut nested = Vec::new();
        nested_fields(pkg, data, &props, 0, 0, &mut nested);
        for (p, at) in nested {
            if p.st == "Vector" && p.size == 12 && p.name == "OutVal" {
                found.insert(base + at, json!({"export": pkg.path(index)?, "parameter": "ColorCurve.OutVal", "particle": true, "distribution": index, "neutral_multiplier": neutral}));
            }
        }
    }
    Ok((found, dependencies))
}

pub fn override_color_ranges(pkg: &Package, flat: &[u8]) -> R<(Found, Vec<usize>)> {
    let mut found = Found::default();
    let mut controls: Vec<usize> = Vec::new();
    for (index, exp) in pkg.exports.iter().enumerate() {
        let cls = pkg.class_name(exp);
        let is_particle = cls == "ProductOverride_ParticleSystemColorParameter_TA";
        if !is_particle && cls != "ProductOverride_MaterialParameter_TA" && cls != "ProductOverride_ParticleSystemVectorParameter_TA" { continue; }
        let (data, props) = export_properties(pkg, flat, exp);
        let base = i64_at(exp, 36) as usize;
        for p in &props {
            if p.kind != "ArrayProperty" || p.size < 4 { continue; }
            let arr = &data[p.value..p.value + p.size];
            let count = i32_at(arr, 0);
            if !(0..=512).contains(&count) { continue; }
            let mut cursor = 4;
            for _ in 0..count {
                let Some((next, fields)) = properties(&pkg.names, arr, cursor) else { break };
                if fields.is_empty() { break; }
                cursor = next;
                let key = fields.iter().find(|f| f.kind == "NameProperty" && f.size == 8 && ["PaintParameterName", "ParameterName", "Key"].contains(&f.name.as_str()))
                    .map(|f| pkg.names.get(i32_at(arr, f.value) as usize).cloned().unwrap_or_default()).unwrap_or_default();
                if !is_particle && !color_name(&key) { continue; }
                let mut edited = false;
                for f in &fields {
                    if ["CustomColor", "ParameterValue", "Value"].contains(&f.name.as_str()) && ((f.st == "LinearColor" && f.size == 16) || (f.st == "Vector" && f.size == 12)) {
                        found.insert(base + p.value + f.value, json!({"export": pkg.path(index)?, "parameter": key, "runtime_override": true, "particle": is_particle}));
                        edited = true;
                    }
                }
                if edited && is_particle {
                    for f in &fields {
                        if f.name == "Paint" && f.kind == "ObjectProperty" && f.size == 4 { let at = base + p.value + f.value; if !controls.contains(&at) { controls.push(at); } }
                    }
                }
            }
        }
    }
    Ok((found, controls))
}

pub fn color_ranges(pkg: &Package, flat: &[u8]) -> R<Found> {
    let mut found = material_color_ranges(pkg, flat)?;
    found.update(particle_color_ranges(pkg, flat)?.0);
    found.update(override_color_ranges(pkg, flat)?.0);
    // Explicitly named shader constants are also colour sources, never arbitrary vectors.
    for (index, exp) in pkg.exports.iter().enumerate() {
        let cls = pkg.class_name(exp);
        let (data, props) = export_properties(pkg, flat, exp);
        let base = i64_at(exp, 36) as usize;
        let parameter = props.iter().find(|p| p.name == "ParameterName" && p.size == 8).map(|p| pkg.names.get(i32_at(data, p.value) as usize).cloned().unwrap_or_default()).unwrap_or_default();
        for p in &props {
            let allowed = (p.name == "CarBoostGlowColor" && p.size == 16)
                || (cls == "MaterialExpressionVectorParameter" && color_name(&parameter) && p.name == "DefaultValue" && p.st == "LinearColor" && p.size == 16);
            if allowed { found.insert(base + p.value, json!({"export": pkg.path(index)?, "parameter": if parameter.is_empty() { p.name.clone() } else { parameter.clone() }})); }
        }
    }
    found.0.retain(|(at, _)| sane(rgb_at(flat, *at)));
    Ok(found)
}

/// Stop runtime particle/paint parameters from replacing baked custom RGB.
fn disable_color_overrides(pkg: &Package, before: &[u8], result: &mut [u8], dependencies: &[(usize, bool)]) -> (Vec<String>, Vec<usize>) {
    let (mut changed, mut permitted) = (Vec::new(), Vec::new());
    let none = pkg.names.iter().position(|n| n == "None");
    for (index, exp) in pkg.exports.iter().enumerate() {
        let cls = pkg.class_name(exp);
        if cls != "ProductAttribute_PaintSettings_TA" && !dependencies.iter().any(|d| d.0 == index) { continue; }
        let (data, props) = export_properties(pkg, before, exp);
        let base = i64_at(exp, 36) as usize;
        let mut nested = Vec::new();
        nested_fields(pkg, data, &props, 0, 0, &mut nested);
        let path = pkg.path(index).unwrap_or_default();
        for (p, relative) in nested {
            let at = base + relative;
            if p.kind == "NameProperty" && p.size == 8 && (p.name == "ParameterName" || p.name == "PaintParameterName") {
                if let Some(none) = none {
                    put32(result, at, none as i32);
                    put32(result, at + 4, 0);
                    permitted.extend(at..at + 8);
                    changed.push(format!("{path}.{}", p.name));
                }
            }
            if cls == "ProductAttribute_PaintSettings_TA" && p.kind == "BoolProperty" && ["bPaintParticles", "bPaintBody", "bPaintWheels", "bEnabled"].contains(&p.name.as_str()) {
                result[at] = 0;
                permitted.push(at);
                changed.push(format!("{path}.{}", p.name));
            }
        }
    }
    (changed, permitted)
}

pub fn zlib(data: &[u8], level: u32) -> R<Vec<u8>> {
    let mut encoder = flate2::write::ZlibEncoder::new(Vec::new(), flate2::Compression::new(level));
    std::io::Write::write_all(&mut encoder, data).map_err(err)?;
    encoder.finish().map_err(err)
}

/// Recompress unchanged-size chunks; preserve metadata and trailing bytes.
pub fn repack(pkg: &Package, flat: &[u8]) -> R<Package> {
    if flat.len() != pkg.flat()?.len() { return Err("Tamaño de contenido distinto.".into()); }
    let ordered: Vec<usize> = { let mut o: Vec<usize> = (0..pkg.chunks.len()).collect(); o.sort_by_key(|&i| pkg.chunks[i].coff); o };
    if ordered != (0..pkg.chunks.len()).collect::<Vec<_>>() || ordered.windows(2).any(|w| pkg.chunks[w[0]].coff + pkg.chunks[w[0]].csize as i64 > pkg.chunks[w[1]].coff) {
        return Err("Distribución de chunks no compatible con pintura.".into());
    }
    let mut header = pkg.header.clone();
    let first = pkg.chunks[0].coff as usize;
    let mut output = pkg.raw[..first].to_vec();
    let mut previous = first;
    for (index, chunk) in pkg.chunks.iter().enumerate() {
        let (uoff, usize_, coff) = (chunk.uoff as usize, chunk.usize_ as usize, chunk.coff as usize);
        output.extend(&pkg.raw[previous..coff]);
        let blocks: Vec<&[u8]> = (uoff..uoff + usize_).step_by(131072).map(|p| &flat[p..(p + 131072).min(uoff + usize_)]).collect();
        let encoded: Vec<Vec<u8>> = blocks.iter().map(|b| zlib(b, 6)).collect::<R<_>>()?;
        let mut payload = Vec::new();
        payload.extend(PACKAGE_FILE_TAG.to_le_bytes());
        payload.extend(131072i32.to_le_bytes());
        payload.extend((encoded.iter().map(Vec::len).sum::<usize>() as i32).to_le_bytes());
        payload.extend((usize_ as i32).to_le_bytes());
        for (c, b) in encoded.iter().zip(&blocks) { payload.extend((c.len() as i32).to_le_bytes()); payload.extend((b.len() as i32).to_le_bytes()); }
        for c in &encoded { payload.extend(c); }
        if pkg.nonce.is_some() { payload = crypt(&pkg.key, &payload, chunk.nonce.as_deref(), false)?; }
        let pos = pkg.chunks_offset as usize + 4 + index * pkg.stride;
        put64(&mut header, pos + 12, output.len() as i64);
        put32(&mut header, pos + 20, payload.len() as i32);
        output.extend(&payload);
        previous = coff + chunk.csize as usize;
    }
    output.extend(&pkg.raw[previous..]);
    header.resize(pkg.enc_aligned, 0);
    let off = pkg.summary.name_offset as usize;
    let sealed = crypt(&pkg.key, &header, pkg.nonce.as_deref(), false)?;
    output[off..off + pkg.enc_aligned].copy_from_slice(&sealed);
    let checked = Package::from_raw(&pkg.file, output)?;
    if checked.flat()? != flat { return Err("La verificación de color no coincide.".into()); }
    Ok(checked)
}

/// Paint a cosmetic's colour sources (material RGB, particles, overrides) found by color_ranges; diffuse textures
/// are handled by the caller. Returns the repacked package (None when nothing changed) and the colour report.
pub fn paint_rgb_sources(pkg: &Package, color: &str, ranges: &Found) -> R<(Option<Package>, Value)> {
    let Some(rgb) = paint_rgb(color) else { return Err("Color no válido.".into()) };
    let before = pkg.flat()?;
    let mut result = before.to_vec();
    let mut permitted = vec![false; before.len()];
    for (offset, record) in &ranges.0 {
        let old = rgb_at(before, *offset);
        let values = if record["neutral_multiplier"] == true {
            let v = old.iter().cloned().fold(f64::NEG_INFINITY, f64::max).max(0.);
            [v, v, v]
        } else {
            let hdr = old.iter().map(|x| x.abs()).fold(f64::NEG_INFINITY, f64::max).clamp(1., 6.);
            rgb.map(|x| x * hdr)
        };
        for (i, v) in values.iter().enumerate() { result[offset + i * 4..offset + i * 4 + 4].copy_from_slice(&(*v as f32).to_le_bytes()); }
        for p in *offset..offset + 12 { permitted[p] = true; }
    }
    // Lookup-table min/max are bounds on RGB, not time or spatial data.
    let mut bounds: Vec<usize> = Vec::new();
    for (_, r) in &ranges.0 { if let Some(b) = r["bounds"].as_u64() { if !bounds.contains(&(b as usize)) { bounds.push(b as usize); } } }
    for base in bounds {
        let values: Vec<f64> = ranges.0.iter().filter(|(_, r)| r["bounds"].as_u64() == Some(base as u64)).flat_map(|(at, _)| rgb_at(&result, *at)).collect();
        let (lo, hi) = (values.iter().cloned().fold(f64::INFINITY, f64::min), values.iter().cloned().fold(f64::NEG_INFINITY, f64::max));
        result[base..base + 4].copy_from_slice(&(lo as f32).to_le_bytes());
        result[base + 4..base + 8].copy_from_slice(&(hi as f32).to_le_bytes());
        for p in base..base + 8 { permitted[p] = true; }
    }
    let (_, dependencies) = particle_color_ranges(pkg, before)?;
    let (neutralized, controls) = disable_color_overrides(pkg, before, &mut result, &dependencies);
    for p in controls { permitted[p] = true; }
    let (_, paint_refs) = override_color_ranges(pkg, before)?;
    for &at in &paint_refs { put32(&mut result, at, 0); for p in at..at + 4 { permitted[p] = true; } }
    if before.iter().zip(&result).enumerate().any(|(i, (a, b))| a != b && !permitted[i]) { return Err("Cambio fuera de los parámetros RGB.".into()); }
    let colored = if result != before { Some(repack(pkg, &result)?) } else { None };
    let report = json!({"name": color, "rgb_parameters": ranges.0.len(), "particle_rgb_parameters": ranges.0.iter().filter(|(_, r)| r["particle"] == true).count(),
        "only_color_data_changed": true, "neutralized_color_multipliers": ranges.0.iter().filter(|(_, r)| r["neutral_multiplier"] == true).count(),
        "runtime_paint_references_cleared": paint_refs.len(), "runtime_rgb_overrides": ranges.0.iter().filter(|(_, r)| r["runtime_override"] == true).count(),
        "diffuse_textures_neutralized_or_baked": [], "runtime_overrides_neutralized": neutralized, "visual_verified": false,
        "parameters": ranges.0.iter().map(|(_, r)| r.clone()).collect::<Vec<_>>()});
    Ok((colored, report))
}

/// SP_PAINT_EXPECTED=<json from the Python side> cargo test -- --ignored real_paint
#[cfg(test)]
mod parity {
    use super::*;
    use sha2::{Digest, Sha256};
    #[test]
    #[ignore]
    fn real_paint_matches_python() {
        let Some(path) = std::env::var_os("SP_PAINT_EXPECTED") else { return };
        let expected: Vec<Value> = serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
        let mut failures = Vec::new();
        for want in &expected {
            let file = std::path::PathBuf::from(want["file"].as_str().unwrap());
            let got = (|| -> R<Value> {
                let pkg = Package::open(&file)?;
                let ranges = color_ranges(&pkg, pkg.flat()?)?;
                let records: Vec<Value> = ranges.0.iter().map(|(at, r)| json!([at, r])).collect();
                let painted = match paint_rgb_sources(&pkg, "Rojo", &ranges) {
                    Ok((colored, report)) => {
                        let flat = match &colored { Some(c) => c.flat()?.to_vec(), None => pkg.flat()?.to_vec() };
                        json!({"flat": format!("{:x}", Sha256::digest(&flat)), "report": report})
                    }
                    Err(e) => json!({"error": e}),
                };
                Ok(json!({"ranges": records, "paint": painted}))
            })();
            match got {
                Ok(g) if g["ranges"] == want["ranges"] && g["paint"] == want["paint"] => {}
                Ok(g) => failures.push(format!("{}: rangos {} pintura {} | rust {}", file.display(), g["ranges"] == want["ranges"], g["paint"] == want["paint"], g["paint"])),
                Err(e) => failures.push(format!("{}: {e}", file.display())),
            }
        }
        println!("{} paquetes comparados", expected.len());
        assert!(failures.is_empty(), "{}", failures.join("\n"));
    }
}
