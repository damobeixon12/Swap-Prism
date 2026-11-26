//! Bounded reader/patcher for the HUD's AVM2 (ActionScript 3) data inside GFx movies; never executes bytecode.
//! Port of engine/hud_abc.py and engine/avm2_read.py (operand counts from Adobe avmplus opcodes.tbl, MPL 2.0).
use super::colors::export_properties;
use super::upk::{i32_at, Package};
use super::util::R;
use serde_json::{json, Value};
use std::collections::HashMap;

pub struct Reader<'a> { pub data: &'a [u8], pub pos: usize }
impl<'a> Reader<'a> {
    pub fn byte(&mut self) -> R<u8> { let v = *self.data.get(self.pos).ok_or("ABC truncado.")?; self.pos += 1; Ok(v) }
    pub fn u30(&mut self) -> R<u32> {
        let mut value = 0u32;
        for n in 0..5 {
            let part = *self.data.get(self.pos).ok_or("ABC truncado.")?;
            self.pos += 1;
            value |= ((part & 127) as u32) << (7 * n);
            if part < 128 { return Ok(value); }
        }
        Err("Entero ABC inválido.".into())
    }
    pub fn skip(&mut self, n: usize) -> R<()> { self.pos += n; if self.pos > self.data.len() { Err("ABC fuera de rango.".into()) } else { Ok(()) } }
}

pub fn encode_u30(mut value: u32) -> Vec<u8> {
    let mut out = Vec::new();
    while value >= 128 { out.push(((value & 127) | 128) as u8); value >>= 7; }
    out.push(value as u8);
    out
}

#[derive(Clone, Debug, PartialEq)]
pub struct Multiname { pub kind: u8, pub ns: u32, pub name: u32, pub params: Vec<u32> }
#[derive(Clone, Debug, PartialEq)]
pub struct Method { pub params: Vec<u32>, pub ret: u32, pub name: u32, pub flags: u8 }
#[derive(Clone, Debug, PartialEq)]
pub struct Trait { pub name: String, pub kind: u8, pub method: Option<u32> }
#[derive(Clone, Debug, PartialEq)]
pub struct Class { pub init: u32, pub traits: Vec<Trait>, pub cinit: u32, pub static_traits: Vec<Trait> }
#[derive(Clone, Debug, PartialEq)]
pub struct Body { pub start: usize, pub end: usize, pub stackpos: usize, pub stack: u32, pub locals: u32, pub initial_scope: u32, pub max_scope: u32, pub lengthpos: usize, pub codepos: usize, pub code: Vec<u8>, pub exceptions: u32 }

pub struct Abc {
    pub data: Vec<u8>,
    pub ints: Vec<u32>,
    pub string_count_pos: usize,
    pub strings: Vec<String>,
    pub string_pool_end: usize,
    pub namespaces: Vec<(u8, u32)>,
    pub nssets: Vec<Vec<u32>>,
    pub multinames: Vec<Multiname>,
    pub methods: Vec<Method>,
    pub classes: Vec<(String, Class)>,
    pub bodies: Vec<(u32, Body)>,
    names: std::sync::OnceLock<HashMap<String, Vec<usize>>>,
}

