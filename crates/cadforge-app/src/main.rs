//! Native entry point.
#[cfg(not(target_arch = "wasm32"))]
fn main() -> eframe::Result {
    env_logger::init();
    let options = eframe::NativeOptions { viewport: eframe::egui::ViewportBuilder::default().with_inner_size([1440.0, 900.0]).with_min_inner_size([960.0, 600.0]).with_title("CadForge"), ..Default::default() };
    eframe::run_native("CadForge", options, Box::new(|cc| Ok(Box::new(cadforge_app::CadForgeApp::new(cc)))))
}

#[cfg(target_arch = "wasm32")]
fn main() {}
