//! Client presentation edits in the GFx movies: name, title, ranked-menu rank, local avatar frame and banner.
//! Offline file preparation only; never touches the process, the account or the network. Port of
//! engine/fake_display.py, title_display.py, rank_display.py and avatar_display.py.
use super::colors::export_properties;
use super::hud::{abc_tag, encode_u30, instructions, swf_tags, Abc, Reader};
use super::upk::{i32_at, i64_at, put32, Package};
use super::util::R;
use serde_json::{json, Value};
use std::collections::{BTreeMap, BTreeSet, HashMap};

fn controls(c: char) -> bool { let o = c as u32; o < 32 || o == 127 || (0x202a..=0x202e).contains(&o) || (0x2066..=0x2069).contains(&o) }

pub fn validate_name(value: &Value) -> R<String> {
    let text = value.as_str().ok_or("El nombre debe ser texto.")?.trim().to_owned();
    if text.chars().count() > 32 || text.chars().any(controls) { return Err("Usa hasta 32 caracteres, sin controles ni saltos de línea.".into()); }
    Ok(text)
}

fn getter(abc: &Abc, cls: &str, static_: bool, field: &str) -> R<Option<u32>> {
    let Some(record) = abc.class(cls) else { return Ok(None) };
    let traits = if static_ { &record.static_traits } else { &record.traits };
    let found: Vec<u32> = traits.iter().filter(|t| t.kind == 2 && t.name.rsplit('.').next() == Some(field)).filter_map(|t| t.method).collect();
    if found.len() > 1 { return Err("Getter de presentación ambiguo.".into()); }
    Ok(found.first().copied())
}

fn string_index(abc: &Abc, s: &str) -> R<u32> { abc.strings.iter().position(|x| x == s).map(|i| i as u32).ok_or(format!("'{s}' is not in list")) }
fn op(code: u8, arg: u32) -> Vec<u8> { let mut v = vec![code]; v.extend(encode_u30(arg)); v }
fn jump(code: u8, len: usize) -> Vec<u8> { let mut v = vec![code]; v.extend(&(len as u32).to_le_bytes()[..3]); v }
fn splice(abc: &Abc, edits: &mut Vec<(usize, usize, Vec<u8>)>) -> Vec<u8> {
    edits.sort_by(|a, b| b.0.cmp(&a.0).then(b.1.cmp(&a.1)));
    let mut result = abc.data.clone();
    for (a, b, v) in edits.iter() { result.splice(*a..*b, v.iter().copied()); }
    result
}
fn count_edit(abc: &Abc, extra: usize) -> R<(usize, usize, Vec<u8>)> {
    let mut r = Reader { data: &abc.data, pos: abc.string_count_pos };
    r.u30()?;
    Ok((abc.string_count_pos, r.pos, encode_u30((abc.strings.len() + extra) as u32)))
}
/// Replaces chosen instructions by new code and re-targets every branch (s24 jumps and lookupswitch) to the moved code.
fn replace_instructions(code: &[u8], pick: impl Fn(&[u8], &(usize, usize, u8, Vec<u32>)) -> Option<Vec<u8>>) -> R<Vec<u8>> {
    let list = super::hud::instructions(code)?;
    let pieces: Vec<Vec<u8>> = list.iter().map(|i| pick(code, i).unwrap_or_else(|| code[i.0..i.1].to_vec())).collect();
    let mut moved: HashMap<usize, usize> = HashMap::new();
    let mut at = 0;
    for (i, piece) in list.iter().zip(&pieces) { moved.insert(i.0, at); at += piece.len(); }
    moved.insert(code.len(), at);
    let s24 = |b: &[u8]| i32::from_le_bytes([b[0], b[1], b[2], if b[2] & 0x80 != 0 { 0xff } else { 0 }]) as i64;
    let target = |old: i64| moved.get(&usize::try_from(old).unwrap_or(usize::MAX)).copied().ok_or("Salto fuera de una instrucción.");
    let mut out = Vec::with_capacity(at);
    for ((start, end, op, _), piece) in list.iter().zip(pieces) {
        let mut piece = piece;
        let new_start = out.len() as i64;
        if piece[..] == code[*start..*end] {
            if (0x0c..=0x1a).contains(op) {
                let to = target(*end as i64 + s24(&code[start + 1..start + 4]))? as i64;
                piece[1..4].copy_from_slice(&((to - (new_start + 4)) as i32).to_le_bytes()[..3]);
            } else if *op == 0x1b {
                let mut r = Reader { data: code, pos: start + 4 };
                let n = r.u30()? as usize;
                let cases = r.pos - start;
                for k in std::iter::once(1).chain((0..=n).map(|j| cases + j * 3)) {
                    let to = target(*start as i64 + s24(&code[start + k..start + k + 3]))? as i64;
                    piece[k..k + 3].copy_from_slice(&((to - new_start) as i32).to_le_bytes()[..3]);
                }
            }
        }
        out.extend(piece);
    }
    Ok(out)
}

fn body_edit(abc: &Abc, mid: u32, code: &[u8]) -> R<(usize, usize, Vec<u8>)> {
    let b = abc.body(mid).ok_or("Método sin cuerpo.")?;
    let mut v = encode_u30(code.len() as u32);
    v.extend(code);
    Ok((b.lengthpos, b.codepos + b.code.len(), v))
}
/// After an edit only the listed method bodies may differ; classes, methods and contracts stay identical.
fn verify(abc: &Abc, after: &Abc, changed: &HashMap<u32, Vec<u8>>, contract: bool) -> R<()> {
    if after.classes != abc.classes || after.methods != abc.methods { return Err("Cambió la definición de clases.".into()); }
    for (mid, body) in &abc.bodies {
        let new = after.body(*mid).ok_or("Cambió otro método.")?;
        if new.code != *changed.get(mid).unwrap_or(&body.code) { return Err("Cambió otro método.".into()); }
        if contract && (new.stack, new.locals, new.initial_scope, new.max_scope, new.exceptions) != (body.stack, body.locals, body.initial_scope, body.max_scope, body.exceptions) {
            return Err("Cambió una firma de método.".into());
        }
    }
    Ok(())
}

/// Accept only the linear DataAPI read of the exact PlayerName field: (scope end, init end).
fn verified_getter(abc: &Abc, mid: u32, static_: bool, table: &str) -> R<(usize, usize)> {
    let b = abc.body(mid).ok_or("Método sin cuerpo.")?;
    let m = &abc.methods[mid as usize];
    if b.exceptions != 0 || !m.params.is_empty() || abc.name(m.ret) != "String" { return Err("El getter de nombre cambió de firma.".into()); }
    let counts: HashMap<u8, usize> = [(0xd0, 0), (0x30, 0), (0x5d, 1), (0x4f, 2), (0x60, 1), (0x2c, 1), (0x24, 1), (0x66, 1), (0x46, 2), (0x48, 0)].into_iter().collect();
    let mut r = Reader { data: &b.code, pos: 0 };
    let (mut ops, mut ends): (Vec<(u8, Vec<u32>)>, Vec<usize>) = (Vec::new(), Vec::new());
    while r.pos < b.code.len() {
        let o = r.byte()?;
        if o == 0xf0 || o == 0xf1 { r.u30()?; continue; }
        let n = *counts.get(&o).ok_or(format!("Getter de nombre desconocido {o:#x} en {mid}; no se modifica."))?;
        let args = (0..n).map(|_| if o == 0x24 { r.byte().map(|v| v as u32) } else { r.u30() }).collect::<R<Vec<_>>>()?;
        ops.push((o, args));
        ends.push(r.pos);
    }
    let strings: Vec<&str> = ops.iter().filter(|o| o.0 == 0x2c).map(|o| abc.strings[o.1[0] as usize].as_str()).collect();
    if strings != [if static_ { "LocalPlayer" } else { table }, "PlayerName"] || ops.len() < 2 || ops[0] != (0xd0, vec![]) || ops[1] != (0x30, vec![]) {
        return Err("Getter de nombre sin el origen esperado.".into());
    }
    let calls: Vec<(String, u32)> = ops.iter().filter(|o| o.0 == 0x46).map(|o| (abc.name(o.1[0]), o.1[1])).collect();
    if calls != [("getValue".to_string(), 3)] { return Err("El getter no es una lectura de DataAPI.".into()); }
    if !ops.iter().any(|o| o.0 == 0x60 && abc.name(o.1[0]) == "projectx.api.DataAPI") { return Err("No se reconoce DataAPI.".into()); }
    if ops.last() != Some(&(0x48, vec![])) { return Err("Retorno no reconocido.".into()); }
    let expected: &[u8] = if static_ { &[0xd0, 0x30, 0x5d, 0x4f, 0x60, 0x2c, 0x24, 0x2c, 0x46, 0x48] } else { &[0xd0, 0x30, 0x60, 0x2c, 0xd0, 0x66, 0x2c, 0x46, 0x48] };
    if ops.iter().map(|o| o.0).collect::<Vec<_>>() != expected { return Err("Getter de nombre fuera del perfil verificado.".into()); }
    if static_ && (abc.name(ops[2].1[0]) != "init" || abc.name(ops[3].1[0]) != "init" || ops[3].1[1] != 0) { return Err("Inicialización LocalPlayer desconocida.".into()); }
    Ok((ends[1], if static_ { ends[3] } else { 0 }))
}

