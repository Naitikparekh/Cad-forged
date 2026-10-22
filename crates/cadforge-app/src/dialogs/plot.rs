//! Plot / export dialog: sheet setup with a live preview, writing PDF, SVG or PNG files.
use super::Out;
use crate::app::CadForgeApp;
use cadforge_core::geom::Bounds;
use cadforge_render::{default_plot_options, export_vector, plot_layout, render_plot, Op, PlotArea, PlotFormat, PlotOptions, Poly, PAPERS, SCALES, UNIT_MM};
use eframe::egui::{self, vec2, Color32, Pos2, Rect, RichText, Sense, Stroke, StrokeKind};

pub struct PlotState {
    pub ready: bool,
    pub o: Option<PlotOptions>,
    pub format: String,
    pub scale_idx: usize,
    pub ox: String,
    pub oy: String,
    pub view_bounds: Option<Bounds>,
    pub cache_key: String,
    pub cache: Vec<Op>,
    pub info: String,
    pub page: (f64, f64),
}

impl PlotState {
    pub fn new(format: &str) -> PlotState {
        PlotState { ready: false, o: None, format: format.into(), scale_idx: 0, ox: "0".into(), oy: "0".into(), view_bounds: None, cache_key: String::new(), cache: vec![], info: String::new(), page: (0.0, 0.0) }
    }
    pub fn load(&mut self, app: &CadForgeApp) {
        let a = app.h.borrow();
        let Some(ed) = a.editor() else { return };
        let mut o = default_plot_options(&ed.doc);
        o.format = match self.format.as_str() {
            "svg" => PlotFormat::Svg,
            "png" => PlotFormat::Png,
            _ => PlotFormat::Pdf,
        };
        self.view_bounds = Some(ed.view.world_bounds());
        let tab = ed.current_tab();
        if !tab.eq_ignore_ascii_case(cadforge_core::drawing::MODEL) && ed.doc.layout(&tab).is_some() {
            o.layout = Some(tab);
        }
        self.o = Some(o);
        self.ready = true;
    }
    pub fn run(&self, app: &mut CadForgeApp) {
        let Some(mut o) = self.o.clone() else { return };
        o.offset_x = self.ox.trim().parse().unwrap_or(0.0);
        o.offset_y = self.oy.trim().parse().unwrap_or(0.0);
        o.scale = SCALES[self.scale_idx.min(SCALES.len() - 1)].1;
        let (name, result) = {
            let a = app.h.borrow();
            let Some(doc) = a.doc() else { return };
            let name = doc.name.clone();
            let res: Result<(String, Vec<u8>, &'static str), String> = if o.format == PlotFormat::Png {
                plot_layout(doc, &o, self.view_bounds).and_then(|layout| {
                    let ops = render_plot(doc, &o, &layout);
                    cadforge_render::raster::render_png(&ops, (layout.w, layout.h), UNIT_MM, o.dpi, include_bytes!("../../assets/fonts/LiberationSans-Regular.ttf")).map(|d| ("png".to_string(), d, "image/png"))
                })
            } else {
                export_vector(doc, &o, self.view_bounds).map(|r| (r.ext.to_string(), r.data, r.mime))
            };
            (name, res)
        };
        match result {
            Ok((ext, data, mime)) => {
                app.deliver_file(&format!("{name}.{ext}"), data, mime);
                app.h.borrow_mut().log(&format!("Plotted {name} to {}.", ext.to_uppercase()), "info");
            }
            Err(e) => app.toast(&e),
        }
    }
}

fn scale_ops(ops: &[Op], k: f64) -> Vec<Op> {
    ops.iter()
        .map(|op| match op {
            Op::Stroke { paths, color, alpha, width, dash, round } => Op::Stroke {
                paths: paths.iter().map(|p| Poly { pts: p.pts.iter().map(|q| [q[0] * k, q[1] * k]).collect(), closed: p.closed }).collect(),
                color: *color,
                alpha: *alpha,
                width: (*width * k).max(0.6),
                dash: dash.iter().map(|d| d * k).collect(),
                round: *round,
            },
            Op::Fill { paths, color, alpha, evenodd } => Op::Fill { paths: paths.iter().map(|p| Poly { pts: p.pts.iter().map(|q| [q[0] * k, q[1] * k]).collect(), closed: p.closed }).collect(), color: *color, alpha: *alpha, evenodd: *evenodd },
            Op::Text { s, x, y, m, em, color, alpha } => Op::Text { s: s.clone(), x: *x, y: *y, m: [m[0] * k, m[1] * k, m[2] * k, m[3] * k, m[4] * k, m[5] * k], em: *em, color: *color, alpha: *alpha },
            Op::Image { pix, adj, bg, m, clip, alpha, smooth } => Op::Image { pix: pix.clone(), adj: *adj, bg: *bg, m: [m[0] * k, m[1] * k, m[2] * k, m[3] * k, m[4] * k, m[5] * k], clip: clip.iter().map(|q| [q[0] * k, q[1] * k]).collect(), alpha: *alpha, smooth: *smooth },
        })
        .collect()
}

