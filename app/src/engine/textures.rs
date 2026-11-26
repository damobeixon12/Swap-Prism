//! Append-only Texture2D serial replacement; original non-texture exports stay byte exact. Port of
//! engine/texture_engine.py and engine/color_textures.py. UE3 inline mip research: c1yq/RL-Swapper (MIT).
use super::colors::{self, properties, Prop};
use super::dxt;
use super::upk::{crypt, i32_at, i64_at, put32, put64, Package, PACKAGE_FILE_TAG};
use super::util::{err, R};
use serde_json::{json, Value};
use std::collections::BTreeMap;
use std::path::Path;

pub fn parse_props(pkg: &Package, data: &[u8]) -> R<(usize, usize, Vec<Prop>)> {
    let limit = data.len().min(64);
    for start in (0..limit).step_by(4).chain((0..limit).filter(|i| i % 4 != 0)) {
        if let Some((end, props)) = properties(&pkg.names, data, start) { if !props.is_empty() { return Ok((start, end, props)); } }
    }
    Err("No se identifica la estructura de la textura.".into())
}

/// (table offset, end, levels: (position, flags, disk size, width, height)).
pub fn mip_table(data: &[u8], end: usize) -> R<(usize, usize, Vec<(usize, u32, i32, i32, i32)>)> {
    let get = |o: usize| data.get(o..o + 4).map(|b| i32::from_le_bytes(b.try_into().unwrap()));
    let stop = (end + 40).min(data.len().saturating_sub(32));
    let mut off = end;
    while off < stop {
        let count = get(off).unwrap_or(0);
        if (1..=16).contains(&count) {
            let walk_levels = || -> Option<(usize, Vec<(usize, u32, i32, i32, i32)>)> {
                let mut walk = off + 4;
                let mut levels = Vec::new();
                for _ in 0..count {
                    let (flags, usize_, disk) = (get(walk)? as u32, get(walk + 4)?, get(walk + 8)?);
                    if usize_ < 0 || (disk < 0 && flags & 0x20 == 0) { return None; }
                    let step = if flags & 0x21 != 0 || disk == 0 { 28 } else { 20 + disk as usize };
                    let (w, h) = (get(walk + step - 8)?, get(walk + step - 4)?);
                    if !(1..=8192).contains(&w) || !(1..=8192).contains(&h) || walk + step > data.len() { return None; }
                    levels.push((walk, flags, disk, w, h));
                    walk += step;
                }
                Some((walk, levels))
            };
            if let Some((walk, levels)) = walk_levels() { return Ok((off, walk, levels)); }
        }
        off += 4;
    }
    Err("Mips no reconocidos: no se modifica el paquete.".into())
}

fn name_index(pkg: &Package, name: &str) -> Option<usize> { pkg.names.iter().position(|n| n == name) }
fn name_of(pkg: &Package, data: &[u8], p: &Prop) -> String { pkg.names.get(i32_at(data, p.value) as usize).cloned().unwrap_or_default() }

