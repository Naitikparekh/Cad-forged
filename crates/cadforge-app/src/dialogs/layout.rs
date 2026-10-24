//! Page Setup dialog for the current layout.
use super::Out;
use crate::app::CadForgeApp;
use cadforge_core::drawing::Paper;
use cadforge_engine::runtime::{ArgVal, Args};
use eframe::egui::{self, RichText};

const PAPERS: [&str; 9] = [
    "A0", "A1", "A2", "A3", "A4", "Letter", "Legal", "Tabloid", "ANSI C",
];

pub struct PageSetupState {
    pub layout: String,
    pub paper: String,
    pub landscape: bool,
}

impl PageSetupState {
    pub fn new(app: &CadForgeApp) -> PageSetupState {
        let a = app.h.borrow();
        let tab = a.editor().map(|e| e.current_tab()).unwrap_or_default();
        let p = a
            .doc()
            .and_then(|d| d.paper_of(&tab).cloned())
            .unwrap_or_else(|| Paper::named("A4", true));
        PageSetupState {
            layout: tab,
            paper: p.name,
            landscape: p.landscape,
        }
    }

    pub fn apply(&self, app: &mut CadForgeApp) {
        let args = Args::none()
            .with("paper", ArgVal::Str(self.paper.clone()))
            .with(
                "orientation",
                ArgVal::Str(
                    if self.landscape {
                        "Landscape"
                    } else {
                        "Portrait"
                    }
                    .into(),
                ),
            );
        app.run_command("PAGESETUP", args);
    }
}

impl CadForgeApp {
    pub(super) fn ui_pagesetup(&mut self, ui: &mut egui::Ui, s: &mut PageSetupState) -> Out {
        egui::Grid::new("pagesetup")
            .num_columns(2)
            .spacing([12.0, 8.0])
            .show(ui, |ui| {
                ui.label("Layout");
                ui.label(RichText::new(s.layout.clone()).strong());
                ui.end_row();
                ui.label("Paper size");
                egui::ComboBox::from_id_salt("ps-paper")
                    .selected_text(&s.paper)
                    .width(160.0)
                    .show_ui(ui, |ui| {
                        for p in PAPERS {
                            ui.selectable_value(&mut s.paper, p.to_string(), p);
                        }
                    });
                ui.end_row();
                ui.label("Orientation");
                egui::ComboBox::from_id_salt("ps-or")
                    .selected_text(if s.landscape { "Landscape" } else { "Portrait" })
                    .width(160.0)
                    .show_ui(ui, |ui| {
                        ui.selectable_value(&mut s.landscape, true, "Landscape");
                        ui.selectable_value(&mut s.landscape, false, "Portrait");
                    });
                ui.end_row();
                let p = Paper::named(&s.paper, s.landscape);
                ui.label("Sheet");
                ui.label(format!("{} × {} mm", p.w, p.h));
                ui.end_row();
            });
        self.note(ui, "Plots of this layout use the sheet at 1:1. Viewports keep their size on the sheet; use MVIEW or the Properties palette to adjust them.");
        self.ok_cancel(ui, Some("OK"), Some("Cancel"), &[])
    }
}
