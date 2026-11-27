//! Interface language. Spanish is the source: texts are written in Spanish in the code and looked up in
//! assets/i18n/en.json when the app is in English. A missing translation shows the Spanish text, never breaks.
//! Texts built with format! are matched against templates: «{count} cambios» finds «{count} changes».
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::OnceLock;

static ENGLISH: AtomicBool = AtomicBool::new(false);

/// A Spanish template split around its `{…}` holes, with the English text to fill.
struct Template { parts: Vec<&'static str>, names: Vec<&'static str>, english: &'static str }
struct Table { exact: HashMap<&'static str, &'static str>, templates: Vec<Template> }

/// Hole names of a template: `{count}` → "count", `{}` / `{:.1}` → "" (positional).
fn split(template: &str) -> (Vec<&str>, Vec<&str>) {
    let (mut parts, mut names, mut rest) = (Vec::new(), Vec::new(), template);
    while let (Some(open), Some(close)) = (rest.find('{'), rest.find('}')) {
        if close < open { break; }
        parts.push(&rest[..open]);
        let hole = &rest[open + 1..close];
        names.push(&hole[..hole.find(':').unwrap_or(hole.len())]);
        rest = &rest[close + 1..];
    }
    parts.push(rest);
    (parts, names)
}

fn table() -> &'static Table {
    static TABLE: OnceLock<Table> = OnceLock::new();
    TABLE.get_or_init(|| {
        let raw: HashMap<String, String> = serde_json::from_str(include_str!("../assets/i18n/en.json")).unwrap_or_default();
        let mut exact = HashMap::new();
        let mut templates = Vec::new();
        for (es, en) in raw {
            let (es, en): (&'static str, &'static str) = (Box::leak(es.into_boxed_str()), Box::leak(en.into_boxed_str()));
            if es.contains('{') {
                let (parts, names) = split(es);
                // A template with almost no fixed text would match anything.
                if parts.iter().map(|p| p.trim().len()).sum::<usize>() >= 4 { templates.push(Template { parts, names, english: en }); }
            } else {
                exact.insert(es, en);
            }
        }
        // The most specific templates first.
        templates.sort_by_key(|t| std::cmp::Reverse(t.parts.iter().map(|p| p.len()).sum::<usize>()));
        Table { exact, templates }
    })
}

pub fn set_english(on: bool) { ENGLISH.store(on, Ordering::Relaxed); }
pub fn english() -> bool { ENGLISH.load(Ordering::Relaxed) }

/// Windows' regional language: Spanish keeps Spanish, anything else starts in English.
pub fn system_is_spanish() -> bool {
    use windows_sys::Win32::System::Registry::*;
    let key: Vec<u16> = "Control Panel\\International".encode_utf16().chain(Some(0)).collect();
    let value: Vec<u16> = "LocaleName".encode_utf16().chain(Some(0)).collect();
    let mut buffer = [0u16; 85];
    let mut size = (buffer.len() * 2) as u32;
    let found = unsafe { RegGetValueW(HKEY_CURRENT_USER, key.as_ptr(), value.as_ptr(), RRF_RT_REG_SZ, std::ptr::null_mut(), buffer.as_mut_ptr().cast(), &mut size) } == 0;
    let locale = String::from_utf16_lossy(&buffer[..buffer.iter().position(|c| *c == 0).unwrap_or(0)]);
    !found || locale.to_lowercase().starts_with("es")
}

/// A fixed text.
pub fn t(spanish: &'static str) -> &'static str {
    if !english() { return spanish; }
    table().exact.get(spanish).copied().unwrap_or(spanish)
}

/// Any text, also ones built with format! (and messages from the engine).
pub fn tr(text: &str) -> String {
    if !english() { return text.to_owned(); }
    translate(table(), text)
}

fn translate(table: &Table, text: &str) -> String {
    if let Some(en) = table.exact.get(text) { return (*en).to_owned(); }
    table.templates.iter().find_map(|t| fill(t, text)).unwrap_or_else(|| text.to_owned())
}

/// Matches `text` against a Spanish template and writes its values into the English one.
fn fill(template: &Template, text: &str) -> Option<String> {
    let parts = &template.parts;
    let mut rest = text.strip_prefix(parts[0])?;
    let mut values = Vec::new();
    for (i, part) in parts.iter().enumerate().skip(1) {
        let at = if i == parts.len() - 1 { if rest.ends_with(part) { rest.len() - part.len() } else { return None } } else { rest.find(part)? };
        if part.is_empty() && i < parts.len() - 1 { return None; } // two holes in a row are ambiguous
        values.push(&rest[..at]);
        rest = &rest[at + part.len()..];
    }
    let (en_parts, en_names) = split(template.english);
    let mut out = String::from(en_parts[0]);
    let mut positional = template.names.iter().enumerate().filter(|(_, n)| n.is_empty()).map(|(i, _)| i);
    for (name, part) in en_names.iter().zip(&en_parts[1..]) {
        let index = if name.is_empty() { positional.next()? } else { template.names.iter().position(|n| n == name)? };
        out.push_str(values.get(index)?);
        out.push_str(part);
    }
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn exact_and_templates() {
        let table = table();
        assert_eq!(translate(table, "Cancelar"), "Cancel");
        assert_eq!(translate(table, "Rocket League ha quitado 3 cambios"), "Rocket League removed 3 changes");
        assert_eq!(translate(table, "«Ranked» listo. Pulsa Aplicar."), "«Ranked» ready. Press Apply.");
        assert_eq!(translate(table, "Texto que no existe"), "Texto que no existe");
        // Named holes keep their own value even if the English order differs.
        assert_eq!(translate(table, "Título: Rey · sustituye Reina"), "Title: Rey · replaces Reina");
    }
    #[test]
    fn every_translation_keeps_its_holes() {
        let raw: HashMap<String, String> = serde_json::from_str(include_str!("../assets/i18n/en.json")).unwrap();
        for (es, en) in raw {
            let holes = |t: &str| { let mut h: Vec<String> = t.split('{').skip(1).map(|p| p.split(['}', ':']).next().unwrap_or("").to_owned()).collect(); h.sort(); h };
            assert_eq!(holes(&es), holes(&en), "{es}");
        }
    }
}
