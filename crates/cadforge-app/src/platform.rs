//! Platform glue: file download / upload (browser) and a tiny persistent key-value store (browser localStorage or a
//! JSON file in the user's config directory).
use std::cell::RefCell;
use std::rc::Rc;

#[derive(Clone, Debug)]
pub struct Picked {
    pub purpose: &'static str,
    pub name: String,
    pub bytes: Vec<u8>,
}

pub type Inbox = Rc<RefCell<Vec<Picked>>>;

// ---------------------------------------------------------------- web

#[cfg(target_arch = "wasm32")]
mod imp {
    use super::*;
    use wasm_bindgen::prelude::*;
    use wasm_bindgen::JsCast;

    pub fn download(name: &str, bytes: &[u8], mime: &str) {
        let Some(window) = web_sys::window() else { return };
        let Some(document) = window.document() else { return };
        let arr = js_sys::Uint8Array::from(bytes);
        let parts = js_sys::Array::new();
        parts.push(&arr.buffer());
        let opts = web_sys::BlobPropertyBag::new();
        opts.set_type(mime);
        let Ok(blob) = web_sys::Blob::new_with_buffer_source_sequence_and_options(&parts, &opts) else { return };
        let Ok(url) = web_sys::Url::create_object_url_with_blob(&blob) else { return };
        if let Ok(a) = document.create_element("a") {
            let a: web_sys::HtmlAnchorElement = a.unchecked_into();
            a.set_href(&url);
            a.set_download(name);
            if let Some(body) = document.body() {
                let _ = body.append_child(&a);
                a.click();
                let _ = body.remove_child(&a);
            }
        }
        let url2 = url.clone();
        let cb = Closure::once_into_js(move || {
            let _ = web_sys::Url::revoke_object_url(&url2);
        });
        let _ = window.set_timeout_with_callback_and_timeout_and_arguments_0(cb.unchecked_ref(), 4000);
    }

    /// Open the browser's file chooser; the picked files arrive in `inbox` (and trigger a repaint).
    pub fn pick_files(accept: &str, multiple: bool, purpose: &'static str, inbox: Inbox, ctx: eframe::egui::Context) {
        let Some(document) = web_sys::window().and_then(|w| w.document()) else { return };
        let Ok(input) = document.create_element("input") else { return };
        let input: web_sys::HtmlInputElement = input.unchecked_into();
        input.set_type("file");
        input.set_accept(accept);
        input.set_multiple(multiple);
        let input2 = input.clone();
        let cb = Closure::<dyn FnMut()>::new(move || {
            let Some(files) = input2.files() else { return };
            for i in 0..files.length() {
                let Some(f) = files.get(i) else { continue };
                let (inbox, ctx) = (inbox.clone(), ctx.clone());
                wasm_bindgen_futures::spawn_local(async move {
                    let name = f.name();
                    if let Ok(buf) = wasm_bindgen_futures::JsFuture::from(f.array_buffer()).await {
                        let bytes = js_sys::Uint8Array::new(&buf).to_vec();
                        inbox.borrow_mut().push(Picked { purpose, name, bytes });
                        ctx.request_repaint();
                    }
                });
            }
        });
        input.set_onchange(Some(cb.as_ref().unchecked_ref()));
        cb.forget();
        input.click();
    }

    fn storage() -> Option<web_sys::Storage> {
        web_sys::window()?.local_storage().ok()?
    }
    pub fn store_get(key: &str) -> Option<String> {
        storage()?.get_item(&format!("cf.{key}")).ok()?
    }
    pub fn store_set(key: &str, value: &str) {
        if let Some(s) = storage() {
            let _ = s.set_item(&format!("cf.{key}"), value);
        }
    }
    pub fn copy_to_clipboard(text: &str) {
        if let Some(w) = web_sys::window() {
            let _ = w.navigator().clipboard().write_text(text);
        }
    }