/// The local player's name everywhere it is read, plus the goal message text.
pub fn patch_name(data: &[u8], name: &str) -> R<(Vec<u8>, Vec<String>)> {
    if name.is_empty() { return Ok((data.to_vec(), Vec::new())); }
    let abc = Abc::parse(data)?;
    let mut edits = Vec::new();
    let mut changed: HashMap<u32, Vec<u8>> = HashMap::new();
    let mut fields = Vec::new();
    let sid = abc.strings.len() as u32;
    for (cls, static_) in [("tagame.api.LocalPlayer", true), ("tagame.api.PlayerInfoRow", false), ("tagame.api.NameplateRow", false)] {
        let Some(mid) = getter(&abc, cls, static_, "PlayerName")? else { continue };
        let (scope_end, init_end) = verified_getter(&abc, mid, static_, if cls.ends_with(".NameplateRow") { "Nameplate" } else { "PlayerInfo" })?;
        let code = &abc.body(mid).unwrap().code;
        let mut over = op(0x2c, sid);
        over.push(0x48);
        let new = if static_ {
            // Retain init() and its registration effects, replacing only the read.
            [&code[..init_end], &over[..]].concat()
        } else {
            if getter(&abc, cls, false, "bLocalPlayer")?.is_none() { return Err("No se puede distinguir al jugador local.".into()); }
            // Read bLocalPlayer; other rows execute their exact original getter.
            let ids: Vec<usize> = (1..abc.multinames.len()).filter(|&i| abc.name(i as u32) == "bLocalPlayer").collect();
            let [id] = ids[..] else { return Err("Campo bLocalPlayer ambiguo.".into()) };
            let mut check = vec![0xd0];
            check.extend(op(0x66, id as u32));
            check.extend(jump(0x12, over.len()));
            [&code[..scope_end], &check[..], &over[..], &code[scope_end..]].concat()
        };
        edits.push(body_edit(&abc, mid, &new)?);
        changed.insert(mid, new);
        fields.push(format!("{cls}.PlayerName"));
    }
    // Goal messages arrive preformatted, bypassing PlayerName getters: replace our original name only.
    let goals: Vec<u32> = abc.class("tagame.hud.GameMessageView").map(|c| c.traits.iter().filter(|t| t.kind == 1 && t.name.ends_with(".handleGameMessageReceived")).filter_map(|t| t.method).collect()).unwrap_or_default();
    // StartMenu embeds this class as unused shared code without String.replace.
    let has_replace = (1..abc.multinames.len()).any(|i| abc.name(i as u32) == "http://adobe.com/AS3/2006/builtin.replace");
    if let ([mid], true) = (&goals[..], has_replace) {
        let body = abc.body(*mid).ok_or("Método sin cuerpo.")?;
        let ops: Vec<_> = instructions(&body.code)?.into_iter().filter(|o| o.2 != 0xf0 && o.2 != 0xf1).collect();
        if body.exceptions != 0 || body.stack < 6 || ops.len() < 2 || ops[0].2 != 0xd0 || ops[1].2 != 0x30 { return Err("El renderizador del mensaje de gol cambió.".into()); }
        let r = |key: &str| abc.reference(key).map(encode_u30);
        let (table, field) = (string_index(&abc, "LocalPlayer")?, string_index(&abc, "PlayerName")?);
        // Our own original name read directly from DataAPI (goal text may already be uppercased).
        let mut original = vec![0x60];
        original.extend(r("projectx.api.DataAPI")?);
        original.extend(op(0x2c, table));
        original.extend([0x24, 0x00]);
        original.extend(op(0x2c, field));
        original.push(0x46);
        original.extend(r("getValue")?);
        original.push(0x03);
        let mut upper = vec![0x46];
        upper.extend(r("http://adobe.com/AS3/2006/builtin.toUpperCase")?);
        upper.push(0x00);
        let mut prefix = original.clone();
        let branch = prefix.len();
        prefix.extend([0x12, 0, 0, 0]);
        prefix.extend([0xd1, 0xd1, 0x66]);
        prefix.extend(r("message")?);
        prefix.extend(&upper);
        prefix.extend(&original);
        prefix.extend(&upper);
        prefix.extend(op(0x2c, sid));
        prefix.push(0x46);
        prefix.extend(r("http://adobe.com/AS3/2006/builtin.replace")?);
        prefix.extend([0x02, 0x61]);
        prefix.extend(r("message")?);
        let offset = (prefix.len() - branch - 4) as u32;
        prefix[branch + 1..branch + 4].copy_from_slice(&offset.to_le_bytes()[..3]);
        let scope_end = ops[1].1;
        let new = [&body.code[..scope_end], &prefix[..], &body.code[scope_end..]].concat();
        edits.push(body_edit(&abc, *mid, &new)?);
        changed.insert(*mid, new);
        fields.push("GameMessageView.localGoalName".into());
    }
    if changed.is_empty() { return Ok((data.to_vec(), Vec::new())); }
    let mut pooled = encode_u30(name.len() as u32);
    pooled.extend(name.as_bytes());
    edits.push((abc.string_pool_end, abc.string_pool_end, pooled));
    edits.push(count_edit(&abc, 1)?);
    let result = splice(&abc, &mut edits);
    let checked = Abc::parse(&result)?;
    if checked.strings[..checked.strings.len() - 1] != abc.strings[..] || checked.strings.last().map(String::as_str) != Some(name) { return Err("Cambió otra constante.".into()); }
    verify(&abc, &checked, &changed, true)?;
    Ok((result, fields))
}

/// Settings of a custom title, or None when switched off.
pub fn normalize_title(value: &Value) -> R<Option<Value>> {
    if value.is_null() || value.as_object().is_some_and(|m| m.is_empty()) || value["enabled"] == false { return Ok(None); }
    let text = value["text"].as_str().unwrap_or("").trim().to_owned();
    if text.is_empty() || text.chars().count() > 48 || text.chars().any(controls) { return Err("El título necesita entre 1 y 48 caracteres sin controles.".into()); }
    let target = value["target"].as_str().filter(|t| !t.is_empty() && t.chars().count() <= 128).ok_or("Elige un título original de tu inventario.")?;
    // bool(value.get(key, default)): a missing key takes the default, an explicit null is false.
    let flag = |key: &str, default: bool| value.get(key).map(truthy).unwrap_or(default);
    let mut out = json!({"enabled": true, "text": text, "target": target, "multicolor": flag("multicolor", false)});
    for (key, default) in [("color", "#75EAA6"), ("color2", "#68DAF9"), ("glow", "#75EAA6")] {
        let color = if value.get(key).is_some() { value[key].as_str().ok_or("Color de título no válido.")? } else { default };
        if color.len() != 7 || !color.starts_with('#') || !color[1..].bytes().all(|b| b.is_ascii_hexdigit()) { return Err("Color de título no válido.".into()); }
        out[key] = color.to_uppercase().into();
    }
    out["glow_enabled"] = flag("glow_enabled", true).into();
    Ok(Some(out))
}

fn truthy(v: &Value) -> bool {
    match v { Value::Null => false, Value::Bool(b) => *b, Value::Number(n) => n.as_f64() != Some(0.), Value::String(s) => !s.is_empty(), Value::Array(a) => !a.is_empty(), Value::Object(o) => !o.is_empty() }
}

fn round_half_even(x: f64) -> i64 { let r = x.round(); if (x - x.trunc()).abs() == 0.5 { (2. * (x / 2.).round()) as i64 } else { r as i64 } }