impl Abc {
    pub fn parse(data: &[u8]) -> R<Abc> {
        let mut r = Reader { data, pos: 0 };
        r.skip(4)?;
        let mut ints = vec![0u32];
        for _ in 0..r.u30()?.saturating_sub(1) { ints.push(r.u30()?); }
        for _ in 0..r.u30()?.saturating_sub(1) { r.u30()?; }
        for _ in 0..r.u30()?.saturating_sub(1) { r.skip(8)?; }
        let string_count_pos = r.pos;
        let mut strings = vec![String::new()];
        for _ in 0..r.u30()?.saturating_sub(1) {
            let n = r.u30()? as usize;
            strings.push(String::from_utf8(data.get(r.pos..r.pos + n).ok_or("ABC fuera de rango.")?.to_vec()).map_err(|_| "Texto ABC no válido.".to_string())?);
            r.skip(n)?;
        }
        let string_pool_end = r.pos;
        let mut namespaces = vec![(0u8, 0u32)];
        for _ in 0..r.u30()?.saturating_sub(1) { namespaces.push((r.byte()?, r.u30()?)); }
        let mut nssets = vec![Vec::new()];
        for _ in 0..r.u30()?.saturating_sub(1) { let n = r.u30()?; nssets.push((0..n).map(|_| r.u30()).collect::<R<_>>()?); }
        let mut multinames = vec![Multiname { kind: 0, ns: 0, name: 0, params: Vec::new() }];
        for _ in 0..r.u30()?.saturating_sub(1) {
            let kind = r.byte()?;
            let m = match kind {
                7 | 13 => Multiname { kind, ns: r.u30()?, name: r.u30()?, params: Vec::new() },
                15 | 16 => Multiname { kind, ns: 0, name: r.u30()?, params: Vec::new() },
                17 | 18 => Multiname { kind, ns: 0, name: 0, params: Vec::new() },
                9 | 14 => { let name = r.u30()?; let nsset = r.u30()?; Multiname { kind, ns: nsset, name, params: Vec::new() } }
                27 | 28 => Multiname { kind, ns: r.u30()?, name: 0, params: Vec::new() },
                29 => { let base = r.u30()?; let n = r.u30()?; Multiname { kind, ns: base, name: 0, params: (0..n).map(|_| r.u30()).collect::<R<_>>()? } }
                _ => return Err(format!("Multiname ABC no reconocido: {kind}")),
            };
            multinames.push(m);
        }
        let mut abc = Abc { data: data.to_vec(), ints, string_count_pos, strings, string_pool_end, namespaces, nssets, multinames, methods: Vec::new(), classes: Vec::new(), bodies: Vec::new(), names: std::sync::OnceLock::new() };
        for _ in 0..r.u30()? {
            let count = r.u30()?;
            let ret = r.u30()?;
            let params = (0..count).map(|_| r.u30()).collect::<R<Vec<_>>>()?;
            let name = r.u30()?;
            let flags = r.byte()?;
            if flags & 8 != 0 { for _ in 0..r.u30()? { r.u30()?; r.byte()?; } }
            if flags & 128 != 0 { for _ in 0..count { r.u30()?; } }
            abc.methods.push(Method { params, ret, name, flags });
        }
        for _ in 0..r.u30()? { r.u30()?; let count = r.u30()?; for _ in 0..count * 2 { r.u30()?; } }
        let count = r.u30()?;
        let mut order = Vec::new();
        for _ in 0..count {
            let name = r.u30()?;
            r.u30()?;
            let flags = r.byte()?;
            if flags & 8 != 0 { r.u30()?; }
            for _ in 0..r.u30()? { r.u30()?; }
            let init = r.u30()?;
            let traits = abc.traits(&mut r)?;
            let key = abc.name(name);
            let class = Class { init, traits, cinit: 0, static_traits: Vec::new() };
            match abc.classes.iter().position(|c| c.0 == key) { Some(i) => abc.classes[i].1 = class, None => abc.classes.push((key.clone(), class)) }
            order.push(key);
        }
        // Static parts follow in declaration order; a repeated class name keeps the last record (Python dict values).
        for i in 0..abc.classes.len() {
            let cinit = r.u30()?;
            let traits = abc.traits(&mut r)?;
            abc.classes[i].1.cinit = cinit;
            abc.classes[i].1.static_traits = traits;
        }
        let _ = order;
        for _ in 0..r.u30()? { r.u30()?; abc.traits(&mut r)?; }
        for _ in 0..r.u30()? {
            let start = r.pos;
            let method = r.u30()?;
            let stackpos = r.pos;
            let (stack, locals, initial_scope, max_scope) = (r.u30()?, r.u30()?, r.u30()?, r.u30()?);
            let lengthpos = r.pos;
            let length = r.u30()? as usize;
            let codepos = r.pos;
            r.skip(length)?;
            let exceptions = r.u30()?;
            for _ in 0..exceptions * 5 { r.u30()?; }
            abc.traits(&mut r)?;
            let body = Body { start, end: r.pos, stackpos, stack, locals, initial_scope, max_scope, lengthpos, codepos, code: data[codepos..codepos + length].to_vec(), exceptions };
            match abc.bodies.iter().position(|b| b.0 == method) { Some(i) => abc.bodies[i].1 = body, None => abc.bodies.push((method, body)) }
        }
        if r.pos != data.len() { return Err("Hay datos ABC sin interpretar.".into()); }
        Ok(abc)
    }