/// A new Texture2D serial with the picture as inline mips. fit=true crops to the texture's proportions instead of stretching.
pub fn inline_serial(pkg: &Package, data: &[u8], raw: &[u8], fit: bool) -> R<Vec<u8>> {
    let (start, end, props) = parse_props(pkg, data)?;
    let (off, tail, levels) = mip_table(data, end)?;
    let fmt = props.iter().find(|p| p.name == "Format" && p.size == 8).map(|p| name_of(pkg, data, p)).unwrap_or_default();
    if !["PF_DXT1", "PF_DXT5", "PF_A8R8G8B8"].contains(&fmt.as_str()) { return Err(format!("Formato de textura no admitido: {fmt}")); }
    let none = name_index(pkg, "None").ok_or("Falta el terminador del paquete.")?;
    let image = dxt::load(raw)?;
    // Preserve existing dimensions/aspect and pixel format, including normal-map encoding.
    let sx = props.iter().find(|p| p.name == "SizeX").map(|p| i32_at(data, p.value)).unwrap_or(levels[0].3);
    let sy = props.iter().find(|p| p.name == "SizeY").map(|p| i32_at(data, p.value)).unwrap_or(levels[0].4);
    if !(1..=4096).contains(&sx) || !(1..=4096).contains(&sy) { return Err("Resolución no admitida.".into()); }
    let (sx, sy) = (sx as u32, sy as u32);
    let base = if fit { dxt::fit(&image, sx, sy) } else { dxt::resize(&image, sx, sy) };
    let mut header = data[..start].to_vec();
    let mut previous = start;
    let mut never = false;
    for p in &props {
        let stop = p.value + p.size;
        let mut part = data[previous..stop].to_vec();
        let rel = p.value - previous;
        previous = stop;
        if p.name == "TextureFileCacheName" || p.name == "FirstResourceMemMip" { continue; }
        if p.name == "NeverStream" { part[rel] = 1; never = true; }
        if p.name == "LODBias" && p.size == 4 { put32(&mut part, rel, 0); }
        header.extend(part);
    }
    if let (false, Some(ns), Some(bp)) = (never, name_index(pkg, "NeverStream"), name_index(pkg, "BoolProperty")) {
        for v in [ns as i32, 0, bp as i32, 0, 0, 0] { header.extend(v.to_le_bytes()); }
        header.push(1);
    }
    header.extend((none as i32).to_le_bytes());
    header.extend(0i32.to_le_bytes());
    header.extend(&data[end..off]);
    let no_mips = props.iter().any(|p| p.name == "MipGenSettings" && p.size == 8 && name_of(pkg, data, p) == "TMGS_NoMipmaps");
    let mut chunks: Vec<Vec<u8>> = Vec::new();
    let (mut w, mut h) = (sx, sy);
    loop {
        let level = dxt::resize(&base, w, h);
        let (pixels, mw, mh) = if fmt == "PF_A8R8G8B8" {
            (level.pixels().flat_map(|p| [p[2], p[1], p[0], p[3]]).collect::<Vec<u8>>(), w, h)
        } else {
            let (mw, mh) = (w.max(4), h.max(4));
            (dxt::encode(&dxt::resize(&level, mw, mh), &fmt[3..]), mw, mh)
        };
        let mut chunk = 0x10000u32.to_le_bytes().to_vec();
        chunk.extend((pixels.len() as i32).to_le_bytes());
        chunk.extend((pixels.len() as i32).to_le_bytes());
        chunk.extend(&pixels);
        chunk.extend((mw as i32).to_le_bytes());
        chunk.extend((mh as i32).to_le_bytes());
        chunks.push(chunk);
        if no_mips || (w == 1 && h == 1) { break; }
        w = (w / 2).max(1);
        h = (h / 2).max(1);
    }
    let mut out = header;
    out.extend((chunks.len() as i32).to_le_bytes());
    for c in chunks { out.extend(c); }
    out.extend(&data[tail..]);
    Ok(out)
}

pub type Validator<'a> = Option<&'a dyn Fn(usize, &[u8]) -> bool>;

