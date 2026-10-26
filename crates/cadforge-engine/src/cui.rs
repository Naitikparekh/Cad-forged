//! User interface customization (CUI): extra Quick Access Toolbar buttons, keyboard shortcuts and ribbon panels of
//! the user's own. The main customization is the user's; partial customizations are groups loaded from files
//! (CUILOAD) that add their buttons, shortcuts and panels until they are unloaded (CUIUNLOAD). A customization file is
//! JSON (`*.cui.json`): `{ "cadforge_cui": 1, "qat": [...], "shortcuts": [...], "panels": [...] }`.
use serde::{Deserialize, Serialize};

/// A Quick Access Toolbar button: a command, with the arguments of the ribbon button it was added from.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct QatItem {
    pub cmd: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub args: Option<serde_json::Value>,
}

/// A keyboard shortcut: keys such as `Ctrl+Shift+L` start a command.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Shortcut {
    pub keys: String,
    pub cmd: String,
}

/// A ribbon panel of the user's own (shown on the Custom tab).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct UserPanel {
    pub title: String,
    pub cmds: Vec<String>,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Customization {
    #[serde(default)]
    pub qat: Vec<QatItem>,
    #[serde(default)]
    pub shortcuts: Vec<Shortcut>,
    #[serde(default)]
    pub panels: Vec<UserPanel>,
}

/// A partial customization: a named group loaded from a file.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Partial {
    pub name: String,
    pub cui: Customization,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct CuiState {
    #[serde(default)]
    pub main: Customization,
    #[serde(default)]
    pub partials: Vec<Partial>,
    /// Counts changes, so a host knows when to store the customization again.
    #[serde(skip)]
    pub rev: u64,
}

const MODS: [&str; 3] = ["Ctrl", "Alt", "Shift"];

/// Keys written the one way they are compared: `ctrl + shift + l` is `Ctrl+Shift+L`. `None` when there is no key or
/// only modifiers, or when a single printable key has no Ctrl or Alt (that would take a letter away from typing).
pub fn normalize_keys(s: &str) -> Option<String> {
    let parts: Vec<String> = s.split('+').map(|p| p.trim().to_string()).filter(|p| !p.is_empty()).collect();
    let mut mods: Vec<&str> = vec![];
    let mut key: Option<String> = None;
    for p in &parts {
        match MODS.iter().find(|m| m.eq_ignore_ascii_case(p) || (p.eq_ignore_ascii_case("control") && **m == "Ctrl") || (p.eq_ignore_ascii_case("cmd") && **m == "Ctrl")) {
            Some(m) if !mods.contains(m) => mods.push(m),
            Some(_) => {}
            None if key.is_none() => key = Some(if p.chars().count() == 1 { p.to_uppercase() } else { capitalize(p) }),
            None => return None,
        }
    }
    let key = key?;
    let function_key = key.len() > 1 && key.starts_with('F') && key[1..].parse::<u8>().is_ok_and(|n| (1..=24).contains(&n));
    if !function_key && !mods.contains(&"Ctrl") && !mods.contains(&"Alt") {
        return None;
    }
    let mut out: Vec<&str> = MODS.iter().filter(|m| mods.contains(m)).copied().collect();
    out.push(&key);
    Some(out.join("+"))
}

fn capitalize(s: &str) -> String {
    let mut c = s.chars();
    c.next().map_or(String::new(), |f| f.to_uppercase().chain(c.flat_map(char::to_lowercase)).collect())
}

impl Customization {
    pub fn is_empty(&self) -> bool {
        self.qat.is_empty() && self.shortcuts.is_empty() && self.panels.is_empty()
    }

    /// The file text (CUIEXPORT).
    pub fn to_json(&self) -> String {
        let mut v = serde_json::to_value(self).unwrap_or_default();
        if let Some(m) = v.as_object_mut() {
            m.insert("cadforge_cui".into(), serde_json::Value::from(1));
        }
        serde_json::to_string_pretty(&v).unwrap_or_default()
    }