    pub fn name(&self, index: u32) -> String {
        if index == 0 { return "*".into(); }
        let Some(m) = self.multinames.get(index as usize) else { return String::new() };
        if m.kind == 29 { return format!("{}<{}>", self.name(m.ns), m.params.iter().map(|i| self.name(*i)).collect::<Vec<_>>().join(",")); }
        let text = self.strings.get(m.name as usize).cloned().unwrap_or_default();
        if m.kind == 7 || m.kind == 13 {
            let namespace = self.namespaces.get(m.ns as usize).and_then(|n| self.strings.get(n.1 as usize)).cloned().unwrap_or_default();
            if !namespace.is_empty() { return format!("{namespace}.{text}"); }
        }
        text
    }

    fn traits(&self, r: &mut Reader) -> R<Vec<Trait>> {
        let mut out = Vec::new();
        for _ in 0..r.u30()? {
            let name = r.u30()?;
            let flags = r.byte()?;
            let kind = flags & 15;
            let mut value = None;
            match kind {
                0 | 6 => { r.u30()?; r.u30()?; let v = r.u30()?; if v != 0 { r.byte()?; } }
                1..=5 => { r.u30()?; value = Some(r.u30()?); }
                _ => return Err("Trait ABC no reconocido.".into()),
            }
            if flags & 64 != 0 { for _ in 0..r.u30()? { r.u30()?; } }
            out.push(Trait { name: self.name(name), kind, method: value });
        }
        Ok(out)
    }

    pub fn class(&self, name: &str) -> Option<&Class> { self.classes.iter().find(|c| c.0 == name).map(|c| &c.1) }
    pub fn body(&self, method: u32) -> Option<&Body> { self.bodies.iter().find(|b| b.0 == method).map(|b| &b.1) }

    /// Multiname index for a public name (title_display._ref): a public QName first, then the smallest public multiname set.
    pub fn reference(&self, name: &str) -> R<u32> {
        let map = self.names.get_or_init(|| {
            let mut m: HashMap<String, Vec<usize>> = HashMap::new();
            for i in 1..self.multinames.len() { m.entry(self.name(i as u32)).or_default().push(i); }
            m
        });
        let found = map.get(name).cloned().unwrap_or_default();
        let public_ns = |n: u32| self.namespaces.get(n as usize).is_some_and(|ns| ns.0 == 0x16 && self.strings.get(ns.1 as usize).is_some_and(|s| s.is_empty()));
        if let Some(&i) = found.iter().find(|&&i| { let m = &self.multinames[i]; (m.kind == 7 || m.kind == 13) && public_ns(m.ns) }) { return Ok(i as u32); }
        let sets: Vec<usize> = found.iter().copied().filter(|&i| { let m = &self.multinames[i]; (m.kind == 9 || m.kind == 14) && self.nssets.get(m.ns as usize).is_some_and(|s| s.iter().any(|n| public_ns(*n))) }).collect();
        if let Some(&best) = sets.iter().min_by_key(|&&i| self.nssets[self.multinames[i].ns as usize].len()) { return Ok(best as u32); }
        let [one] = found[..] else { return Err(format!("Referencia de título ambigua: {name}")) };
        Ok(one as u32)
    }
}

