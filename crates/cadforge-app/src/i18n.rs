//! Interface languages. The ribbon, menus, status bar, palettes and dialog frames (titles, buttons, groups, tabs) can
//! be shown in German, French or Spanish; command prompts, command descriptions and the text inside most dialogs stay
//! English. Strings are looked up by their English text in the tables `assets/lang/*.tsv` (English, a tab, the
//! translation; `\n` stands for a line break); anything not in a table is shown as it is. LOCALE chooses the language
//! (ENU, DEU, FRA or ESP).
use std::collections::HashMap;
use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::OnceLock;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Lang {
    En,
    De,
    Fr,
    Es,
}

/// The languages: LOCALE code and the language's own name.
pub const LANGS: [(Lang, &str, &str); 4] = [(Lang::En, "ENU", "English"), (Lang::De, "DEU", "Deutsch"), (Lang::Fr, "FRA", "Français"), (Lang::Es, "ESP", "Español")];

static CURRENT: AtomicU8 = AtomicU8::new(0);

pub fn lang() -> Lang {
    LANGS[CURRENT.load(Ordering::Relaxed) as usize % LANGS.len()].0
}

pub fn set_lang(l: Lang) {
    CURRENT.store(LANGS.iter().position(|x| x.0 == l).unwrap_or(0) as u8, Ordering::Relaxed);
}

/// The language of a LOCALE code (also takes `de`, `fr`, `es`, `en` and the languages' names); `None` if unknown.
pub fn from_code(code: &str) -> Option<Lang> {
    let c = code.trim().to_uppercase();
    LANGS.iter().find(|(_, k, name)| *k == c || name.to_uppercase() == c || (c.len() == 2 && k.starts_with(&c))).map(|x| x.0)
}

pub fn code(l: Lang) -> &'static str {
    LANGS.iter().find(|x| x.0 == l).map_or("ENU", |x| x.1)
}

fn source(l: Lang) -> &'static str {
    match l {
        Lang::En => "",
        Lang::De => include_str!("../assets/lang/de.tsv"),
        Lang::Fr => include_str!("../assets/lang/fr.tsv"),
        Lang::Es => include_str!("../assets/lang/es.tsv"),
    }
}

fn unescape(s: &str) -> String {
    s.replace("\\n", "\n").replace("\\t", "\t")
}

/// Parse a table: `(english, translation)` pairs, comments (`#`) and blank lines skipped.
pub fn parse(src: &str) -> Vec<(String, String)> {
    src.lines().filter(|l| !l.trim().is_empty() && !l.starts_with('#')).filter_map(|l| l.split_once('\t')).map(|(k, v)| (unescape(k), unescape(v.trim_end_matches('\r')))).collect()
}

fn table(l: Lang) -> &'static HashMap<String, String> {
    static TABLES: [OnceLock<HashMap<String, String>>; 4] = [OnceLock::new(), OnceLock::new(), OnceLock::new(), OnceLock::new()];
    let i = LANGS.iter().position(|x| x.0 == l).unwrap_or(0);
    TABLES[i].get_or_init(|| parse(source(l)).into_iter().collect())
}

/// `s` in the current language (or `s` itself).
pub fn tr(s: &str) -> &str {
    tr_in(lang(), s)
}

