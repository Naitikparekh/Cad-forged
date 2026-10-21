//! CadForge application (egui): native and web front end for the CadForge engine.
pub mod app;
pub mod chrome;
pub mod data;
pub mod dialogs;
pub mod gfx;
pub mod icons;
pub mod keys;
pub mod layouts_ui;
pub mod menu;
pub mod misc;
pub mod palettes;
pub mod platform;
pub mod ribbon;
pub mod theme;
pub mod viewport;

pub use app::CadForgeApp;
#[cfg(target_arch = "wasm32")]
pub(crate) use web::install_hook;

#[cfg(target_arch = "wasm32")]
mod web {
    use super::CadForgeApp;
    use eframe::wasm_bindgen::{self, prelude::*};
    use std::cell::RefCell;

    thread_local! {
        /// Debug / test hook: the engine handle and the viewport rectangle of the running application.
        static HOOK: RefCell<Option<(cadforge_engine::app::Handle, std::rc::Rc<RefCell<Option<[f32; 4]>>>)>> = const { RefCell::new(None) };
    }

    pub fn install_hook(h: cadforge_engine::app::Handle, rect: std::rc::Rc<RefCell<Option<[f32; 4]>>>) {
        HOOK.with(|x| *x.borrow_mut() = Some((h, rect)));
    }

    /// JSON snapshot of the engine state (used by the browser tests).
    #[wasm_bindgen]
    pub fn state_json() -> String {
        HOOK.with(|x| {
            let x = x.borrow();
            let Some((h, _)) = x.as_ref() else { return "{}".into() };
            let a = h.borrow();
            let Some(ed) = a.editor() else { return "{\"editors\":0}".into() };
            let tail: Vec<&str> = a.history.iter().rev().take(8).map(|l| l.text.as_str()).collect();
            let types: Vec<&str> = ed.doc.entities().iter().map(|e| e.type_name()).collect();
            serde_json::json!({
                "editors": a.editors.len(),
                "name": ed.doc.name,
                "dirty": ed.doc.dirty,
                "entities": ed.doc.entities().len(),
                "types": types,
                "selected": ed.sel.iter().collect::<Vec<_>>(),
                "solids": ed.doc.solids.len(),
                "busy": a.busy(),
                "prompt": a.prompt_line(),
                "history": tail,
                "layers": ed.doc.layers.iter().map(|l| l.name.clone()).collect::<Vec<_>>(),
                "curLayer": ed.doc.cur.layer,
                "view": { "x": ed.view.target.x, "y": ed.view.target.y, "scale": ed.view.scale, "yaw": ed.view.yaw, "pitch": ed.view.pitch },
                "osmode": a.sysvars.int("OSMODE"),
                "canUndo": ed.doc.can_undo(),
                "style": ed.visual_style,
            })
            .to_string()
        })
    }

    /// Page coordinates of a world point in the active viewport.
    #[wasm_bindgen]
    pub fn world_to_page(x: f64, y: f64) -> Vec<f64> {
        HOOK.with(|c| {
            let c = c.borrow();
            let Some((h, rect)) = c.as_ref() else { return vec![] };
            let a = h.borrow();
            let (Some(ed), Some(r)) = (a.editor(), *rect.borrow()) else { return vec![] };
            let s = ed.view.to_screen(cadforge_core::geom::pt(x, y));
            vec![r[0] as f64 + s.x, r[1] as f64 + s.y]
        })
    }

    /// Start the application on the canvas with the given element id.
    #[wasm_bindgen]
    pub async fn start(canvas_id: String) -> Result<(), JsValue> {
        console_error_panic_hook_set();
        let window = web_sys::window().ok_or("no window")?;
        let document = window.document().ok_or("no document")?;
        let canvas = document.get_element_by_id(&canvas_id).ok_or("canvas not found")?.dyn_into::<web_sys::HtmlCanvasElement>()?;
        eframe::WebRunner::new().start(canvas, eframe::WebOptions::default(), Box::new(|cc| Ok(Box::new(CadForgeApp::new(cc))))).await
    }

    fn console_error_panic_hook_set() {
        std::panic::set_hook(Box::new(|info| {
            web_sys::console::error_1(&format!("{info}").into());
        }));
    }
}