/// GFx tags: (code, start, body, end).
pub fn swf_tags(data: &[u8]) -> R<Vec<(u16, usize, usize, usize)>> {
    if data.get(..3) != Some(b"GFX") { return Err("El HUD no usa el formato GFX esperado.".into()); }
    let n = (*data.get(8).ok_or("GFX truncado.")? >> 3) as usize;
    let mut p = 8 + (5 + 4 * n + 7) / 8 + 4;
    let mut out = Vec::new();
    while p + 2 <= data.len() {
        let head = u16::from_le_bytes([data[p], data[p + 1]]);
        let code = head >> 6;
        let (mut size, mut header) = ((head & 63) as usize, 2);
        if size == 63 { size = u32::from_le_bytes(data.get(p + 2..p + 6).ok_or("Tag GFX fuera de rango.")?.try_into().unwrap()) as usize; header = 6; }
        let (body, end) = (p + header, p + header + size);
        if end > data.len() { return Err("Tag GFX fuera de rango.".into()); }
        out.push((code, p, body, end));
        if code == 0 { break; }
        p = end;
    }
    Ok(out)
}

/// (tag start, body, end, ABC start, parsed ABC) of the movie's single DoABC tag.
pub fn abc_tag(data: &[u8]) -> R<(usize, usize, usize, usize, Abc)> {
    let matches: Vec<_> = swf_tags(data)?.into_iter().filter(|t| t.0 == 82).collect();
    let [(_, start, body, end)] = matches[..] else { return Err("No hay un único bloque ABC en el HUD.".into()) };
    let pos = data[body + 4..end].iter().position(|b| *b == 0).map(|i| body + 4 + i + 1).ok_or("ABC sin nombre.")?;
    Ok((start, body, end, pos, Abc::parse(&data[pos..end])?))
}

pub fn operand_count(op: u8) -> i32 {
    static TABLE: std::sync::OnceLock<Vec<i32>> = std::sync::OnceLock::new();
    TABLE.get_or_init(|| serde_json::from_str(include_str!("../../assets/engine/avm2_operands.json")).unwrap_or_default())[op as usize]
}

/// Every instruction (start, end, opcode, operands) of a method body.
pub fn instructions(code: &[u8]) -> R<Vec<(usize, usize, u8, Vec<u32>)>> {
    let mut r = Reader { data: code, pos: 0 };
    let mut out = Vec::new();
    while r.pos < code.len() {
        let start = r.pos;
        let op = r.byte()?;
        let count = operand_count(op);
        let mut args = Vec::new();
        if (0x0c..=0x1a).contains(&op) { r.skip(3)?; }
        else if op == 0x1b { r.skip(3)?; let n = r.u30()? as usize; r.skip((n + 1) * 3)?; }
        else if op == 0x24 { args.push(r.byte()? as u32); }
        else if op == 0xef { args = vec![r.byte()? as u32, r.u30()?, r.byte()? as u32, r.u30()?]; }
        else if count >= 0 { for _ in 0..count { args.push(r.u30()?); } }
        else { return Err("Instrucción AVM2 no reconocida.".into()); }
        out.push((start, r.pos, op, args));
    }
    Ok(out)
}

/// Only the instruction subset present in BoostMeterView's display-colour method.
fn decode_color_instructions(code: &[u8]) -> R<Vec<(usize, usize, u8, Vec<u32>)>> {
    let counts: HashMap<u8, usize> = [(0x08, 1), (0x20, 0), (0x29, 0), (0x2a, 0), (0x2d, 1), (0x2f, 1), (0x30, 0), (0x46, 2), (0x47, 0), (0x60, 1), (0x61, 1),
        (0x26, 0), (0x27, 0), (0x2c, 1), (0x4f, 2), (0x55, 1), (0x56, 1), (0x5d, 1), (0x66, 1), (0x68, 1), (0x74, 0), (0x75, 0),
        (0x80, 1), (0x82, 0), (0x85, 0), (0xa0, 0), (0xa3, 0), (0xa6, 0), (0xaf, 0), (0xb0, 0),
        (0xd0, 0), (0xd1, 0), (0xd2, 0), (0xd3, 0), (0xd5, 0), (0xd6, 0), (0xd7, 0), (0x02, 0)].into_iter().collect();
    let mut r = Reader { data: code, pos: 0 };
    let mut result = Vec::new();
    while r.pos < code.len() {
        let at = r.pos;
        let op = r.byte()?;
        let args = if (0x0c..=0x1a).contains(&op) { r.skip(3)?; Vec::new() }
            else if op == 0x24 { vec![r.byte()? as u32] }
            else if let Some(&n) = counts.get(&op) { (0..n).map(|_| r.u30()).collect::<R<_>>()? }
            else { return Err("El método de color del HUD usa instrucciones no verificadas.".into()) };
        result.push((at, r.pos, op, args));
    }
    Ok(result)
}

