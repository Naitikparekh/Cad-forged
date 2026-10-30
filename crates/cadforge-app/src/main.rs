//! Native entry point.
// release builds on Windows are GUI programs: no console window opens next to the app
#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]
#[cfg(not(target_arch = "wasm32"))]
fn main() -> eframe::Result {
    env_logger::init();
    let icon = eframe::egui::IconData { rgba: cadforge_app::logo::rgba(256), width: 256, height: 256 };
    let options = eframe::NativeOptions { viewport: eframe::egui::ViewportBuilder::default().with_inner_size([1440.0, 900.0]).with_min_inner_size([960.0, 600.0]).with_title("CadForge").with_icon(std::sync::Arc::new(icon)), ..Default::default() };
    eframe::run_native("CadForge", options, Box::new(|cc| Ok(Box::new(cadforge_app::CadForgeApp::new(cc)))))
}

#[cfg(target_arch = "wasm32")]
fn main() {}