/// New serials appended after the package's last byte; every other object keeps its address and bytes.
pub fn append_exports(pkg: &Package, replacements: &BTreeMap<usize, Vec<u8>>, swf_validator: Validator, body_validator: Validator) -> R<Vec<u8>> {
    let before = pkg.flat()?;
    let mut flat = before.to_vec();
    let mut header = pkg.header.clone();
    let mut export_cursor = (pkg.summary.export_offset - pkg.summary.name_offset) as usize;
    for (index, exp) in pkg.exports.iter().enumerate() {
        if let Some(blob) = replacements.get(&index) {
            put32(&mut header, export_cursor + 32, blob.len() as i32);
            put64(&mut header, export_cursor + 36, flat.len() as i64);
            flat.extend(blob);
        }
        export_cursor += exp.len();
    }
    // Only extend the last virtual chunk, preserving every original object address.
    let virtual_last = (0..pkg.chunks.len()).max_by_key(|&i| pkg.chunks[i].uoff).ok_or("Paquete sin chunks.")?;
    let last = &pkg.chunks[virtual_last];
    if (last.uoff + last.usize_ as i64) as usize != before.len() { return Err("Cola de chunks no compatible.".into()); }
    let growth = flat.len() - before.len();
    let pos = pkg.chunks_offset as usize + 4 + virtual_last * pkg.stride;
    put32(&mut header, pos + 8, last.usize_ + growth as i32);
    let mut ordered: Vec<usize> = (0..pkg.chunks.len()).collect();
    ordered.sort_by_key(|&i| pkg.chunks[i].coff);
    let first = pkg.chunks[ordered[0]].coff as usize;
    let mut out = pkg.raw[..first].to_vec();
    let mut previous = first;
    for &index in &ordered {
        let chunk = &pkg.chunks[index];
        let (uoff, coff, csize) = (chunk.uoff as usize, chunk.coff as usize, chunk.csize as usize);
        let usize_ = chunk.usize_ as usize + if index == virtual_last { growth } else { 0 };
        if coff < previous { return Err("Chunks solapados.".into()); }
        out.extend(&pkg.raw[previous..coff]);
        let payload = if index != virtual_last { pkg.raw[coff..coff + csize].to_vec() } else {
            let blocks: Vec<&[u8]> = (uoff..uoff + usize_).step_by(131072).map(|p| &flat[p..(p + 131072).min(uoff + usize_)]).collect();
            // Old compressed blocks are reused when their content did not change.
            let reusable: Vec<(Vec<u8>, usize)> = (|| -> R<Vec<(Vec<u8>, usize)>> {
                let mut old = pkg.raw[coff..coff + csize].to_vec();
                if pkg.nonce.is_some() { old = crypt(&pkg.key, &old, chunk.nonce.as_deref(), true)?; }
                let total = i32_at(&old, 12) as i64;
                let (mut cursor, mut amount, mut entries) = (16usize, 0i64, Vec::new());
                while amount < total {
                    let (c, u) = (i32_at(&old, cursor), i32_at(&old, cursor + 4));
                    cursor += 8;
                    if c <= 0 || u <= 0 { return Err("Bloque inválido".into()); }
                    entries.push((c as usize, u as usize));
                    amount += u as i64;
                }
                Ok(entries.into_iter().map(|(c, u)| { let block = old[cursor..cursor + c].to_vec(); cursor += c; (block, u) }).collect())
            })().unwrap_or_default();
            let mut encoded = Vec::new();
            for (n, block) in blocks.iter().enumerate() {
                let reuse = reusable.get(n).filter(|c| c.1 == block.len()).filter(|c| {
                    let mut plain = Vec::new();
                    std::io::Read::read_to_end(&mut flate2::read::ZlibDecoder::new(&c.0[..]), &mut plain).is_ok() && plain == *block
                });
                encoded.push(match reuse { Some(c) => c.0.clone(), None => colors::zlib(block, 3)? });
            }
            let mut payload = PACKAGE_FILE_TAG.to_le_bytes().to_vec();
            payload.extend(131072i32.to_le_bytes());
            payload.extend((encoded.iter().map(Vec::len).sum::<usize>() as i32).to_le_bytes());
            payload.extend((usize_ as i32).to_le_bytes());
            for (c, b) in encoded.iter().zip(&blocks) { payload.extend((c.len() as i32).to_le_bytes()); payload.extend((b.len() as i32).to_le_bytes()); }
            for c in &encoded { payload.extend(c); }
            if pkg.nonce.is_some() { payload = crypt(&pkg.key, &payload, chunk.nonce.as_deref(), false)?; }
            payload
        };
        let pos = pkg.chunks_offset as usize + 4 + index * pkg.stride;
        put64(&mut header, pos + 12, out.len() as i64);
        put32(&mut header, pos + 20, payload.len() as i32);
        out.extend(&payload);
        previous = coff + csize;
    }
    out.extend(&pkg.raw[previous..]);
    header.resize(pkg.enc_aligned, 0);
    let off = pkg.summary.name_offset as usize;
    let sealed = crypt(&pkg.key, &header, pkg.nonce.as_deref(), false)?;
    out[off..off + pkg.enc_aligned].copy_from_slice(&sealed);
    let checked = Package::from_raw(&pkg.file, out)?;
    let after = checked.flat()?;
    for (index, (old, new)) in pkg.exports.iter().zip(&checked.exports).enumerate() {
        let expected: &[u8] = match replacements.get(&index) { Some(r) => r, None => colors::export_data(before, old) };
        if colors::export_data(after, new) != expected { return Err("Falló la validación de exports.".into()); }
        if replacements.contains_key(&index) {
            let cls = pkg.class_name(old);
            if cls == "Texture2D" || cls == "MaterialInstanceConstant" { continue; }
            if cls == "ProductAsset_Body_TA" && body_validator.is_some_and(|v| v(index, expected)) { continue; }
            let hud = pkg.file.file_name().is_some_and(|f| f.to_string_lossy().to_lowercase() == "gfx_hud_sf.upk") && pkg.path(index)? == "GFX_Hud.HUD";
            if !(cls == "SwfMovie" && (hud || swf_validator.is_some_and(|v| v(index, expected)))) {
                return Err("Solo se admiten texturas, sus materiales y el color del boostmeter verificado.".into());
            }
        }
    }
    Ok(checked.raw)
}