fn neutralize_color_method(code: &[u8], abc: &Abc, identity_id: u32, require_full_profile: bool) -> R<(Vec<u8>, Vec<(usize, usize)>)> {
    let instructions = decode_color_instructions(code)?;
    if abc.name(identity_id) != "COLOR_NORMAL" { return Err("Constante neutra no válida.".into()); }
    let mut neutral = vec![0x60u8];
    neutral.extend(encode_u30(identity_id));
    let mut out = code.to_vec();
    let mut edits = Vec::new();
    let replacements = ["COLOR_GLOW_RED", "COLOR_GLOW_AMBER", "COLOR_BOOST_MAX", "COLOR_BOOST_NORMAL", "COLOR_BACKGROUND_AMBER"];
    let mut found: Vec<String> = Vec::new();
    let mut put = |out: &mut Vec<u8>, start: usize, end: usize| -> R<()> {
        if neutral.len() > end - start { return Err("El reemplazo de color cambia los saltos del HUD.".into()); }
        out[start..start + neutral.len()].copy_from_slice(&neutral);
        out[start + neutral.len()..end].fill(0x02);
        Ok(())
    };
    for (start, end, op, args) in &instructions {
        if *op == 0x60 && replacements.contains(&abc.name(args[0]).as_str()) {
            put(&mut out, *start, *end)?;
            edits.push((*start, *end));
            let n = abc.name(args[0]);
            if !found.contains(&n) { found.push(n); }
        }
    }
    let mut team = 0;
    for w in instructions.windows(3) {
        let (a, b, c) = (&w[0], &w[1], &w[2]);
        if a.2 == 0xd0 && b.2 == 0x66 && abc.name(b.3[0]).ends_with(".m_teamStyle") && c.2 == 0x66 && abc.name(c.3[0]) == "colorTransform" {
            put(&mut out, a.0, c.1)?;
            edits.push((a.0, c.1));
            team += 1;
        }
    }
    if require_full_profile && (found.len() != replacements.len() || team != 1) { return Err("El HUD no coincide con el perfil de colores verificado.".into()); }
    if edits.is_empty() { return Err("No se localizaron colores de presentación en el método del HUD.".into()); }
    if code.iter().zip(&out).enumerate().any(|(i, (a, b))| a != b && !edits.iter().any(|e| (e.0..e.1).contains(&i))) { return Err("Cambio fuera del color del boostmeter.".into()); }
    Ok((out, edits))
}

const METER_METHODS: [&str; 3] = ["onTeamColorChanged", "onBoostChanged", "updateBoost"];

fn method_of(cls: &Class, method_name: &str) -> Vec<u32> {
    cls.traits.iter().filter(|t| t.kind == 1 && t.name.ends_with(&format!(".{method_name}"))).filter_map(|t| t.method).collect()
}