    /// Read a customization file; shortcuts are normalized and ones that cannot be used are dropped.
    pub fn from_json(text: &str) -> Result<Customization, String> {
        let v: serde_json::Value = serde_json::from_str(text).map_err(|e| format!("not a customization file ({e})"))?;
        if v.get("cadforge_cui").is_none() {
            return Err("not a customization file (no \"cadforge_cui\" key)".into());
        }
        let mut c: Customization = serde_json::from_value(v).map_err(|e| format!("not a customization file ({e})"))?;
        c.shortcuts = c.shortcuts.into_iter().filter_map(|s| Some(Shortcut { keys: normalize_keys(&s.keys)?, cmd: s.cmd.trim().to_uppercase() })).filter(|s| !s.cmd.is_empty()).collect();
        for q in &mut c.qat {
            q.cmd = q.cmd.trim().to_uppercase();
        }
        c.qat.retain(|q| !q.cmd.is_empty());
        for p in &mut c.panels {
            p.cmds = p.cmds.iter().map(|x| x.trim().to_uppercase()).filter(|x| !x.is_empty()).collect();
        }
        c.panels.retain(|p| !p.title.trim().is_empty());
        Ok(c)
    }

    /// Add what `other` has that this does not (CUIIMPORT): toolbar buttons, shortcuts (a shortcut for the same keys
    /// is replaced) and panels (a panel of the same title gets the other's commands). Returns (buttons, shortcuts,
    /// panels) added or changed.
    pub fn merge(&mut self, other: &Customization) -> (usize, usize, usize) {
        let mut n = (0, 0, 0);
        for q in &other.qat {
            if !self.qat.contains(q) {
                self.qat.push(q.clone());
                n.0 += 1;
            }
        }
        for s in &other.shortcuts {
            match self.shortcuts.iter_mut().find(|x| x.keys == s.keys) {
                Some(x) if x.cmd == s.cmd => {}
                Some(x) => {
                    x.cmd = s.cmd.clone();
                    n.1 += 1;
                }
                None => {
                    self.shortcuts.push(s.clone());
                    n.1 += 1;
                }
            }
        }
        for p in &other.panels {
            match self.panels.iter_mut().find(|x| x.title.eq_ignore_ascii_case(&p.title)) {
                Some(x) if x.cmds == p.cmds => {}
                Some(x) => {
                    x.cmds = p.cmds.clone();
                    n.2 += 1;
                }
                None => {
                    self.panels.push(p.clone());
                    n.2 += 1;
                }
            }
        }
        n
    }
}

impl CuiState {
    pub fn touch(&mut self) {
        self.rev += 1;
    }

    /// Everything in force: the main customization, then each partial's (the main one wins a shortcut both define).
    pub fn effective(&self) -> Customization {
        let mut all = Customization::default();
        for c in std::iter::once(&self.main).chain(self.partials.iter().map(|p| &p.cui)) {
            for q in &c.qat {
                if !all.qat.contains(q) {
                    all.qat.push(q.clone());
                }
            }
            for s in &c.shortcuts {
                if !all.shortcuts.iter().any(|x| x.keys == s.keys) {
                    all.shortcuts.push(s.clone());
                }
            }
            all.panels.extend(c.panels.iter().cloned());
        }
        all
    }

    /// The command the keys start, if a shortcut has them.
    pub fn shortcut(&self, keys: &str) -> Option<String> {
        let k = normalize_keys(keys)?;
        self.effective().shortcuts.into_iter().find(|s| s.keys == k).map(|s| s.cmd)
    }

    /// Load a partial customization; one of the same name is replaced.
    pub fn load(&mut self, name: &str, c: Customization) {
        self.partials.retain(|p| !p.name.eq_ignore_ascii_case(name));
        self.partials.push(Partial { name: name.to_string(), cui: c });
        self.touch();
    }

    /// Unload a partial customization; false if none has that name.
    pub fn unload(&mut self, name: &str) -> bool {
        let before = self.partials.len();
        self.partials.retain(|p| !p.name.eq_ignore_ascii_case(name));
        let gone = self.partials.len() < before;
        if gone {
            self.touch();
        }
        gone
    }