/// The title as shown: uppercase, or per-letter FONT colours for a gradient.
pub fn title_text(title: &Value) -> String {
    let text = title["text"].as_str().unwrap_or("").to_uppercase();
    if title["multicolor"] != true { return text; }
    let hex = |s: &str| (1..7).step_by(2).map(|i| u8::from_str_radix(&s[i..i + 2], 16).unwrap_or(0)).collect::<Vec<_>>();
    let (a, b) = (hex(title["color"].as_str().unwrap_or("#000000")), hex(title["color2"].as_str().unwrap_or("#000000")));
    let chars: Vec<char> = text.chars().collect();
    chars.iter().enumerate().map(|(i, c)| {
        let t = i as f64 / (chars.len().max(2) - 1) as f64;
        let rgb: String = a.iter().zip(&b).map(|(x, y)| format!("{:02X}", round_half_even(*x as f64 + (*y as f64 - *x as f64) * t))).collect();
        let escaped = match c { '&' => "&#38;".into(), '<' => "&#60;".into(), '>' => "&#62;".into(), '"' => "&#34;".into(), '\'' => "&#39;".into(), c => c.to_string() };
        format!("<FONT COLOR=\"#{rgb}\">{escaped}</FONT>")
    }).collect()
}

/// A title getter: the DataAPI read at RowIndex of one field. Returns its scope end.
fn read_getter(abc: &Abc, mid: u32, table: &str, field: &str, return_type: &str) -> R<usize> {
    let body = abc.body(mid).ok_or("Método sin cuerpo.")?;
    let m = &abc.methods[mid as usize];
    if body.exceptions != 0 || !m.params.is_empty() || abc.name(m.ret) != return_type { return Err("Firma de título no reconocida.".into()); }
    let counts: HashMap<u8, usize> = [(0xd0, 0), (0x30, 0), (0x60, 1), (0x2c, 1), (0x66, 1), (0x46, 2), (0x48, 0)].into_iter().collect();
    let mut r = Reader { data: &body.code, pos: 0 };
    let (mut ops, mut ends): (Vec<(u8, Vec<u32>)>, Vec<usize>) = (Vec::new(), Vec::new());
    while r.pos < body.code.len() {
        let o = r.byte()?;
        if o == 0xf0 || o == 0xf1 { r.u30()?; continue; }
        let n = *counts.get(&o).ok_or("Getter de título fuera del perfil verificado.")?;
        ops.push((o, (0..n).map(|_| r.u30()).collect::<R<_>>()?));
        ends.push(r.pos);
    }
    if ops.iter().map(|o| o.0).collect::<Vec<_>>() != [0xd0, 0x30, 0x60, 0x2c, 0xd0, 0x66, 0x2c, 0x46, 0x48] { return Err("Estructura de título desconocida.".into()); }
    let strings: Vec<&str> = ops.iter().filter(|o| o.0 == 0x2c).map(|o| abc.strings[o.1[0] as usize].as_str()).collect();
    let call = &ops[ops.len() - 2];
    if strings != [table, field] || abc.name(ops[2].1[0]) != "projectx.api.DataAPI" || abc.name(call.1[0]) != "getValue" || call.1[1] != 3 { return Err("Origen de título desconocido.".into()); }
    if abc.name(ops[5].1[0]) != "RowIndex" { return Err("Índice de título no reconocido.".into()); }
    Ok(ends[1])
}

/// Skip debug metadata, then require "getlocal0; pushscope". Returns the scope end.
fn scope_after_header(code: &[u8]) -> R<Option<usize>> {
    let mut r = Reader { data: code, pos: 0 };
    while matches!(code.get(r.pos), Some(0xf0) | Some(0xf1)) { r.byte()?; r.u30()?; }
    Ok(if r.byte()? == 0xd0 && r.byte()? == 0x30 { Some(r.pos) } else { None })
}