/// Material parameter name -> exact texture exports. No guessing texture suffixes.
pub fn bindings(pkg: &Package, flat: &[u8]) -> Vec<(String, Vec<usize>)> {
    let mut result: Vec<(String, Vec<usize>)> = Vec::new();
    for exp in &pkg.exports {
        if pkg.class_name(exp) != "MaterialInstanceConstant" { continue; }
        let data = colors::export_data(flat, exp);
        let Ok((_, _, props)) = parse_props(pkg, data) else { continue };
        for p in &props {
            if p.name != "TextureParameterValues" || p.kind != "ArrayProperty" { continue; }
            let arr = &data[p.value..p.value + p.size];
            let count = i32_at(arr, 0);
            if !(0..=128).contains(&count) { continue; }
            let mut cursor = 4;
            for _ in 0..count {
                let Some((next, fields)) = properties(&pkg.names, arr, cursor) else { break };
                cursor = next;
                let n = fields.iter().find(|f| f.name == "ParameterName" && f.size == 8);
                let v = fields.iter().find(|f| f.name == "ParameterValue" && f.size == 4);
                if let (Some(n), Some(v)) = (n, v) {
                    let (ni, idx) = (i32_at(arr, n.value), i32_at(arr, v.value) - 1);
                    if ni >= 0 && (ni as usize) < pkg.names.len() && idx >= 0 && (idx as usize) < pkg.exports.len() && pkg.class_name(&pkg.exports[idx as usize]) == "Texture2D" {
                        let name = pkg.names[ni as usize].clone();
                        let slot = match result.iter().position(|r| r.0 == name) { Some(i) => i, None => { result.push((name, Vec::new())); result.len() - 1 } };
                        if !result[slot].1.contains(&(idx as usize)) { result[slot].1.push(idx as usize); result[slot].1.sort(); }
                    }
                }
            }
        }
    }
    result
}

/// Add only an existing shader mask parameter, reusing the pack's local blank texture.
fn add_mask_binding(pkg: &Package, flat: &[u8], raw: &[u8], replacements: &mut BTreeMap<usize, Vec<u8>>) -> R<bool> {
    let parameter = "2_Diffuse_Skin_Mask";
    let Some(param_index) = name_index(pkg, parameter) else { return Ok(false) };
    let blanks: Vec<usize> = pkg.exports.iter().enumerate().filter(|(_, e)| pkg.class_name(e) == "Texture2D" && pkg.export_name(e) == "T_BlackAlpha").map(|(i, _)| i).collect();
    let [index] = blanks[..] else { return Ok(false) };
    let mut changed = 0;
    for (mi, exp) in pkg.exports.iter().enumerate() {
        if pkg.class_name(exp) != "MaterialInstanceConstant" { continue; }
        let data = colors::export_data(flat, exp);
        let Ok((_, _, props)) = parse_props(pkg, data) else { continue };
        for p in &props {
            if p.name != "TextureParameterValues" || p.kind != "ArrayProperty" { continue; }
            let arr = &data[p.value..p.value + p.size];
            let mut cursor = 4;
            let (mut template, mut exists) = (None, false);
            for _ in 0..i32_at(arr, 0).max(0) {
                let start = cursor;
                let Some((next, fields)) = properties(&pkg.names, arr, cursor) else { return Ok(false) };
                cursor = next;
                let n = fields.iter().find(|f| f.name == "ParameterName" && f.size == 8).cloned();
                let v = fields.iter().find(|f| f.name == "ParameterValue" && f.size == 4).cloned();
                if let (Some(n), Some(v)) = (n, v) {
                    let nm = &pkg.names[i32_at(arr, n.value) as usize];
                    if nm == parameter { exists = true; }
                    if nm == "1_Diffuse_Skin" { template = Some((start, cursor, n, v, fields.clone())); }
                }
            }
            let (false, Some((start, stop, n, v, fields))) = (exists, template) else { continue };
            let mut elem = arr[start..stop].to_vec();
            put32(&mut elem, n.value - start, param_index as i32);
            put32(&mut elem, v.value - start, index as i32 + 1);
            for f in &fields { if f.name == "ExpressionGUID" && f.size == 16 { elem[f.value - start..f.value - start + 16].fill(0); } }
            let at = p.value + p.size;
            let mut new = data[..at].to_vec();
            new.extend(&elem);
            new.extend(&data[at..]);
            put32(&mut new, p.value - 8, (p.size + elem.len()) as i32);
            put32(&mut new, p.value, i32_at(arr, 0) + 1);
            replacements.insert(mi, new);
            changed += 1;
        }
    }
    if changed > 0 { let exp = &pkg.exports[index]; replacements.insert(index, inline_serial(pkg, colors::export_data(flat, exp), raw, false)?); }
    Ok(changed > 0)
}