    pub fn add_qat(&mut self, cmd: &str, args: Option<serde_json::Value>) -> bool {
        let q = QatItem { cmd: cmd.trim().to_uppercase(), args };
        if q.cmd.is_empty() || self.main.qat.contains(&q) {
            return false;
        }
        self.main.qat.push(q);
        self.touch();
        true
    }

    /// Set a shortcut of the main customization (replacing one for the same keys).
    pub fn set_shortcut(&mut self, keys: &str, cmd: &str) -> Result<String, String> {
        let k = normalize_keys(keys).ok_or_else(|| format!("\"{keys}\" cannot be a shortcut: use Ctrl or Alt with a key, or a function key."))?;
        let cmd = cmd.trim().to_uppercase();
        if cmd.is_empty() {
            return Err("No command.".into());
        }
        self.main.shortcuts.retain(|s| s.keys != k);
        self.main.shortcuts.push(Shortcut { keys: k.clone(), cmd });
        self.touch();
        Ok(k)
    }
}

/// The group name of a customization file: its name without folders and extensions (`tools.cui.json` is `tools`).
pub fn group_name(file: &str) -> String {
    let base = file.rsplit(['/', '\\']).next().unwrap_or(file);
    base.split('.').next().filter(|s| !s.is_empty()).unwrap_or(base).to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keys_are_normalized() {
        assert_eq!(normalize_keys("ctrl + shift + l").as_deref(), Some("Ctrl+Shift+L"));
        assert_eq!(normalize_keys("Shift+Alt+q").as_deref(), Some("Alt+Shift+Q"));
        assert_eq!(normalize_keys("f6").as_deref(), Some("F6"));
        assert_eq!(normalize_keys("Control+delete").as_deref(), Some("Ctrl+Delete"));
        assert_eq!(normalize_keys("L"), None, "a bare letter would take it from typing");
        assert_eq!(normalize_keys("Shift+L"), None);
        assert_eq!(normalize_keys("Ctrl+Shift"), None, "no key");
        assert_eq!(normalize_keys("Ctrl+A+B"), None, "two keys");
    }

    #[test]
    fn files_round_trip_and_merge() {
        let mut s = CuiState::default();
        assert!(s.add_qat("line", None));
        assert!(!s.add_qat("LINE", None), "once");
        s.set_shortcut("ctrl+shift+l", "line").unwrap();
        s.main.panels.push(UserPanel { title: "Mine".into(), cmds: vec!["CIRCLE".into()] });
        let text = s.main.to_json();
        let back = Customization::from_json(&text).unwrap();
        assert_eq!(back, s.main);
        assert!(Customization::from_json("{\"qat\": []}").is_err(), "needs the marker key");
        let other = Customization::from_json(r#"{"cadforge_cui":1,"qat":[{"cmd":"move"}],"shortcuts":[{"keys":"ctrl+shift+l","cmd":"pline"},{"keys":"x","cmd":"erase"}],"panels":[{"title":"mine","cmds":["arc"]}]}"#).unwrap();
        assert_eq!(other.shortcuts.len(), 1, "the bare-letter shortcut is dropped");
        assert_eq!(s.main.merge(&other), (1, 1, 1));
        assert_eq!(s.main.shortcuts[0].cmd, "PLINE");
        assert_eq!(s.main.panels[0].cmds, vec!["ARC".to_string()]);
        // partial customizations add to the main one until unloaded
        s.load("extra", Customization { shortcuts: vec![Shortcut { keys: "Alt+E".into(), cmd: "ERASE".into() }], ..Default::default() });
        assert_eq!(s.shortcut("alt+e").as_deref(), Some("ERASE"));
        assert!(s.unload("EXTRA"));
        assert_eq!(s.shortcut("Alt+E"), None);
        assert!(!s.unload("extra"));
        assert_eq!(group_name("C:\\cui\\tools.cui.json"), "tools");
    }
}