pub fn patch_title(data: &[u8], value: &Value) -> R<(Vec<u8>, Vec<String>)> {
    let Some(title) = normalize_title(value)? else { return Ok((data.to_vec(), Vec::new())) };
    let abc = Abc::parse(data)?;
    let mut changed: BTreeMap<u32, Vec<u8>> = BTreeMap::new();
    let mut order: Vec<u32> = Vec::new();
    let mut fields = Vec::new();
    let mut strings: Vec<String> = Vec::new();
    let base = abc.strings.len();
    let mut string = |value: &str, strings: &mut Vec<String>| -> Vec<u8> {
        let i = match strings.iter().position(|s| s == value) { Some(i) => i, None => { strings.push(value.to_owned()); strings.len() - 1 } };
        op(0x2c, (base + i) as u32)
    };
    let prop = |name: &str| abc.reference(name).map(|r| op(0x66, r));
    let lex = |name: &str| abc.reference(name).map(|r| op(0x60, r));
    let multicolor = title["multicolor"] == true;
    let hexint = |key: &str| i64::from_str_radix(&title[key].as_str().unwrap_or("#000000")[1..], 16).unwrap_or(0);
    let colors: HashMap<&str, String> = [("Text", title_text(&title)), ("Color", (if multicolor { 0xffffff } else { hexint("color") }).to_string()),
        ("GlowColor", (if title["glow_enabled"] == true { hexint("glow") } else { 0 }).to_string())].into_iter().collect();
    let mut set = |mid: u32, code: Vec<u8>, changed: &mut BTreeMap<u32, Vec<u8>>, order: &mut Vec<u32>| { if !order.contains(&mid) { order.push(mid); } changed.insert(mid, code); };
    let cls = "tagame.api.PlayerTitlesPlayerTitlesRow";
    for field in ["Text", "Color", "GlowColor"] {
        let Some(mid) = getter(&abc, cls, false, field)? else { continue };
        let scope = read_getter(&abc, mid, "PlayerTitlesPlayerTitles", field, if field == "Text" { "String" } else { "uint" })?;
        let mut over = string(&colors[field], &mut strings);
        if field != "Text" { over.push(0x74); }
        over.push(0x48);
        let mut check = vec![0xd0];
        check.extend(prop("RowIndex")?);
        check.extend(lex("tagame.api.PlayerTitles")?);
        check.extend(prop("SelectedTitle")?);
        check.extend(jump(0x14, over.len()));
        let code = &abc.body(mid).unwrap().code;
        set(mid, [&code[..scope], &check[..], &over[..], &code[scope..]].concat(), &mut changed, &mut order);
        fields.push(format!("{cls}.{field}"));
    }
    // Per-player title display is guarded by bLocalPlayer; PlayerInfo stays plain text (HTML only in local views).
    let cls = "tagame.api.PlayerInfoRow";
    if let Some(xp) = getter(&abc, cls, false, "XPTitle")? {
        read_getter(&abc, xp, "PlayerInfo", "XPTitle", "String")?;
        for (field, key) in [("XPTitle", "Text"), ("TitleColor", "Color"), ("TitleGlowColor", "GlowColor")] {
            let Some(mid) = getter(&abc, cls, false, field)? else { continue };
            let scope = read_getter(&abc, mid, "PlayerInfo", field, if key == "Text" { "String" } else { "uint" })?;
            let value = if key == "Text" { title["text"].as_str().unwrap_or("").to_uppercase() } else { colors[key].clone() };
            let mut over = string(&value, &mut strings);
            if key != "Text" { over.push(0x74); }
            over.push(0x48);
            let mut check = vec![0xd0];
            check.extend(prop("bLocalPlayer")?);
            check.extend(jump(0x12, over.len()));
            let code = &abc.body(mid).unwrap().code;
            set(mid, [&code[..scope], &check[..], &over[..], &code[scope..]].concat(), &mut changed, &mut order);
            fields.push(format!("{cls}.{field}"));
        }
    }
    let cls = "tagame.controls.LocalPlayerView";
    if let Some(record) = abc.class(cls) {
        for t in record.traits.iter().filter(|t| t.kind == 1 && t.name.rsplit('.').next() == Some("updatePlayerTitle")) {
            let mid = t.method.unwrap_or(0);
            let body = abc.body(mid).ok_or("Método sin cuerpo.")?;
            let scope = scope_after_header(&body.code)?;
            let (Some(scope), 0, true) = (scope, body.exceptions, body.stack >= 4) else { return Err("Renderizador de título no reconocido.".into()) };
            let mut over = lex("tagame.managers.PlayerTitleStyleManager")?;
            over.extend(op(0x46, abc.reference("getInstance")?));
            over.push(0x00);
            let field = prop("tagame.controls:LocalPlayerView.rankNameTextField")?;
            over.push(0xd0); over.extend(&field); over.extend(string(&colors["Color"], &mut strings)); over.push(0x74);
            over.extend(string(&colors["GlowColor"], &mut strings)); over.extend([0x74, 0x4f]); over.extend(encode_u30(abc.reference("applyTitleStyle")?)); over.push(0x03);
            over.push(0xd0); over.extend(&field); over.extend(string(&colors["Text"], &mut strings));
            over.extend(op(0x61, abc.reference(if multicolor { "htmlText" } else { "text" })?)); over.push(0x47);
            let preview = "tagame.controls:LocalPlayerView.m_playerTitlePreviewRow";
            let guard = if record.traits.iter().any(|t| t.name == preview) { let mut g = vec![0xd0]; g.extend(prop(preview)?); g.extend(jump(0x11, over.len())); g } else { Vec::new() };
            set(mid, [&body.code[..scope], &guard[..], &over[..], &body.code[scope..]].concat(), &mut changed, &mut order);
            fields.push(format!("{cls}.equippedTitle"));
        }
    }
    let cls = "tagame.controls.ScoreboardRenderer";
    if let Some(record) = abc.class(cls) {
        for t in record.traits.iter().filter(|t| t.kind == 1 && t.name.rsplit('.').next() == Some("setPlayerTitle")) {
            let mid = t.method.unwrap_or(0);
            let body = abc.body(mid).ok_or("Método sin cuerpo.")?;
            let scope = scope_after_header(&body.code)?;
            let params: Vec<String> = abc.methods[mid as usize].params.iter().map(|p| abc.name(*p)).collect();
            let (Some(scope), 0, Some(&0x47), true) = (scope, body.exceptions, body.code.last(), params == ["String"]) else { return Err("El renderizador del título del marcador cambió.".into()) };
            let mut row = vec![0xd0];
            row.extend(prop("m_playerData")?);
            let local_only = |code: Vec<u8>| -> R<Vec<u8>> {
                let mut inner = row.clone();
                inner.extend(prop("bLocalPlayer")?);
                inner.extend(jump(0x12, code.len()));
                inner.extend(code);
                let mut out = row.clone();
                out.extend(jump(0x12, inner.len()));
                out.extend(inner);
                Ok(out)
            };
            // Native layout/style runs with plain text; the HTML gradient is applied afterwards.
            let mut first = string(&title["text"].as_str().unwrap_or("").to_uppercase(), &mut strings);
            first.extend([0x85, 0xd5]);
            let before = local_only(first)?;
            let mut late = vec![0xd0];
            late.extend(prop("xpRankTextField")?);
            late.extend(string(&colors["Text"], &mut strings));
            late.extend(op(0x61, abc.reference("htmlText")?));
            let after = local_only(late)?;
            set(mid, [&body.code[..scope], &before[..], &body.code[scope..body.code.len() - 1], &after[..], &[0x47]].concat(), &mut changed, &mut order);
            fields.push(format!("{cls}.localTitleHtml"));
        }
    }
    if multicolor && (abc.class("tagame.controls.LocalPlayerView").is_some() || abc.class("tagame.controls.TitleListItemRenderer").is_some()) {
        let html_id = abc.reference("htmlText")?;
        let cls = "tagame.controls.TitleListItemRenderer";
        if let Some(record) = abc.class(cls) {
            for t in record.traits.iter().filter(|t| t.kind == 1 && t.name.rsplit('.').next() == Some("setData")) {
                let mid = t.method.unwrap_or(0);
                let body = abc.body(mid).ok_or("Método sin cuerpo.")?;
                if body.exceptions != 0 || body.code.last() != Some(&0x47) { return Err("El renderer de lista ha cambiado.".into()); }
                let row = prop("tagame.controls:TitleListItemRenderer.m_titleRow")?;
                let mut render = lex("textField")?;
                render.push(0xd0); render.extend(&row); render.extend(prop("Text")?);
                render.extend(op(0x46, abc.reference("toUpperCase")?)); render.push(0x00); render.extend(op(0x61, html_id));
                let mut check = vec![0xd0];
                check.extend(&row);
                check.extend(jump(0x12, render.len()));
                set(mid, [&body.code[..body.code.len() - 1], &check[..], &render[..], &[0x47]].concat(), &mut changed, &mut order);
                fields.push(format!("{cls}.htmlText"));
            }
        }
    }
    if changed.is_empty() { return Ok((data.to_vec(), Vec::new())); }
    let mut edits: Vec<(usize, usize, Vec<u8>)> = order.iter().map(|m| body_edit(&abc, *m, &changed[m])).collect::<R<_>>()?;
    let payload: Vec<u8> = strings.iter().flat_map(|s| { let mut v = encode_u30(s.len() as u32); v.extend(s.as_bytes()); v }).collect();
    edits.push((abc.string_pool_end, abc.string_pool_end, payload));
    edits.push(count_edit(&abc, strings.len())?);
    let result = splice(&abc, &mut edits);
    let after = Abc::parse(&result)?;
    if after.strings[..abc.strings.len()] != abc.strings[..] { return Err("Cambió una definición fuera del título.".into()); }
    let changed: HashMap<u32, Vec<u8>> = changed.into_iter().collect();
    verify(&abc, &after, &changed, false).map_err(|_| "Cambió otro método al preparar títulos.".to_string())?;
    Ok((result, fields))
}

const PLAYLISTS: [i64; 7] = [10, 11, 13, 27, 28, 29, 30];

pub fn normalize_rank(value: &Value) -> R<Option<Value>> {
    if !value.is_null() && !value.is_object() { return Err("Configuración de rango no válida.".into()); }
    if value.is_null() || value.as_object().is_some_and(|m| m.is_empty()) || value["enabled"] == false { return Ok(None); }
    let mut result = json!({"enabled": true});
    for (name, low, high) in [("playlist", 0, 100), ("mmr", 0, 10000), ("tier", 0, 22), ("division", 0, 3)] {
        let n = value[name].as_i64().filter(|_| value[name].is_i64() || value[name].is_u64()).filter(|n| (low..=high).contains(n)).ok_or(format!("Rango visual no válido: {name}"))?;
        result[name] = n.into();
    }
    if !PLAYLISTS.contains(&result["playlist"].as_i64().unwrap()) { return Err("Modalidad de rango no disponible.".into()); }
    Ok(Some(result))
}