pub fn neutral_boostmeter_gfx(data: &[u8]) -> R<(Vec<u8>, Value)> {
    let (_, _, _, start, abc) = abc_tag(data)?;
    let cls = abc.class("tagame.hud.BoostMeterView").ok_or("No se identifica BoostMeterView.")?;
    let matches = method_of(cls, "onTeamColorChanged");
    let [method] = matches[..] else { return Err("No hay un único método de color del boostmeter.".into()) };
    let body = abc.body(method).ok_or("Método sin cuerpo.")?;
    if !abc.methods[method as usize].params.is_empty() || body.exceptions != 0 { return Err("El método de color del HUD cambió de estructura.".into()); }
    let identity = decode_color_instructions(&body.code)?.into_iter().find(|i| i.2 == 0x60 && abc.name(i.3[0]) == "COLOR_NORMAL").map(|i| i.3[0]).ok_or("No se identifica la transformación neutra del boostmeter.")?;
    let mut out = data.to_vec();
    let mut changed: Vec<u32> = Vec::new();
    let mut allowed: Vec<(usize, usize)> = Vec::new();
    let mut sites = 0;
    for method_name in METER_METHODS {
        let traits = method_of(cls, method_name);
        let [mid] = traits[..] else { return Err("El HUD no tiene el método de presentación esperado.".into()) };
        let record = abc.body(mid).ok_or("Método sin cuerpo.")?;
        if record.exceptions != 0 { return Err("El método del HUD cambió de estructura.".into()); }
        let (patched, edits) = neutralize_color_method(&record.code, &abc, identity, method_name == "onTeamColorChanged")?;
        let at = start + record.codepos;
        out[at..at + patched.len()].copy_from_slice(&patched);
        changed.push(mid);
        sites += edits.len();
        allowed.extend(edits.iter().map(|e| (at + e.0, at + e.1)));
    }
    let checked = abc_tag(&out)?.4;
    for (index, old) in &abc.bodies {
        if !changed.contains(index) && checked.body(*index).map(|b| &b.code) != Some(&old.code) { return Err("Cambió otro método del HUD.".into()); }
    }
    if data.iter().zip(&out).enumerate().any(|(i, (a, b))| a != b && !allowed.iter().any(|e| (e.0..e.1).contains(&i))) { return Err("Cambio fuera de las referencias de color de presentación.".into()); }
    let colored = white_meter_labels(&out)?;
    Ok((colored, json!({"boostmeter_text_color": "explicit-white", "only_meter_display_methods_changed": true, "boost_amount_calculation_instructions_unchanged": true,
        "methods": METER_METHODS, "color_sites": sites, "label_and_number_textColor_explicit": true, "runtime_visual_verified": false})))
}

/// Finish native meter updates with explicit label colours; never edits the global HUD.
fn white_meter_labels(movie: &[u8]) -> R<Vec<u8>> {
    let (start, body, end, pos, abc) = abc_tag(movie)?;
    let r = |name: &str| abc.reference(name);
    let prop = |name: &str| -> R<Vec<u8>> { let mut v = vec![0x66u8]; v.extend(encode_u30(r(name)?)); Ok(v) };
    let setprop = |name: &str| -> R<Vec<u8>> { let mut v = vec![0x61u8]; v.extend(encode_u30(r(name)?)); Ok(v) };
    let white = abc.ints.iter().position(|v| *v == 0xffffff).ok_or("No se identifica el blanco nativo del medidor.")? as u32;
    // Fresh transforms rather than a shared static object; tweens lose only their colorTransform property.
    let transform = r("flash.geom.ColorTransform")?;
    let mut normal = vec![0x5d];
    normal.extend(encode_u30(transform));
    normal.push(0x4a);
    normal.extend(encode_u30(transform));
    normal.push(0x00);
    let mut tween = vec![0x60];
    tween.extend(encode_u30(r("com.greensock.TweenMax")?));
    let mut color_string = vec![0x2c];
    color_string.extend(encode_u30(abc.strings.iter().position(|s| s == "colorTransform").ok_or("Falta colorTransform.")? as u32));
    let mut suffix = vec![0xd0];
    suffix.extend(prop("transform")?);
    suffix.extend(&normal);
    suffix.extend(setprop("colorTransform")?);
    let mut fields: Vec<Vec<u8>> = Vec::new();
    for name in ["backgroundMovieClip", "fillProgressBar", "fillProgressBarTinted", "glowMovieClip", "backgroundTextField", "boostTextField", "boostLabel"] {
        let mut f = vec![0xd0]; f.extend(prop(name)?); fields.push(f);
    }
    let mut label_text = vec![0xd0];
    label_text.extend(prop("boostLabel")?);
    label_text.extend(prop("textField")?);
    fields.push(label_text.clone());
    let kill = encode_u30(r("killTweensOf")?);
    for field in &fields {
        suffix.extend(&tween); suffix.extend(field); suffix.push(0x27); suffix.extend(&color_string); suffix.extend([0x26, 0x55, 0x01, 0x4f]); suffix.extend(&kill); suffix.push(0x03);
        suffix.extend(field); suffix.extend(prop("transform")?); suffix.extend(&normal); suffix.extend(setprop("colorTransform")?);
    }
    let mut text_field = vec![0xd0];
    text_field.extend(prop("boostTextField")?);
    for field in [text_field, label_text] {
        suffix.extend(&field); suffix.push(0x2d); suffix.extend(encode_u30(white)); suffix.extend(setprop("textColor")?);
        suffix.extend(&field); suffix.extend(prop("transform")?); suffix.extend(&normal); suffix.extend(setprop("colorTransform")?);
    }
    let cls = abc.class("tagame.hud.BoostMeterView").ok_or("No se identifica BoostMeterView.")?;
    let mut edits: Vec<(usize, usize, Vec<u8>)> = Vec::new();
    let mut changed: HashMap<u32, Vec<u8>> = HashMap::new();
    for t in &cls.traits {
        if t.kind != 1 || !METER_METHODS.contains(&t.name.rsplit('.').next().unwrap_or("")) { continue; }
        let mid = t.method.unwrap_or(0);
        let record = abc.body(mid).ok_or("Método sin cuerpo.")?;
        if record.exceptions != 0 || record.code.last() != Some(&0x47) || record.stack < 5 { return Err("Método de medidor no reconocido.".into()); }
        let mut code = record.code[..record.code.len() - 1].to_vec();
        code.extend(&suffix);
        code.push(0x47);
        let mut value = encode_u30(code.len() as u32);
        value.extend(&code);
        edits.push((record.lengthpos, record.codepos + record.code.len(), value));
        changed.insert(mid, code);
    }
    if changed.len() != 3 { return Err("No se verificaron las tres actualizaciones del medidor.".into()); }
    let mut data = abc.data.clone();
    edits.sort_by(|a, b| b.0.cmp(&a.0));
    for (a, b, value) in edits { data.splice(a..b, value); }
    let after = Abc::parse(&data)?;
    for (mid, record) in &abc.bodies {
        if after.body(*mid).map(|b| &b.code) != Some(changed.get(mid).unwrap_or(&record.code)) { return Err("Cambió otro método del HUD.".into()); }
    }
    let mut payload = movie[body..pos].to_vec();
    payload.extend(&data);
    let mut result = movie[..start].to_vec();
    result.extend((((82u16) << 6) | 63).to_le_bytes());
    result.extend((payload.len() as u32).to_le_bytes());
    result.extend(&payload);
    result.extend(&movie[end..]);
    let total = result.len() as u32;
    result[4..8].copy_from_slice(&total.to_le_bytes());
    abc_tag(&result)?;
    Ok(result)
}

