//! CadForge application (egui): native and web front end for the CadForge engine.
pub mod app;
pub mod chrome;
pub mod data;
pub mod dialogs;
pub mod gfx;
pub mod icons;
pub mod keys;
pub mod menu;
pub mod misc;
pub mod palettes;
pub mod platform;
pub mod ribbon;
pub mod theme;
pub mod viewport;

pub use app::CadForgeApp;

#[cfg(target_arch = "wasm32")]
mod web {
    use super::CadForgeApp;
    use eframe::wasm_bindgen::{self, prelude::*};

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