/// Ranked-menu getters return the chosen tier, division and rating for one playlist.
pub fn patch_rank(data: &[u8], value: &Value) -> R<(Vec<u8>, Vec<String>)> {
    let Some(rank) = normalize_rank(value)? else { return Ok((data.to_vec(), Vec::new())) };
    let abc = Abc::parse(data)?;
    let tier = rank["tier"].as_i64().unwrap();
    let (division, mmr, playlist) = (rank["division"].as_i64().unwrap(), rank["mmr"].as_i64().unwrap(), rank["playlist"].as_i64().unwrap());
    let overrides: [(&str, Value); 6] = [("SkillTier", tier.into()), ("PrevSkillTier", tier.into()), ("SkillDivision", division.into()), ("PrevSkillDivision", division.into()), ("SkillMMR", mmr.into()), ("bCompletedPlacementMatches", (tier != 0).into())];
    let mut changes: HashMap<u32, Vec<u8>> = HashMap::new();
    let mut fields = Vec::new();
    let mut edits = Vec::new();
    for (cls, static_) in [("tagame.api.Playlists", true), ("tagame.api.PlaylistsRow", false)] {
        let Some(record) = abc.class(cls) else { continue };
        for t in if static_ { &record.static_traits } else { &record.traits } {
            let name = t.name.rsplit('.').next().unwrap_or("");
            let field = if static_ && name.starts_with("Get") { &name[3..] } else { name };
            let Some(val) = overrides.iter().find(|o| o.0 == field).map(|o| o.1.clone()) else { continue };
            if t.kind != if static_ { 1 } else { 2 } { continue; }
            let mid = t.method.unwrap_or(0);
            let body = abc.body(mid).ok_or("Método sin cuerpo.")?;
            let method = &abc.methods[mid as usize];
            let expected_return = if field.starts_with('b') { "Boolean" } else if field == "SkillMMR" { "Number" } else { "int" };
            let params: Vec<String> = method.params.iter().map(|p| abc.name(*p)).collect();
            if body.exceptions != 0 || params != if static_ { vec!["int".to_string()] } else { vec![] } || abc.name(method.ret) != expected_return { return Err("Firma de rango modificada; no se parchea.".into()); }
            // Debug metadata differs between menus: preserved, but excluded from the shape check.
            let ops: Vec<_> = instructions(&body.code)?.into_iter().filter(|o| ![0xef, 0xf0, 0xf1].contains(&o.2)).collect();
            let expected: &[u8] = if static_ { &[0xd0, 0x30, 0x60, 0x2c, 0xd1, 0x2c, 0x46, 0x48] } else { &[0xd0, 0x30, 0x60, 0x2c, 0xd0, 0x66, 0x2c, 0x46, 0x48] };
            if ops.iter().map(|o| o.2).collect::<Vec<_>>() != expected {
                return Err(format!("Getter de rango fuera del perfil verificado: {cls}.{name} {}", ops.iter().map(|o| format!("{:#x}", o.2)).collect::<Vec<_>>().join(",")));
            }
            let strings: Vec<&str> = ops.iter().filter(|o| o.2 == 0x2c).map(|o| abc.strings[o.3[0] as usize].as_str()).collect();
            let call = &ops[ops.len() - 2];
            if strings != ["Playlists", field] || abc.name(ops[2].3[0]) != "projectx.api.DataAPI" || abc.name(call.3[0]) != "getValue" || call.3[1] != 3 { return Err("El getter no es una lectura de presentación de Playlists.".into()); }
            if !static_ && abc.name(ops[5].3[0]) != "RowIndex" { return Err("Índice de fila desconocido.".into()); }
            // Match the native playlist Id, not its incidental row position.
            let row = if static_ { vec![0xd1] } else { let mut v = vec![0xd0]; v.extend(op(0x66, abc.reference("RowIndex")?)); v };
            let mut guard = op(0x60, abc.reference("projectx.api.DataAPI")?);
            guard.extend(op(0x2c, string_index(&abc, "Playlists")?));
            guard.extend(&row);
            guard.extend(op(0x2c, string_index(&abc, "Id")?));
            guard.extend(op(0x46, abc.reference("getValue")?));
            guard.push(0x03);
            guard.extend(op(0x25, playlist as u32));
            let mut replacement = match &val { Value::Bool(b) => vec![if *b { 0x26 } else { 0x27 }], v => op(0x25, v.as_i64().unwrap() as u32) };
            if field == "SkillMMR" {
                // Rankings.convertMMR displays round(native * 20 + 100): return its inverse.
                replacement.extend(op(0x25, 100));
                replacement.push(0xa1);
                replacement.extend(op(0x25, 20));
                replacement.push(0xa3);
            }
            replacement.push(0x48);
            let mut prefix = guard;
            prefix.extend(jump(0x14, replacement.len()));
            prefix.extend(&replacement);
            let end = ops[1].1;
            let code = [&body.code[..end], &prefix[..], &body.code[end..]].concat();
            if body.stack < 4 { return Err("Pila de rango no reconocida.".into()); }
            edits.push(body_edit(&abc, mid, &code)?);
            changes.insert(mid, code);
            fields.push(format!("{cls}.{name}"));
        }
    }
    if changes.is_empty() { return Ok((data.to_vec(), Vec::new())); }
    let result = splice(&abc, &mut edits);
    let checked = Abc::parse(&result)?;
    if checked.methods != abc.methods || checked.classes != abc.classes || checked.strings != abc.strings { return Err("Cambió la definición de rango.".into()); }
    for (mid, body) in &abc.bodies {
        let after = checked.body(*mid).ok_or("Cambió un método ajeno al rango.")?;
        if after.code != *changes.get(mid).unwrap_or(&body.code) { return Err("Cambió un método ajeno al rango.".into()); }
        if (after.stack, after.locals, after.initial_scope, after.max_scope, after.exceptions) != (body.stack, body.locals, body.initial_scope, body.max_scope, body.exceptions) { return Err("Cambió el contrato del getter.".into()); }
        if changes.contains_key(mid) { instructions(&after.code)?; }
    }
    Ok((result, fields))
}

const FRAME: &str = "swap_prism_local";
const SIDE: u32 = 128; // source pixels; the shape stays 64 px (1280 twips)

fn swf_tag(code: u16, data: &[u8]) -> Vec<u8> {
    let mut out = ((code << 6) | 63).to_le_bytes().to_vec();
    out.extend((data.len() as u32).to_le_bytes());
    out.extend(data);
    out
}

struct Bits(Vec<u8>);
impl Bits {
    fn put(&mut self, value: i64, count: u32) { for i in (0..count).rev() { self.0.push(((value >> i) & 1) as u8); } }
    fn finish(mut self) -> Vec<u8> {
        while self.0.len() % 8 != 0 { self.0.push(0); }
        self.0.chunks(8).map(|c| c.iter().enumerate().fold(0u8, |a, (j, b)| a | (b << (7 - j)))).collect()
    }
}

/// DefineBitsLossless2 + DefineShape3 drawing the picture as a 64 px square (Adobe SWF 10 encoding).
fn bitmap_shape(bitmap: u16, shape: u16, raw: &[u8]) -> R<Vec<u8>> {
    let image = super::dxt::fit(&super::dxt::load(raw)?, SIDE, SIDE);
    let mut pixels = Vec::with_capacity((SIDE * SIDE * 4) as usize);
    for p in image.pixels() {
        let a = p[3] as u32;
        pixels.extend([p[3], ((p[0] as u32 * a + 127) / 255) as u8, ((p[1] as u32 * a + 127) / 255) as u8, ((p[2] as u32 * a + 127) / 255) as u8]);
    }
    let mut head = bitmap.to_le_bytes().to_vec();
    head.push(5);
    head.extend((SIDE as u16).to_le_bytes());
    head.extend((SIDE as u16).to_le_bytes());
    head.extend(super::colors::zlib(&pixels, 6)?);
    let bitmap_tag = swf_tag(36, &head);
    let mut rect = Bits(Vec::new());
    rect.put(12, 5);
    for v in [0, 1280, 0, 1280] { rect.put(v, 12); }
    let mut matrix = Bits(Vec::new());
    matrix.put(1, 1); matrix.put(22, 5); matrix.put(((1280i64) << 16) / SIDE as i64, 22); matrix.put(((1280i64) << 16) / SIDE as i64, 22); matrix.put(0, 1); matrix.put(0, 5);
    let mut edges = Bits(Vec::new());
    edges.put(1, 4); edges.put(0, 4); // one fill index bit, no lines
    edges.put(0, 1); edges.put(5, 5); edges.put(1, 5); edges.put(0, 1); edges.put(0, 1); edges.put(1, 1);
    for (vertical, delta) in [(false, 1280), (true, 1280), (false, -1280), (true, -1280)] {
        edges.put(1, 1); edges.put(1, 1); edges.put(10, 4); edges.put(0, 1); edges.put(vertical as i64, 1); edges.put(delta, 12);
    }
    edges.put(0, 6);
    let mut body = shape.to_le_bytes().to_vec();
    body.extend(rect.finish());
    body.extend([0x01, 0x41]);
    body.extend(bitmap.to_le_bytes());
    body.extend(matrix.finish());
    body.push(0);
    body.extend(edges.finish());
    Ok([bitmap_tag, swf_tag(32, &body)].concat())
}