/// Material-bound diffuse colour textures (never normals/masks).
pub fn diffuse_textures(pkg: &Package) -> R<Vec<usize>> {
    let mut found = Vec::new();
    for (name, indexes) in bindings(pkg, pkg.flat()?) {
        let low = name.to_lowercase();
        if ["diffuse", "albedo", "basecolor", "rimdiffuse"].iter().any(|s| low.contains(s)) && !["normal", "mask", "emiss", "noise", "uv", "offset"].iter().any(|s| low.contains(s)) {
            for i in indexes { if !found.contains(&i) { found.push(i); } }
        }
    }
    found.sort();
    Ok(found)
}

fn decompress_bulk(data: &[u8]) -> R<Vec<u8>> {
    if data.len() < 16 || u32::from_le_bytes(data[..4].try_into().unwrap()) != PACKAGE_FILE_TAG { return Ok(data.to_vec()); }
    let total = i32_at(data, 12) as i64;
    let (mut cursor, mut size, mut blocks) = (16usize, 0i64, Vec::new());
    while size < total {
        let (comp, raw) = (i32_at(data, cursor), i32_at(data, cursor + 4));
        cursor += 8;
        if comp <= 0 || raw <= 0 || total > 64 * 1024 * 1024 { return Err("Bulk fuera de rango.".into()); }
        blocks.push((comp as usize, raw as usize));
        size += raw as i64;
    }
    let mut out = Vec::new();
    for (comp, raw) in blocks {
        let before = out.len();
        std::io::Read::read_to_end(&mut flate2::read::ZlibDecoder::new(data.get(cursor..cursor + comp).ok_or("Bulk inválido.")?), &mut out).map_err(err)?;
        if out.len() - before != raw { return Err("Bulk inválido.".into()); }
        cursor += comp;
    }
    Ok(out)
}