/// The HUD movie inside GFX_Hud_SF with the neutral boost meter colours.
pub fn patch_boostmeter_movie(pkg: &Package, flat: &[u8]) -> R<(usize, Vec<u8>, Value)> {
    let movies: Vec<usize> = (0..pkg.exports.len()).filter(|&i| pkg.class_name(&pkg.exports[i]) == "SwfMovie" && pkg.path(i).is_ok_and(|p| p == "GFX_Hud.HUD")).collect();
    let [index] = movies[..] else { return Err("No se identifica la película del boostmeter.".into()) };
    let (data, props) = export_properties(pkg, flat, &pkg.exports[index]);
    let raw = props.iter().find(|p| p.name == "RawData" && p.kind == "ArrayProperty").filter(|p| i32_at(data, p.value) as usize == p.size - 4).ok_or("RawData del HUD no válido.")?;
    let at = raw.value + 4;
    let old = &data[at..at + raw.size - 4];
    let (new, report) = neutral_boostmeter_gfx(old)?;
    let mut result = data[..at].to_vec();
    result.extend(&new);
    result.extend(&data[at + old.len()..]);
    result[raw.value - 8..raw.value - 4].copy_from_slice(&(4 + new.len() as i32).to_le_bytes());
    result[raw.value..raw.value + 4].copy_from_slice(&(new.len() as i32).to_le_bytes());
    if let Some(crc) = props.iter().find(|p| p.name == "SourceFileCRC" && p.size == 4) {
        let pos = (crc.value as i64 + if crc.value > at { new.len() as i64 - old.len() as i64 } else { 0 }) as usize;
        result[pos..pos + 4].copy_from_slice(&crc32fast::hash(&new).to_le_bytes());
    }
    Ok((index, result, report))
}