/// A new last frame on the shared TintedAvatarIcons sprite showing the picture; all native frames stay byte exact.
fn add_local_frame(movie: &[u8], raw: &[u8]) -> R<(Vec<u8>, bool)> {
    let records = swf_tags(movie)?;
    let mut symbol = None;
    for &(code, _, body, end) in &records {
        if code != 76 { continue; }
        let data = &movie[body..end];
        let mut at = 2;
        for _ in 0..u16::from_le_bytes([data[0], data[1]]) {
            let ident = u16::from_le_bytes([data[at], data[at + 1]]);
            at += 2;
            let stop = data[at..].iter().position(|b| *b == 0).map(|i| at + i).ok_or("SymbolClass dañado.")?;
            if &data[at..stop] == b"TintedAvatarIcons" { symbol = Some(ident); }
            at = stop + 1;
        }
    }
    let Some(symbol) = symbol else { return Ok((movie.to_vec(), false)) };
    let candidates: Vec<_> = records.iter().filter(|r| r.0 == 39 && u16::from_le_bytes([movie[r.2], movie[r.2 + 1]]) == symbol).collect();
    let [&(_, start, body, end)] = candidates[..] else { return Err("No se identifica el sprite de avatar compartido.".into()) };
    let data = &movie[body..end];
    if data.windows(FRAME.len() + 1).any(|w| &w[..FRAME.len()] == FRAME.as_bytes() && w[FRAME.len()] == 0) { return Err("El sprite ya contiene una capa de avatar.".into()); }
    let used: BTreeSet<u16> = records.iter().filter(|r| r.3 - r.2 >= 2).map(|r| u16::from_le_bytes([movie[r.2], movie[r.2 + 1]])).collect();
    let (bitmap, shape) = (65000u16, 65001u16);
    if used.contains(&bitmap) || used.contains(&shape) { return Err("Identificadores de avatar ocupados.".into()); }
    // The existing default placement's matrix determines the exact avatar size.
    let (mut at, mut matrix, mut depth, mut end_at) = (4usize, None, 0u16, None);
    while at < data.len() {
        let record_at = at;
        let head = u16::from_le_bytes([data[at], data[at + 1]]);
        at += 2;
        let (code, mut n) = (head >> 6, (head & 63) as usize);
        if n == 63 { n = u32::from_le_bytes(data[at..at + 4].try_into().unwrap()) as usize; at += 4; }
        let value = &data[at..at + n];
        at += n;
        if code == 70 && matrix.is_none() {
            if value.get(..2) != Some(&[0x06, 0x10]) { return Err("La colocación nativa del avatar cambió.".into()); }
            depth = u16::from_le_bytes([value[2], value[3]]);
            matrix = Some(value[6..].to_vec());
        }
        if code == 0 { end_at = Some(record_at); break; }
    }
    let (Some(matrix), Some(end_at)) = (matrix, end_at) else { return Err("Timeline de avatar incompleto.".into()) };
    let mut place = vec![0x06];
    place.extend(depth.to_le_bytes());
    place.extend(shape.to_le_bytes());
    place.extend(&matrix);
    let mut label = FRAME.as_bytes().to_vec();
    label.push(0);
    let extra = [swf_tag(28, &depth.to_le_bytes()), swf_tag(43, &label), swf_tag(26, &place), swf_tag(1, &[]), swf_tag(0, &[])].concat();
    let mut new = symbol.to_le_bytes().to_vec();
    new.extend((u16::from_le_bytes([data[2], data[3]]) + 1).to_le_bytes());
    new.extend(&data[4..end_at]);
    new.extend(extra);
    let mut result = movie[..start].to_vec();
    result.extend(bitmap_shape(bitmap, shape, raw)?);
    result.extend(swf_tag(39, &new));
    result.extend(&movie[end..]);
    let total = result.len() as u32;
    result[4..8].copy_from_slice(&total.to_le_bytes());
    swf_tags(&result)?;
    Ok((result, true))
}

/// The local player's avatar shows the embedded frame without the team tint.
fn patch_avatar(data: &[u8]) -> R<(Vec<u8>, Vec<String>)> {
    let abc = Abc::parse(data)?;
    let Some(cls) = abc.class("tagame.controls.PlayerAvatar") else { return Ok((data.to_vec(), Vec::new())) };
    let methods: Vec<u32> = cls.traits.iter().filter(|t| t.kind == 1 && t.name.ends_with(".validateAvatarIcon")).filter_map(|t| t.method).collect();
    let Some(&mid) = methods.first() else { return Ok((data.to_vec(), Vec::new())) };
    let body = abc.body(mid).ok_or("Método sin cuerpo.")?;
    let local_id = getter(&abc, "tagame.api.LocalPlayer", true, "PlayerID")?;
    let mut r = Reader { data: &body.code, pos: 0 };
    while matches!(body.code.get(r.pos), Some(0xf0) | Some(0xf1)) { r.byte()?; r.u30()?; }
    let header_ok = r.byte()? == 0xd0 && r.byte()? == 0x30;
    if methods.len() != 1 || local_id.is_none_or(|id| abc.name(abc.methods[id as usize].ret) != "String") || body.exceptions != 0 || !header_ok {
        return Err("No se puede verificar la identidad local del avatar.".into());
    }
    let scope = r.pos;
    let prop = |name: &str| abc.reference(name).map(|r| op(0x66, r));
    let setprop = |name: &str| abc.reference(name).map(|r| op(0x61, r));
    let mut tinted = vec![0xd0]; tinted.extend(prop("tintedAvatarIcons")?);
    let mut loader = vec![0xd0]; loader.extend(prop("avatarIconLoader")?);
    // The team colour is applied to the whole sprite; the photo keeps its own colours.
    let mut untint = tinted.clone();
    untint.extend(prop("transform")?); untint.extend(&loader); untint.extend(prop("transform")?); untint.extend(prop("colorTransform")?); untint.extend(setprop("colorTransform")?);
    let mut render = loader.clone();
    render.push(0x27); render.extend(setprop("visible")?);
    render.extend(&tinted); render.push(0x26); render.extend(setprop("visible")?);
    render.extend(&tinted); render.extend(op(0x2c, abc.strings.len() as u32)); render.extend(op(0x4f, abc.reference("gotoAndStop")?)); render.push(0x01);
    render.extend(&untint); render.push(0x47);
    let guard = |check: &[u8], code: &[u8]| [check, &jump(0x12, code.len()), code].concat();
    // The class can be shared with movies without the photo frame (the goal replay loads the shell's PlayerAvatar):
    // there the icon must stay native, or it shows as an empty black avatar. The frame is the sprite's last one.
    let mut untint = untint;
    if let (Ok(total), Ok(label), Ok(goto)) = (abc.reference("totalFrames"), abc.reference("currentLabel"), abc.reference("gotoAndStop")) {
        let has_frame = |code: &[u8]| -> Vec<u8> {
            let mut c = tinted.clone(); c.extend(op(0x66, label)); c.extend(op(0x2c, abc.strings.len() as u32));
            c.extend(jump(0x14, code.len())); // ifne
            c.extend(code);
            c
        };
        let mut seek = tinted.clone(); seek.extend(&tinted); seek.extend(op(0x66, total)); seek.extend(op(0x4f, goto)); seek.push(0x01);
        render = [seek, has_frame(&render)].concat();
        untint = has_frame(&untint);
    }
    let mut row = vec![0xd0]; row.extend(prop("tagame.controls:PlayerAvatar.m_playerInfoRow")?);
    let mut ident = vec![0xd0]; ident.extend(prop("tagame.controls:PlayerAvatar.m_playerId")?);
    // With a player id only the id decides; bLocalPlayer is a fallback for rows without one.
    let local_only = |code: &[u8]| -> R<Vec<u8>> {
        let fallback = guard(&row, &guard(&[&row[..], &prop("bLocalPlayer")?].concat(), code));
        let skip = jump(0x10, fallback.len());
        let mut compare = ident.clone();
        compare.extend(op(0x60, abc.reference("tagame.api.LocalPlayer")?));
        compare.extend(prop("PlayerID")?);
        compare.extend(jump(0x14, code.len() + skip.len() + fallback.len()));
        compare.extend(code);
        compare.extend(&skip);
        Ok([guard(&ident, &compare), fallback].concat())
    };
    let code = [&body.code[..scope], &local_only(&render)?[..], &body.code[scope..]].concat();
    let mut edits = vec![body_edit(&abc, mid, &code)?];
    let mut pooled = encode_u30(FRAME.len() as u32);
    pooled.extend(FRAME.as_bytes());
    edits.push((abc.string_pool_end, abc.string_pool_end, pooled));
    let mut changed: HashMap<u32, Vec<u8>> = HashMap::from([(mid, code)]);
    let tint: Vec<u32> = cls.traits.iter().filter(|t| t.kind == 1 && t.name.ends_with(".updateTeamColor")).filter_map(|t| t.method).collect();
    if let [t] = tint[..] {
        let tbody = abc.body(t).ok_or("Método sin cuerpo.")?;
        if tbody.code.last() == Some(&0x47) && tbody.exceptions == 0 {
            // Runs after every team-colour update, just before the final returnvoid.
            let tcode = [&tbody.code[..tbody.code.len() - 1], &local_only(&[&untint[..], &[0x47]].concat())?[..], &[0x47]].concat();
            edits.push(body_edit(&abc, t, &tcode)?);
            changed.insert(t, tcode);
        }
    }
    // The "Joining/Ready" list tweens the icon sprite's colour; with the photo shown, tween the hidden loader instead.
    let waiting: Vec<u32> = abc.class("tagame.hud.WaitingForPlayerRenderer").into_iter().flat_map(|c| c.traits.iter())
        .filter(|t| t.kind == 1 && t.name.ends_with(".setConnectionStatus")).filter_map(|t| t.method).collect();
    if let [w] = waiting[..] {
        let wbody = abc.body(w).ok_or("Método sin cuerpo.")?;
        let tinted_ref = abc.reference("tintedAvatarIcons")?;
        let mut swap = vec![0x2a]; // dup
        swap.extend(op(0x66, tinted_ref));
        swap.extend(op(0x66, abc.reference("currentLabel")?));
        swap.extend(op(0x2c, abc.strings.len() as u32));
        let photo = op(0x66, abc.reference("avatarIconLoader")?);
        let original = op(0x66, tinted_ref);
        swap.extend(jump(0x14, photo.len() + 4)); // ifne: not the photo frame
        swap.extend(&photo);
        swap.extend(jump(0x10, original.len()));
        swap.extend(&original);
        let wcode = replace_instructions(&wbody.code, |code, (s, e, _, _)| (code[*s] == 0x66 && code[*s..*e] == original[..]).then(|| swap.clone()))?;
        if wbody.exceptions == 0 && wcode != wbody.code {
            edits.push(body_edit(&abc, w, &wcode)?);
            let mut r = Reader { data: &abc.data, pos: wbody.stackpos };
            r.u30()?;
            edits.push((wbody.stackpos, r.pos, encode_u30(wbody.stack + 2)));
            changed.insert(w, wcode);
        }
    }
    edits.push(count_edit(&abc, 1)?);
    let result = splice(&abc, &mut edits);
    let after = Abc::parse(&result)?;
    if after.classes != abc.classes || after.methods != abc.methods { return Err("Cambió una definición del avatar.".into()); }
    for (m, old) in &abc.bodies { if after.body(*m).map(|b| &b.code) != Some(changed.get(m).unwrap_or(&old.code)) { return Err("Cambió otro método del avatar.".into()); } }
    let mut fields = vec!["tagame.controls.PlayerAvatar.localOnlyImage".to_string()];
    if changed.len() > 1 { fields.push("tagame.controls.PlayerAvatar.localUntinted".into()); }
    Ok((result, fields))
}

