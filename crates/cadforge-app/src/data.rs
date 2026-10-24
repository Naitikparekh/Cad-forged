//! Ribbon layout (exported from the web UI by `tools/gen-app-data.mjs`).
use cadforge_engine::runtime::{ArgVal, Args};
use serde::Deserialize;
use serde_json::Value;

#[derive(Deserialize, Clone, Debug)]
pub struct RibbonData {
    pub tabs: Vec<Tab>,
    pub hatch: Tab,
    pub hatchedit: Tab,
}

#[derive(Deserialize, Clone, Debug)]
pub struct Tab {
    pub id: String,
    pub title: String,
    #[serde(default)]
    pub contextual: bool,
    pub panels: Vec<Panel>,
}

#[derive(Deserialize, Clone, Debug)]
pub struct Panel {
    pub title: String,
    #[serde(default)]
    pub launcher: Option<String>,
    #[serde(default)]
    pub prio: Option<i32>,
    pub items: Vec<Item>,
    #[serde(default)]
    pub slideout: Vec<Item>,
}

#[derive(Deserialize, Clone, Debug)]
#[serde(tag = "t", rename_all = "lowercase")]
pub enum Item {
    Btn {
        cmd: String,
        size: String,
        #[serde(default)]
        label: Option<String>,
        #[serde(default)]
        icon: Option<String>,
        #[serde(default)]
        args: Option<Value>,
        #[serde(default)]
        menu: Option<Menu>,
    },
    Stack { items: Vec<Item> },
    Col { items: Vec<Item> },
    Row { items: Vec<Item> },
    Combo { kind: String },
    HatchPick,
    Patterns,
    HatchProps,
    HatchClose,
}

#[derive(Deserialize, Clone, Debug)]
#[serde(untagged)]
pub enum Menu {
    Named(String),
    Items(Vec<MenuEntry>),
}

#[derive(Deserialize, Clone, Debug)]
pub struct MenuEntry {
    #[serde(default)]
    pub sep: bool,
    #[serde(default)]
    pub cmd: String,
    #[serde(default)]
    pub label: Option<String>,
    #[serde(default)]
    pub icon: Option<String>,
    #[serde(default)]
    pub args: Option<Value>,
}

impl RibbonData {
    pub fn load() -> RibbonData {
        serde_json::from_str(include_str!("../assets/ribbon.json")).expect("ribbon.json")
    }
}

/// Convert JSON arguments (`{ "mode": "3p" }`) into command [`Args`]. Keys are interned (leaked once; the set is tiny).
pub fn args_from_json(v: Option<&Value>) -> Args {
    let mut a = Args::none();
    if let Some(Value::Object(m)) = v {
        for (k, val) in m {
            let key: &'static str = Box::leak(k.clone().into_boxed_str());
            let av = match val {
                Value::String(s) => ArgVal::Str(s.clone()),
                Value::Bool(b) => ArgVal::Bool(*b),
                Value::Number(n) => ArgVal::Num(n.as_f64().unwrap_or(0.0)),
                _ => continue,
            };
            a = a.with(key, av);
        }
    }
    a
}