impl CadForgeApp {
    pub(super) fn ui_plot(&mut self, _ctx: &egui::Context, ui: &mut egui::Ui, s: &mut PlotState) -> Out {
        let t = self.t;
        let Some(o) = s.o.as_mut() else { return self.ok_cancel(ui, None, Some("Close"), &[]) };
        let mut changed = s.cache.is_empty();
        let layout_mode = o.layout.is_some();
        ui.horizontal_top(|ui| {
            ui.vertical(|ui| {
                ui.set_width(300.0);
                egui::Grid::new("plot-form").num_columns(2).spacing([12.0, 8.0]).show(ui, |ui| {
                    ui.label("Output");
                    let fmt_label = |f: PlotFormat| match f {
                        PlotFormat::Pdf => "PDF (vector)",
                        PlotFormat::Svg => "SVG (vector)",
                        PlotFormat::Png => "PNG (raster)",
                    };
                    egui::ComboBox::from_id_salt("pl-fmt").selected_text(fmt_label(o.format)).width(150.0).show_ui(ui, |ui| {
                        for f in [PlotFormat::Pdf, PlotFormat::Svg, PlotFormat::Png] {
                            changed |= ui.selectable_value(&mut o.format, f, fmt_label(f)).changed();
                        }
                    });
                    ui.end_row();
                    if let Some(name) = &o.layout {
                        ui.label("Layout");
                        ui.label(RichText::new(name.clone()).strong());
                        ui.end_row();
                        ui.label("Paper");
                        let paper = self.h.borrow().doc().and_then(|d| d.paper_of(name).cloned());
                        ui.label(paper.map_or(String::new(), |p| format!("{} {} × {} mm, 1:1", p.name, p.w, p.h)));
                        ui.end_row();
                    }
                    if !layout_mode {
                    ui.label("Paper size");
                    egui::ComboBox::from_id_salt("pl-paper").selected_text(&o.paper).width(150.0).show_ui(ui, |ui| {
                        for (n, _, _) in PAPERS.iter() {
                            changed |= ui.selectable_value(&mut o.paper, n.to_string(), *n).changed();
                        }
                    });
                    ui.end_row();
                    ui.label("Orientation");
                    egui::ComboBox::from_id_salt("pl-or").selected_text(if o.landscape { "Landscape" } else { "Portrait" }).width(150.0).show_ui(ui, |ui| {
                        changed |= ui.selectable_value(&mut o.landscape, true, "Landscape").changed();
                        changed |= ui.selectable_value(&mut o.landscape, false, "Portrait").changed();
                    });
                    ui.end_row();
                    ui.label("Plot area");
                    egui::ComboBox::from_id_salt("pl-area").selected_text(if o.area == PlotArea::Extents { "Extents" } else { "Display" }).width(150.0).show_ui(ui, |ui| {
                        changed |= ui.selectable_value(&mut o.area, PlotArea::Extents, "Extents").changed();
                        changed |= ui.selectable_value(&mut o.area, PlotArea::Display, "Display").changed();
                    });
                    ui.end_row();
                    ui.label("Scale");
                    egui::ComboBox::from_id_salt("pl-scale").selected_text(SCALES[s.scale_idx.min(SCALES.len() - 1)].0).width(150.0).show_ui(ui, |ui| {
                        for (i, (l, _)) in SCALES.iter().enumerate() {
                            changed |= ui.selectable_value(&mut s.scale_idx, i, *l).changed();
                        }
                    });
                    ui.end_row();
                    ui.label("");
                    changed |= ui.checkbox(&mut o.center, "Center the plot").changed();
                    ui.end_row();
                    ui.label("Offset X, Y (mm)");
                    ui.horizontal(|ui| {
                        ui.add_enabled_ui(!o.center, |ui| {
                            changed |= ui.add(egui::TextEdit::singleline(&mut s.ox).desired_width(60.0)).changed();
                            changed |= ui.add(egui::TextEdit::singleline(&mut s.oy).desired_width(60.0)).changed();
                        });
                    });
                    ui.end_row();
                    }
                    ui.label("");
                    changed |= ui.checkbox(&mut o.lineweights, "Plot with lineweights").changed();
                    ui.end_row();
                    let table = self.h.borrow().doc().and_then(|d| d.plot_styles.as_ref().map(|t| t.name.clone()));
                    let styled = table.is_some() && o.use_styles;
                    ui.label("");
                    ui.add_enabled_ui(!styled, |ui| {
                        changed |= ui.checkbox(&mut o.mono, "Monochrome (black ink)").changed();
                    });
                    ui.end_row();
                    if let Some(name) = table {
                        ui.label("Plot styles");
                        changed |= ui.checkbox(&mut o.use_styles, format!("Use table \"{name}\" (colours, lineweights)")).changed();
                        ui.end_row();
                    }
                    ui.label("PNG resolution (dpi)");
                    ui.add_enabled_ui(o.format == PlotFormat::Png, |ui| {
                        egui::ComboBox::from_id_salt("pl-dpi").selected_text(format!("{}", o.dpi as i32)).width(150.0).show_ui(ui, |ui| {
                            for d in [100.0, 150.0, 200.0, 300.0, 600.0] {
                                ui.selectable_value(&mut o.dpi, d, format!("{}", d as i32));
                            }
                        });
                    });
                    ui.end_row();
                });
            });
            ui.vertical(|ui| {
                let (r, _) = ui.allocate_exact_size(vec2(420.0, 300.0), Sense::hover());
                ui.painter().rect_filled(r, 0.0, Color32::from_rgb(0x5b, 0x64, 0x70));
                let key = format!("{:?}|{:?}|{}|{}|{:?}|{}|{}|{}|{}|{}|{}", o.format, o.layout, o.paper, o.landscape, o.area as u8, s.scale_idx, o.center, o.lineweights, o.mono, format_args!("{}/{}", s.ox, s.oy), o.use_styles);
                if changed || key != s.cache_key {
                    s.cache_key = key;
                    o.scale = SCALES[s.scale_idx.min(SCALES.len() - 1)].1;
                    o.offset_x = s.ox.trim().parse().unwrap_or(0.0);
                    o.offset_y = s.oy.trim().parse().unwrap_or(0.0);
                    let a = self.h.borrow();
                    if let Some(doc) = a.doc() {
                        match plot_layout(doc, o, s.view_bounds) {
                            Ok(layout) => {
                                let k = ((420.0 - 24.0) / layout.w).min((300.0 - 24.0) / layout.h);
                                s.cache = scale_ops(&render_plot(doc, o, &layout), k);
                                s.page = (layout.w * k, layout.h * k);
                                s.info = format!("{} × {} mm · 1 drawing unit = {} mm on paper{}", layout.page.0, layout.page.1, (layout.mm_per_unit * 10000.0).round() / 10000.0, if layout.fits { "" } else { " · the drawing is larger than the printable area" });
                            }
                            Err(e) => {
                                s.cache = vec![];
                                s.info = e;
                                s.page = (0.0, 0.0);
                            }
                        }
                    }
                }
                if s.page.0 > 0.0 {
                    let pr = Rect::from_center_size(r.center(), vec2(s.page.0 as f32, s.page.1 as f32));
                    ui.painter().rect_filled(pr, 0.0, Color32::WHITE);
                    let clip = ui.painter().with_clip_rect(pr);
                    crate::gfx::paint_ops(&clip, pr.min, &s.cache, pr);
                    let m = (o.margin * 10.0) as f32 * (s.page.0 as f32 / ((o.paper_w()) * 10.0));
                    let _ = m;
                    ui.painter().rect_stroke(pr, 0.0, Stroke::new(1.0, Color32::from_black_alpha(90)), StrokeKind::Outside);
                }
                ui.label(RichText::new(&s.info).small().color(t.text3));
            });
        });
        let _ = Pos2::ZERO;
        self.ok_cancel(ui, Some("Plot"), Some("Cancel"), &[])
    }
}

trait PaperW {
    fn paper_w(&self) -> f32;
}
impl PaperW for PlotOptions {
    fn paper_w(&self) -> f32 {
        cadforge_render::page_size(self).0 as f32
    }
}