/// Every presentation patch for one GFx movie: (movie, fields, features).
pub fn patch_movie(movie: &[u8], name: &str, title: &Value, avatar: Option<&[u8]>, rank: &Value) -> R<(Vec<u8>, Vec<String>, BTreeSet<String>)> {
    let mut movie = movie.to_vec();
    let mut avatar_fields = Vec::new();
    if let Some(picture) = avatar {
        let (m, embedded) = add_local_frame(&movie, picture)?;
        movie = m;
        if embedded { avatar_fields.push("TintedAvatarIcons.localFrame".to_string()); }
    }
    let (start, body, end, pos, _) = abc_tag(&movie)?;
    let (new, title_fields) = patch_title(&movie[pos..end], title)?;
    let (mut new, mut fields) = patch_name(&new, name)?;
    let mut features = BTreeSet::new();
    if !fields.is_empty() { features.insert("name".to_string()); }
    if !title_fields.is_empty() { features.insert("title".into()); }
    fields.extend(title_fields);
    if !rank.is_null() && !rank.as_object().is_some_and(|m| m.is_empty()) {
        let (n, rank_fields) = patch_rank(&new, rank)?;
        new = n;
        if !rank_fields.is_empty() { features.insert("rank".into()); }
        fields.extend(rank_fields);
    }
    if avatar.is_some() {
        let (n, scoped) = patch_avatar(&new)?;
        new = n;
        if !scoped.is_empty() || !avatar_fields.is_empty() { features.insert("avatar".into()); }
        fields.extend(scoped);
    }
    fields.extend(avatar_fields);
    if fields.is_empty() { return Ok((movie, fields, features)); }
    let mut payload = movie[body..pos].to_vec();
    payload.extend(&new);
    let mut result = movie[..start].to_vec();
    result.extend(((82u16 << 6) | 63).to_le_bytes());
    result.extend((payload.len() as u32).to_le_bytes());
    result.extend(&payload);
    result.extend(&movie[end..]);
    let total = result.len() as u32;
    result[4..8].copy_from_slice(&total.to_le_bytes());
    abc_tag(&result)?;
    Ok((result, fields, features))
}

pub fn is_banner_item(pkg: &Package) -> bool { pkg.exports.iter().any(|e| pkg.class_name(e) == "ProductAsset_PlayerBanner_TA") }

/// The package with every presentation change, or None when nothing applies; never installs it.
pub fn patch_package(pkg: &Package, name: &str, images: &[(String, Vec<u8>)], title: &Value, avatar: Option<&[u8]>, rank: &Value) -> R<Option<(Vec<u8>, Value)>> {
    let name = validate_name(&Value::String(name.into()))?;
    let flat = pkg.flat()?;
    let mut replacements: BTreeMap<usize, Vec<u8>> = BTreeMap::new();
    let mut movies = BTreeSet::new();
    let (mut fields, mut paths, mut features) = (Vec::new(), Vec::new(), BTreeSet::new());
    let mut avatar = avatar.or(images.iter().find(|i| i.0 == "avatar").map(|i| i.1.as_slice()));
    // The goal replay HUD flags the replayed player as local, which tinted other players' photos black.
    if pkg.file.file_name().is_some_and(|f| f.to_string_lossy().to_lowercase() == "gfx_replayhud_sf.upk") { avatar = None; }
    let titled = !title.is_null() && !title.as_object().is_some_and(|m| m.is_empty());
    let ranked = !rank.is_null() && !rank.as_object().is_some_and(|m| m.is_empty());
    if !name.is_empty() || titled || avatar.is_some() || ranked {
        for (i, e) in pkg.exports.iter().enumerate() {
            if pkg.class_name(e) != "SwfMovie" { continue; }
            let (data, props) = export_properties(pkg, flat, e);
            let Some(raw) = props.iter().find(|p| p.name == "RawData" && p.kind == "ArrayProperty") else { continue };
            if i32_at(data, raw.value) as usize != raw.size - 4 { return Err("RawData no válido.".into()); }
            let at = raw.value + 4;
            let movie = &data[at..at + raw.size - 4];
            // Some static GFX have no ActionScript; leave them intact.
            if !swf_tags(movie)?.iter().any(|t| t.0 == 82) { continue; }
            let (new, changed, found) = patch_movie(movie, &name, title, avatar, rank)?;
            features.extend(found);
            if changed.is_empty() { continue; }
            let mut serial = [&data[..at], &new[..], &data[at + movie.len()..]].concat();
            put32(&mut serial, raw.value - 8, 4 + new.len() as i32);
            put32(&mut serial, raw.value, new.len() as i32);
            if let Some(check) = props.iter().find(|p| (p.name == "RawDataCRC" || p.name == "SourceFileCRC") && p.size == 4) {
                let pos = (check.value as i64 + if check.value > at { new.len() as i64 - movie.len() as i64 } else { 0 }) as usize;
                serial[pos..pos + 4].copy_from_slice(&crc32fast::hash(&new).to_le_bytes());
            }
            replacements.insert(i, serial);
            movies.insert(i);
            fields.extend(changed);
            paths.push(pkg.path(i)?);
        }
    }
    for (key, raw) in images {
        if key == "avatar" { continue; } // Embedded local frame; never replace the shared fallback.
        let startup = pkg.file.file_name().is_some_and(|f| f.to_string_lossy().to_lowercase() == "startup.upk");
        // Startup also holds a banner product asset: there the shared default banner is the target, not its largest texture.
        let found: Vec<usize> = if key == "banner" && is_banner_item(pkg) && !startup {
            // The equipped banner item shows its own artwork: replace its main texture.
            let art: Vec<usize> = (0..pkg.exports.len()).filter(|&i| pkg.class_name(&pkg.exports[i]) == "Texture2D" && pkg.path(i).is_ok_and(|p| !p.rsplit('.').next().unwrap_or("").starts_with("Thumb_"))).collect();
            if art.is_empty() { return Err("El banner equipado no tiene una textura reconocible.".into()); }
            vec![*art.iter().rev().max_by_key(|&&i| i32_at(&pkg.exports[i], 32)).unwrap()]
        } else {
            let allowed = match key.as_str() { "banner" => "gfx_shared.PlayerBanner_Default", _ => "" };
            if pkg.file.file_name().is_none_or(|f| f.to_string_lossy().to_lowercase() != "startup.upk") || allowed.is_empty() { return Err("Textura de perfil no permitida.".into()); }
            (0..pkg.exports.len()).filter(|&i| pkg.class_name(&pkg.exports[i]) == "Texture2D" && pkg.path(i).is_ok_and(|p| p == allowed)).collect()
        };
        let [i] = found[..] else { return Err("No se localiza la textura exacta del perfil.".into()) };
        let e = &pkg.exports[i];
        let serial = &flat[i64_at(e, 36) as usize..(i64_at(e, 36) + i32_at(e, 32) as i64) as usize];
        replacements.insert(i, super::textures::inline_serial(pkg, serial, raw, true)?);
        paths.push(pkg.path(i)?);
        features.insert(key.clone());
    }
    if replacements.is_empty() { return Ok(None); }
    let validator = |i: usize, serial: &[u8]| movies.contains(&i) && replacements.get(&i).is_some_and(|r| r == serial);
    let result = super::textures::append_exports(pkg, &replacements, Some(&validator), None)?;
    let fields: BTreeSet<String> = fields.into_iter().collect();
    Ok(Some((result, json!({"only_client_presentation": true, "other_payloads_unchanged": true, "fields": fields, "exports": paths, "features": features, "visual_verified": false}))))
}