    const AUTOSAVE_PREFIX: &str = "cf.autosave.";
    /// Store a recovery file; false when the browser refuses (storage full).
    pub fn autosave_put(key: &str, json: &str) -> bool {
        storage().is_some_and(|s| s.set_item(&format!("{AUTOSAVE_PREFIX}{key}"), json).is_ok())
    }
    pub fn autosave_remove(key: &str) {
        if let Some(s) = storage() {
            let _ = s.remove_item(&format!("{AUTOSAVE_PREFIX}{key}"));
        }
    }
    /// Every stored recovery file as (key, project JSON).
    pub fn autosave_list() -> Vec<(String, String)> {
        let Some(s) = storage() else { return vec![] };
        let keys: Vec<String> = (0..s.length().unwrap_or(0)).filter_map(|i| s.key(i).ok().flatten()).filter(|k| k.starts_with(AUTOSAVE_PREFIX)).collect();
        keys.into_iter().filter_map(|k| Some((k[AUTOSAVE_PREFIX.len()..].to_string(), s.get_item(&k).ok()??))).collect()
    }
    /// Unique per session: the clock in milliseconds.
    pub fn session_salt() -> String {
        format!("{}", js_sys::Date::now() as u64)
    }
}

// ---------------------------------------------------------------- native

#[cfg(not(target_arch = "wasm32"))]
mod imp {
    use super::*;
    use std::path::PathBuf;

    fn config_file() -> PathBuf {
        let base = std::env::var_os("XDG_CONFIG_HOME").map(PathBuf::from).or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".config"))).or_else(|| std::env::var_os("APPDATA").map(PathBuf::from)).unwrap_or_else(|| PathBuf::from("."));
        base.join("cadforge").join("settings.json")
    }

    fn load() -> serde_json::Map<String, serde_json::Value> {
        std::fs::read_to_string(config_file()).ok().and_then(|s| serde_json::from_str(&s).ok()).unwrap_or_default()
    }

    pub fn store_get(key: &str) -> Option<String> {
        load().get(key).and_then(|v| v.as_str().map(str::to_string))
    }
    pub fn store_set(key: &str, value: &str) {
        let mut m = load();
        m.insert(key.to_string(), serde_json::Value::String(value.to_string()));
        let f = config_file();
        if let Some(d) = f.parent() {
            let _ = std::fs::create_dir_all(d);
        }
        let _ = std::fs::write(f, serde_json::to_string_pretty(&m).unwrap_or_default());
    }
    pub fn download(_name: &str, _bytes: &[u8], _mime: &str) {}
    pub fn pick_files(_accept: &str, _multiple: bool, _purpose: &'static str, _inbox: Inbox, _ctx: eframe::egui::Context) {}
    pub fn copy_to_clipboard(_text: &str) {}

    fn autosave_dir() -> PathBuf {
        config_file().parent().map(|p| p.join("autosave")).unwrap_or_else(|| PathBuf::from("autosave"))
    }
    /// Store a recovery file; false when it could not be written.
    pub fn autosave_put(key: &str, json: &str) -> bool {
        let dir = autosave_dir();
        std::fs::create_dir_all(&dir).is_ok() && std::fs::write(dir.join(format!("{key}.json")), json).is_ok()
    }
    pub fn autosave_remove(key: &str) {
        let _ = std::fs::remove_file(autosave_dir().join(format!("{key}.json")));
    }
    /// Every stored recovery file as (key, project JSON).
    pub fn autosave_list() -> Vec<(String, String)> {
        let Ok(rd) = std::fs::read_dir(autosave_dir()) else { return vec![] };
        let mut v: Vec<(String, String)> = rd
            .filter_map(|e| {
                let p = e.ok()?.path();
                (p.extension()? == "json").then_some(())?;
                Some((p.file_stem()?.to_string_lossy().to_string(), std::fs::read_to_string(&p).ok()?))
            })
            .collect();
        v.sort();
        v
    }
    /// Unique per session: the clock in milliseconds.
    pub fn session_salt() -> String {
        format!("{}", std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_millis()).unwrap_or(0))
    }
}

pub use imp::*;

/// Carry out an autosave plan: store the files, delete the ones no longer needed. Returns how many could not be stored.
pub fn autosave_apply(plan: &cadforge_engine::autosave::AutoSavePlan) -> usize {
    for key in &plan.remove {
        autosave_remove(key);
    }
    plan.write.iter().filter(|f| !autosave_put(&f.key, &f.json)).count()
}