pub fn tr_in(l: Lang, s: &str) -> &str {
    if l == Lang::En {
        return s;
    }
    table(l).get(s).map_or(s, String::as_str)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeSet;

    /// Every string of the ribbon, the menus, the status bar and the palette and dialog frames.
    fn ui_strings() -> BTreeSet<String> {
        let mut out = BTreeSet::new();
        let reg = cadforge_engine::commands::build_registry();
        let data = crate::data::RibbonData::load();
        let json: serde_json::Value = serde_json::from_str(include_str!("../assets/ribbon.json")).unwrap();
        fn walk(v: &serde_json::Value, out: &mut BTreeSet<String>, cmds: &mut Vec<String>) {
            match v {
                serde_json::Value::Object(m) => {
                    for k in ["title", "label"] {
                        if let Some(serde_json::Value::String(s)) = m.get(k) {
                            out.insert(s.clone());
                        }
                    }
                    if let (Some(serde_json::Value::String(c)), None) = (m.get("cmd"), m.get("label")) {
                        cmds.push(c.clone());
                    }
                    m.values().for_each(|x| walk(x, out, cmds));
                }
                serde_json::Value::Array(a) => a.iter().for_each(|x| walk(x, out, cmds)),
                _ => {}
            }
        }
        let mut cmds = vec![];
        walk(&json, &mut out, &mut cmds);
        let _ = data;
        for c in cmds {
            if let Some(d) = reg.get(&c) {
                out.insert(d.title.clone());
            }
        }
        // literals at the translated call sites of the app's sources
        let pats = ["MItem::item(\"", "MItem::Header(\"", "palette_header(ui, \"", "pal_section(ui, \"", "title: \"", "desc: \"", ".btn(ui, \"", "group(ui, \"", "on_hover_text(\"", "i18n::tr(\""];
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
        let mut files = vec![];
        let mut stack = vec![dir];
        while let Some(d) = stack.pop() {
            for e in std::fs::read_dir(&d).unwrap().flatten() {
                let p = e.path();
                if p.is_dir() {
                    stack.push(p);
                } else if p.extension().is_some_and(|x| x == "rs") && !p.ends_with("i18n.rs") {
                    files.push(p);
                }
            }
        }
        // every literal on the lines of button rows and tab strips, and in the dialog titles
        fn literals(text: &str, out: &mut BTreeSet<String>) {
            let mut rest = text;
            while let Some(i) = rest.find('"') {
                rest = &rest[i + 1..];
                let Some(j) = rest.find('"') else { break };
                out.insert(rest[..j].to_string());
                rest = &rest[j + 1..];
            }
        }
        for f in files {
            let src = std::fs::read_to_string(&f).unwrap();
            for line in src.lines().filter(|l| l.contains("ok_cancel(ui") || l.contains("tab_strip(ui") || l.contains("FileBrowserState { title") || l.contains("pal_row(t, ui, \"") || l.contains("ro_row(ui, &t, \"")) {
                literals(line, &mut out);
            }
            if let Some(i) = src.find("fn dialog_title_width") {
                literals(&src[i..i + src[i..].find("\n}\n").unwrap()], &mut out);
            }
            if f.ends_with("palettes.rs") {
                for pat in ["x(\"", "xmin(\"", "ro(\"", "ro_n(\"", "ang(\"", "boolf(\"", "textf(\""] {
                    let mut rest = src.as_str();
                    while let Some(i) = rest.find(pat) {
                        let before = rest[..i].chars().last();
                        rest = &rest[i + pat.len()..];
                        if before.is_some_and(|c| c.is_alphanumeric() || c == '_') {
                            continue;
                        }
                        out.insert(rest[..rest.find('"').unwrap()].to_string());
                    }
                }
            }
            for pat in pats {
                let mut rest = src.as_str();
                while let Some(i) = rest.find(pat) {
                    rest = &rest[i + pat.len()..];
                    let mut s = String::new();
                    let mut chars = rest.chars();
                    while let Some(c) = chars.next() {
                        match c {
                            '"' => break,
                            '\\' => match chars.next() {
                                Some('n') => s.push('\n'),
                                Some(x) => s.push(x),
                                None => {}
                            },
                            _ => s.push(c),
                        }
                    }
                    out.insert(s);
                }
            }
        }
        // interface text starts with a capital (or a digit); lower-case words are keys and arguments
        out.retain(|s| s.chars().find(|c| c.is_alphanumeric()).is_some_and(|c| c.is_uppercase() || c.is_ascii_digit()) && s.chars().any(|c| c.is_lowercase()) && !s.contains('{'));
        out
    }

    /// The tables are well formed and cover nearly all of the interface. New strings may stay English (they are shown
    /// as they are); the test lists them, and fails only when the share left untranslated grows past 10%.
    #[test]
    fn every_interface_string_is_translated() {
        let all = ui_strings();
        assert!(all.len() > 300, "{}", all.len());
        let mut report = String::new();
        for (l, code, _) in LANGS.iter().skip(1) {
            let t = parse(source(*l));
            let mut seen = BTreeSet::new();
            for (k, v) in &t {
                assert!(seen.insert(k.clone()), "{code}: {k:?} twice");
                assert!(!v.trim().is_empty(), "{code}: {k:?} has no translation");
                assert_eq!(k.matches('\n').count(), v.matches('\n').count(), "{code}: {k:?} and {v:?} break into different line counts");
            }
            let missing: Vec<&String> = all.iter().filter(|s| !seen.contains(*s)).collect();
            if !missing.is_empty() {
                println!("{code}: {} of {} not translated: {:?}", missing.len(), all.len(), missing);
            }
            if missing.len() * 10 > all.len() {
                report += &format!("{code}: {} of {} missing\n", missing.len(), all.len());
            }
        }
        assert!(report.is_empty(), "{report}");
    }

    #[test]
    fn lookup_falls_back_to_english() {
        assert_eq!(tr_in(Lang::De, "Home"), "Start");
        assert_eq!(tr_in(Lang::Fr, "Draw"), "Dessiner");
        assert_eq!(tr_in(Lang::Es, "Modify"), "Modificar");
        assert_eq!(tr_in(Lang::De, "no such text"), "no such text");
        assert_eq!(tr_in(Lang::En, "Home"), "Home");
        assert_eq!(from_code("deu"), Some(Lang::De));
        assert_eq!(from_code("fr"), Some(Lang::Fr));
        assert_eq!(from_code("Español"), Some(Lang::Es));
        assert_eq!(from_code("XYZ"), None);
    }
}