/// SP_DISPLAY_EXPECTED=<json from the Python side> cargo test -- --ignored real_display
#[cfg(test)]
mod parity {
    use super::*;
    use sha2::{Digest, Sha256};
    fn h(b: &[u8]) -> String { format!("{:x}", Sha256::digest(b)) }
    #[test]
    #[ignore]
    fn real_display_matches_python() {
        let Some(path) = std::env::var_os("SP_DISPLAY_EXPECTED") else { return };
        let cooked = std::path::Path::new(r"C:\Program Files\Epic Games\rocketleague\TAGame\CookedPCConsole");
        let expected: Value = serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
        let s = &expected["settings"];
        let name = s["name"].as_str().unwrap_or("");
        let avatar = expected["avatar"].as_str().map(|p| std::fs::read(p).unwrap());
        let mut failures = Vec::new();
        for want in expected["packages"].as_array().unwrap() {
            let file = want["file"].as_str().unwrap();
            let pkg = Package::open(&cooked.join(file)).unwrap();
            let plain = match patch_package(&pkg, name, &[], &s["title"], None, &s["rank"]) {
                Ok(None) => Value::Null,
                Ok(Some((raw, v))) => json!({"flat": h(Package::from_raw(&pkg.file, raw).unwrap().flat().unwrap()), "validation": v}),
                Err(e) => json!({"error": e}),
            };
            if plain != want["plain"] { failures.push(format!("{file}: sin foto distinto | rust {}", plain.get("validation").or(plain.get("error")).unwrap_or(&Value::Null))); }
            if let (Some(wa), Some(picture)) = (want.get("avatar"), &avatar) {
                let got = match patch_package(&pkg, name, &[], &s["title"], Some(picture), &s["rank"]) {
                    Ok(Some((raw, v))) => {
                        let done = Package::from_raw(&pkg.file, raw).unwrap();
                        let flat = done.flat().unwrap();
                        let mut abcs = Vec::new();
                        for (i, e) in done.exports.iter().enumerate() {
                            if i >= pkg.exports.len() || done.class_name(e) != "SwfMovie" { continue; }
                            let (data, props) = export_properties(&done, flat, e);
                            let Some(rd) = props.iter().find(|p| p.name == "RawData" && p.kind == "ArrayProperty") else { continue };
                            let movie = &data[rd.value + 4..rd.value + rd.size];
                            if let Ok((_, _, end, pos, _)) = abc_tag(movie) { abcs.push(h(&movie[pos..end])); }
                        }
                        json!({"abcs": abcs, "validation": v})
                    }
                    Ok(None) => Value::Null,
                    Err(e) => json!({"error": e}),
                };
                if got != *wa { failures.push(format!("{file}: con foto distinto | rust {}", got.get("validation").or(got.get("error")).unwrap_or(&Value::Null))); }
            }
        }
        println!("{} paquetes comparados", expected["packages"].as_array().unwrap().len());
        assert!(failures.is_empty(), "{}", failures.join("\n"));
    }

    /// Every branch of every method lands on an instruction.
    fn branches_ok(code: &[u8]) -> bool {
        let Ok(list) = crate::engine::hud::instructions(code) else { return false };
        let starts: std::collections::HashSet<usize> = list.iter().map(|i| i.0).chain(Some(code.len())).collect();
        let s24 = |b: &[u8]| i32::from_le_bytes([b[0], b[1], b[2], if b[2] & 0x80 != 0 { 0xff } else { 0 }]) as i64;
        list.iter().all(|(s, e, op, _)| !(0x0c..=0x1a).contains(op) || starts.contains(&((*e as i64 + s24(&code[s + 1..s + 4])) as usize)))
    }

    /// Read only: SP_BACKUPS=<data\backups> SP_PICTURE=<png> cargo test -- --ignored real_avatar_patch_is_valid
    #[test]
    #[ignore]
    fn real_avatar_patch_is_valid() {
        let folder = std::path::PathBuf::from(std::env::var("SP_BACKUPS").unwrap());
        let picture = std::fs::read(std::env::var("SP_PICTURE").unwrap()).unwrap();
        let (mut checked, mut waiting, mut rewritten) = (0, 0, 0);
        let mut original_bodies: HashMap<(String, usize, u32), Vec<u8>> = HashMap::new();
        for entry in std::fs::read_dir(&folder).unwrap().flatten() {
            let file = entry.file_name().to_string_lossy().to_string();
            if !file.to_lowercase().starts_with("gfx") { continue; }
            let pkg = Package::open(&entry.path()).unwrap();
            for (x, e) in pkg.exports.iter().enumerate().filter(|e| pkg.class_name(e.1) == "SwfMovie") {
                let (data, props) = export_properties(&pkg, pkg.flat().unwrap(), e);
                let Some(rd) = props.iter().find(|p| p.name == "RawData" && p.kind == "ArrayProperty") else { continue };
                let movie = &data[rd.value + 4..rd.value + rd.size];
                let Ok((_, _, end, pos, _)) = abc_tag(movie) else { continue };
                for (m, b) in &Abc::parse(&movie[pos..end]).unwrap().bodies { original_bodies.insert((file.clone(), x, *m), b.code.clone()); }
            }
            let Some((raw, _)) = patch_package(&pkg, "Prueba", &[], &Value::Null, Some(&picture), &Value::Null).unwrap_or_else(|e| panic!("{file}: {e}")) else { continue };
            let done = Package::from_raw(&pkg.file, raw).unwrap();
            let flat = done.flat().unwrap();
            for (x, e) in done.exports.iter().enumerate().filter(|e| done.class_name(e.1) == "SwfMovie") {
                let (data, props) = export_properties(&done, flat, e);
                let Some(rd) = props.iter().find(|p| p.name == "RawData" && p.kind == "ArrayProperty") else { continue };
                let movie = &data[rd.value + 4..rd.value + rd.size];
                let Ok((_, _, end, pos, _)) = abc_tag(movie) else { continue };
                let abc = Abc::parse(&movie[pos..end]).unwrap();
                // Only the methods the patch rewrote (some untouched vanilla code has jumps this simple decoder can't follow).
                for (m, b) in &abc.bodies {
                    let original = original_bodies.get(&(file.clone(), x, *m));
                    if original.is_some_and(|o| *o != b.code && branches_ok(o)) {
                        if !branches_ok(&b.code) {
                            let owner = abc.classes.iter().flat_map(|c| c.1.traits.iter().map(move |t| (c.0.clone(), t))).find(|(_, t)| t.method == Some(*m)).map(|(c, t)| format!("{c}::{}", t.name));
                            for i in crate::engine::hud::instructions(&b.code).unwrap() { println!("{} {:02x} {:?} {:02x?}", i.0, i.2, i.3, &b.code[i.0..i.1]); }
                            let o = original.unwrap();
                            for i in crate::engine::hud::instructions(o).unwrap() { println!("ORIG {} {:02x} {:?} {:02x?}", i.0, i.2, i.3, &o[i.0..i.1]); }
                            panic!("{file}: salto roto en {owner:?}");
                        }
                        rewritten += 1;
                    }
                }
                if let Some(c) = abc.class("tagame.hud.WaitingForPlayerRenderer") {
                    let m = c.traits.iter().find(|t| t.name.ends_with(".setConnectionStatus")).and_then(|t| t.method).unwrap();
                    if abc.body(m).unwrap().code.iter().filter(|b| **b == 0x2a).count() >= 4 { waiting += 1; }
                }
                checked += 1;
            }
        }
        println!("RESUMEN {checked} peliculas, {rewritten} metodos reescritos con saltos validos, lista de espera en {waiting}");
        assert!(checked > 50 && waiting >= 1);
    }
}