/// The largest readable mip of a colour texture (inline or in the game's .tfc cache).
pub fn decode_texture(pkg: &Package, index: usize, cooked: Option<&Path>) -> R<image::RgbaImage> {
    let exp = &pkg.exports[index];
    let data = colors::export_data(pkg.flat()?, exp);
    let (_, end, props) = parse_props(pkg, data)?;
    let (_, _, levels) = mip_table(data, end)?;
    let fmt = props.iter().find(|p| p.name == "Format").map(|p| name_of(pkg, data, p)).unwrap_or_default();
    let cache = props.iter().find(|p| p.name == "TextureFileCacheName").map(|p| name_of(pkg, data, p)).unwrap_or("None".into());
    let mut candidates: Vec<(i64, i32, i32, Vec<u8>)> = Vec::new();
    for (pos, flags, disk, w, h) in levels {
        if disk <= 0 || flags & 0x20 != 0 || w as i64 * h as i64 > 4096 * 4096 { continue; }
        let raw = if flags & 1 != 0 {
            let Some(cooked) = cooked else { continue };
            if cache == "None" || cache.contains(['/', '\\', ':']) { continue; }
            let file = cooked.join(format!("{cache}.tfc"));
            let offset = i64_at(data, pos + 12);
            let Ok(meta) = std::fs::metadata(&file) else { continue };
            if !meta.is_file() || offset < 0 || offset as u64 + disk as u64 > meta.len() { continue; }
            let mut stream = std::fs::File::open(&file).map_err(err)?;
            std::io::Seek::seek(&mut stream, std::io::SeekFrom::Start(offset as u64)).map_err(err)?;
            let mut raw = vec![0u8; disk as usize];
            std::io::Read::read_exact(&mut stream, &mut raw).map_err(err)?;
            raw
        } else { data[pos + 12..pos + 12 + disk as usize].to_vec() };
        candidates.push((w as i64 * h as i64, w, h, raw));
    }
    candidates.sort_by(|a, b| b.cmp(a));
    for (_, w, h, raw) in candidates {
        let Ok(raw) = decompress_bulk(&raw) else { continue };
        let decoded = match fmt.as_str() {
            "PF_A8R8G8B8" => image::RgbaImage::from_raw(w as u32, h as u32, raw.chunks_exact(4).flat_map(|p| [p[2], p[1], p[0], p[3]]).collect()).ok_or_else(String::new),
            "PF_DXT1" | "PF_DXT5" => dxt::decode(&raw, w as u32, h as u32, &fmt[3..]),
            _ => continue,
        };
        if let Ok(img) = decoded { return Ok(img); }
    }
    Err(format!("No se pudo leer la textura de color {}", pkg.path(index)?))
}

/// Python's round(): halves go to the even neighbour.
fn round_half_even(x: f64) -> f64 { let r = x.round(); if (x - x.trunc()).abs() == 0.5 { 2. * (x / 2.).round() } else { r } }

/// Colour textures become their brightness (tinted by the paint when the material has no RGB parameter).
pub fn neutralize_diffuse(pkg: Package, cooked: Option<&Path>, color: &str, has_rgb_parameters: bool) -> R<(Package, Vec<String>)> {
    let candidates = diffuse_textures(&pkg)?;
    let mut replacements = BTreeMap::new();
    let mut names = Vec::new();
    let flat = pkg.flat()?;
    let rgb = colors::paint_rgb(color).unwrap_or([1., 1., 1.]);
    // PNG stores sRGB; palette values are linear material RGB.
    let srgb = |x: f64| if x <= 0.0031308 { 12.92 * x } else { 1.055 * x.powf(1. / 2.4) - 0.055 };
    let tables: Vec<Vec<u8>> = rgb.iter().map(|c| { let c = srgb(*c).clamp(0., 1.); (0..256).map(|i| round_half_even(i as f64 * c) as u8).collect() }).collect();
    for index in candidates {
        let mut picture = decode_texture(&pkg, index, cooked)?;
        for p in picture.pixels_mut() {
            let v = p[0].max(p[1]).max(p[2]);
            *p = if has_rgb_parameters { image::Rgba([v, v, v, p[3]]) } else { image::Rgba([tables[0][v as usize], tables[1][v as usize], tables[2][v as usize], p[3]]) };
        }
        replacements.insert(index, inline_serial(&pkg, colors::export_data(flat, &pkg.exports[index]), &dxt::png(&picture)?, false)?);
        names.push(pkg.path(index)?);
    }
    if replacements.is_empty() { return Ok((pkg, Vec::new())); }
    let raw = append_exports(&pkg, &replacements, None, None)?;
    Ok((Package::from_raw(&pkg.file, raw)?, names))
}

/// Paint a cosmetic: RGB sources, then diffuse textures.
pub fn paint(pkg: Package, color: &str, cooked: Option<&Path>) -> R<(Package, Value)> {
    if colors::paint_rgb(color).is_none() { return Err("Color no válido.".into()); }
    let ranges = colors::color_ranges(&pkg, pkg.flat()?)?;
    let diffuse = diffuse_textures(&pkg)?;
    let main = ranges.0.iter().any(|(_, r)| r["neutral_multiplier"] != true);
    if !main && diffuse.is_empty() { return Err("Este paquete no tiene fuentes de color principal editables verificadas.".into()); }
    let (colored, mut report) = colors::paint_rgb_sources(&pkg, color, &ranges)?;
    let colored = colored.unwrap_or(pkg);
    let (colored, textures) = if diffuse.is_empty() { (colored, Vec::new()) } else { neutralize_diffuse(colored, cooked, color, main)? };
    report["diffuse_textures_neutralized_or_baked"] = json!(textures);
    Ok((colored, report))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn python_rounding() {
        assert_eq!(round_half_even(2.5), 2.);
        assert_eq!(round_half_even(3.5), 4.);
        assert_eq!(round_half_even(2.6), 3.);
    }
}