/// SP_HUD_EXPECTED=<json from the Python side> cargo test -- --ignored real_hud
#[cfg(test)]
mod parity {
    use super::*;
    use sha2::{Digest, Sha256};
    fn h(b: &[u8]) -> String { format!("{:x}", Sha256::digest(b)) }
    /// Same summary layout as the Python script (json.dumps defaults).
    fn summary(abc: &Abc) -> String {
        let traits = |ts: &Vec<Trait>| ts.iter().map(|t| json!([t.name, t.kind, t.method])).collect::<Vec<_>>();
        let value = json!({"classes": abc.classes.iter().map(|(n, c)| json!([n, c.init, c.cinit, traits(&c.traits), traits(&c.static_traits)])).collect::<Vec<_>>(),
            "methods": abc.methods.iter().map(|m| json!([m.params, m.ret, m.name, m.flags])).collect::<Vec<_>>(),
            "strings": h(abc.strings.join("\n").as_bytes()),
            "bodies": h(&abc.bodies.iter().flat_map(|(k, b)| { let mut v = k.to_string().into_bytes(); v.extend(&b.code); v }).collect::<Vec<u8>>()),
            "body_count": abc.bodies.len(), "ints": abc.ints.iter().take(50).collect::<Vec<_>>()});
        h(super::super::util::dumps_compact(&value).as_bytes())
    }
    #[test]
    #[ignore]
    fn real_hud_matches_python() {
        let Some(path) = std::env::var_os("SP_HUD_EXPECTED") else { return };
        let cooked = std::path::Path::new(r"C:\Program Files\Epic Games\rocketleague\TAGame\CookedPCConsole");
        let expected: Value = serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
        let mut packages: HashMap<String, Package> = HashMap::new();
        let movie = |pkg: &Package, i: usize| -> Vec<u8> {
            let (data, props) = export_properties(pkg, pkg.flat().unwrap(), &pkg.exports[i]);
            let raw = props.iter().find(|p| p.name == "RawData" && p.kind == "ArrayProperty").unwrap();
            data[raw.value + 4..raw.value + raw.size].to_vec()
        };
        let mut failures = Vec::new();
        for want in expected["movies"].as_array().unwrap() {
            let file = want["file"].as_str().unwrap().to_owned();
            let pkg = packages.entry(file.clone()).or_insert_with(|| Package::open(&cooked.join(&file)).unwrap());
            let m = movie(pkg, want["export"].as_u64().unwrap() as usize);
            match abc_tag(&m) {
                Ok((.., abc)) => if Some(summary(&abc).as_str()) != want["summary"].as_str() { failures.push(format!("{file} {}: ABC distinto", want["export"])); },
                Err(e) => if Some(e.as_str()) != want["error"].as_str() { failures.push(format!("{file}: {e}")); },
            }
        }
        for want in expected["meter"].as_array().unwrap() {
            let file = want["file"].as_str().unwrap().to_owned();
            let pkg = &packages[&file];
            let got = neutral_boostmeter_gfx(&movie(pkg, want["export"].as_u64().unwrap() as usize));
            let ok = match got { Ok((new, report)) => Some(h(&new).as_str()) == want["sha"].as_str() && report == want["report"], Err(e) => Some(e.as_str()) == want["error"].as_str() };
            if !ok { failures.push(format!("{file}: medidor distinto")); }
        }
        let hud = Package::open(&cooked.join("GFX_Hud_SF.upk")).unwrap();
        let (index, serial, report) = patch_boostmeter_movie(&hud, hud.flat().unwrap()).unwrap();
        if json!(index) != expected["hud"]["index"] || Some(h(&serial).as_str()) != expected["hud"]["sha"].as_str() || report != expected["hud"]["report"] { failures.push("GFX_Hud: parche distinto".into()); }
        println!("{} películas y {} medidores comparados", expected["movies"].as_array().unwrap().len(), expected["meter"].as_array().unwrap().len());
        assert!(failures.is_empty(), "{}", failures.join("\n"));
    }
}