/// SP_TEX_CASES=<folder from the Python side> cargo test -- --ignored real_textures
#[cfg(test)]
mod parity {
    use super::*;
    /// Splits a serial into (bytes before the mips, [(flags, sizes, w, h, pixels)], tail).
    fn split(pkg: &Package, data: &[u8]) -> (Vec<u8>, Vec<(u32, i32, i32, i32, Vec<u8>)>, Vec<u8>) {
        let (_, end, _) = parse_props(pkg, data).unwrap();
        let (off, tail, levels) = mip_table(data, end).unwrap();
        let mips = levels.iter().map(|&(pos, flags, disk, w, h)| (flags, i32_at(data, pos + 4), disk, w * 10000 + h, data[pos + 12..pos + 12 + disk as usize].to_vec())).collect();
        (data[..off].to_vec(), mips, data[tail..].to_vec())
    }
    #[test]
    #[ignore]
    fn real_textures_match_python() {
        let Some(dir) = std::env::var_os("SP_TEX_CASES") else { return };
        let dir = std::path::PathBuf::from(dir);
        let cases: Value = serde_json::from_slice(&std::fs::read(dir.join("cases.json")).unwrap()).unwrap();
        let picture = std::fs::read(cases["image"].as_str().unwrap()).unwrap();
        for case in cases["cases"].as_array().unwrap() {
            let pkg = Package::open(Path::new(case["file"].as_str().unwrap())).unwrap();
            let data = colors::export_data(pkg.flat().unwrap(), &pkg.exports[case["export"].as_u64().unwrap() as usize]).to_vec();
            let ours = inline_serial(&pkg, &data, &picture, case["fit"].as_bool().unwrap()).unwrap();
            let theirs = std::fs::read(case["python"].as_str().unwrap()).unwrap();
            let (oh, om, ot) = split(&pkg, &ours);
            let (th, tm, tt) = split(&pkg, &theirs);
            let format = case["format"].as_str().unwrap();
            assert_eq!(oh, th, "{format}: cabecera distinta");
            assert_eq!(ot, tt, "{format}: cola distinta");
            assert_eq!(om.len(), tm.len(), "{format}: número de mips");
            for (a, b) in om.iter().zip(&tm) { assert_eq!((a.0, a.1, a.2, a.3), (b.0, b.1, b.2, b.3), "{format}: mip distinto"); }
            let (w, h) = ((om[0].3 / 10000) as u32, (om[0].3 % 10000) as u32);
            let decode = |raw: &[u8]| if format == "PF_A8R8G8B8" { image::RgbaImage::from_raw(w, h, raw.to_vec()).unwrap() } else { dxt::decode(raw, w, h, &format[3..]).unwrap() };
            // Two lossy encoders differ from each other; each is measured against the source picture instead.
            let source = dxt::load(&picture).unwrap();
            let reference = if case["fit"].as_bool().unwrap() { dxt::fit(&source, w, h) } else { dxt::resize(&source, w, h) };
            let psnr = |img: &image::RgbaImage| {
                let mse: f64 = img.pixels().zip(reference.pixels()).map(|(x, y)| (0..4).map(|i| (x[i] as f64 - y[i] as f64).powi(2)).sum::<f64>()).sum::<f64>() / (w as f64 * h as f64 * 4.);
                10. * (255f64.powi(2) / mse.max(1e-9)).log10()
            };
            let (ours, python) = (psnr(&decode(&om[0].4)), psnr(&decode(&tm[0].4)));
            println!("{format} {}x{} fit={} PSNR rust {:.1} dB / python {:.1} dB", w, h, case["fit"], ours, python);
            assert!(ours >= python - 1.0, "{format}: calidad peor que Python ({ours:.1} < {python:.1} dB)");
        }
    }
}
