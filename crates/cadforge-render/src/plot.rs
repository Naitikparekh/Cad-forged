//! Plotting: lays a drawing out on a sheet and renders it with the scene renderer (port of `src/io/plot.js`).
//! The device unit is 0.1 mm.
use crate::pdf::write_pdf;
use crate::scene::{DrawOpts, LwMode, Renderer};
use crate::svg::write_svg;
use crate::theme::{Prefs, Theme};
use crate::Op;
use cadforge_core::drawing::Drawing;
use cadforge_core::entities::{bounds_of, Entity};
use cadforge_core::geom::*;
use cadforge_core::view::View;

pub const UNIT_MM: f64 = 0.1;

pub const PAPERS: [(&str, f64, f64); 8] =
    [("A4", 297.0, 210.0), ("A3", 420.0, 297.0), ("A2", 594.0, 420.0), ("A1", 841.0, 594.0), ("A0", 1189.0, 841.0), ("Letter", 279.4, 215.9), ("Legal", 355.6, 215.9), ("Tabloid", 431.8, 279.4)];

/// Millimetres per drawing unit for `$INSUNITS` values.
pub fn mm_per_insunit(u: i32) -> f64 {
    match u {
        1 => 25.4,
        2 => 304.8,
        5 => 10.0,
        6 => 1000.0,
        _ => 1.0,
    }
}

/// Preset scales shown in the plot dialog (label, factor); `None` = fit to paper.
pub const SCALES: [(&str, Option<f64>); 12] = [
    ("Fit to paper", None),
    ("1:1", Some(1.0)),
    ("1:2", Some(0.5)),
    ("1:5", Some(0.2)),
    ("1:10", Some(0.1)),
    ("1:20", Some(0.05)),
    ("1:50", Some(0.02)),
    ("1:100", Some(0.01)),
    ("1:200", Some(0.005)),
    ("2:1", Some(2.0)),
    ("5:1", Some(5.0)),
    ("10:1", Some(10.0)),
];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PlotArea {
    Extents,
    Display,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PlotFormat {
    Pdf,
    Svg,
    Png,
}

#[derive(Clone, Debug)]
pub struct PlotOptions {
    pub paper: String,
    pub landscape: bool,
    pub area: PlotArea,
    /// `None` = fit to paper, otherwise plotted units per drawing unit (1:50 = 0.02).
    pub scale: Option<f64>,
    pub center: bool,
    pub offset_x: f64,
    pub offset_y: f64,
    pub margin: f64,
    pub lineweights: bool,
    pub mono: bool,
    pub format: PlotFormat,
    pub dpi: f64,
    pub title: String,
}

pub fn default_plot_options(doc: &Drawing) -> PlotOptions {
    PlotOptions {
        paper: "A4".into(),
        landscape: true,
        area: PlotArea::Extents,
        scale: None,
        center: true,
        offset_x: 0.0,
        offset_y: 0.0,
        margin: 5.0,
        lineweights: true,
        mono: true,
        format: PlotFormat::Pdf,
        dpi: 200.0,
        title: doc.name.clone(),
    }
}

/// Page size in mm for the options.
pub fn page_size(o: &PlotOptions) -> (f64, f64) {
    let (a, b) = PAPERS.iter().find(|p| p.0 == o.paper).map(|p| (p.1, p.2)).unwrap_or((297.0, 210.0));
    if o.landscape {
        (a.max(b), a.min(b))
    } else {
        (a.min(b), a.max(b))
    }
}

fn plottable(doc: &Drawing, e: &Entity) -> bool {
    let l = doc.layer_of(e);
    l.on && !l.frozen && l.plot
}

pub fn plot_bounds(doc: &Drawing, o: &PlotOptions, view_bounds: Option<Bounds>) -> Option<Bounds> {
    if o.area == PlotArea::Display {
        if let Some(b) = view_bounds {
            return Some(b);
        }
    }
    let mut b: Option<Bounds> = None;
    for e in &doc.model.entities {
        if plottable(doc, e) {
            b = Bounds::union_opt(b, bounds_of(e, doc));
        }
    }
    b
}

#[derive(Clone, Copy, Debug)]
pub struct PlotLayout {
    pub page: (f64, f64),
    pub bounds: Bounds,
    pub mm_per_unit: f64,
    pub world_m: Mat,
    pub fits: bool,
    /// Sheet size in device units.
    pub w: f64,
    pub h: f64,
}

/// Work out the world → sheet mapping. Fails when there is nothing to plot.
pub fn plot_layout(doc: &Drawing, o: &PlotOptions, view_bounds: Option<Bounds>) -> Result<PlotLayout, String> {
    let page = page_size(o);
    let bounds = plot_bounds(doc, o, view_bounds).ok_or("There is nothing to plot.")?;
    let bw = (bounds.max_x - bounds.min_x).max(1e-9);
    let bh = (bounds.max_y - bounds.min_y).max(1e-9);
    let (pw, ph) = (page.0 - 2.0 * o.margin, page.1 - 2.0 * o.margin);
    let s = match o.scale {
        None => (pw / bw).min(ph / bh),
        Some(f) => mm_per_insunit(doc.settings.units.ins_units) * f,
    };
    let ox = if o.center { (page.0 - bw * s) / 2.0 } else { o.margin + o.offset_x };
    let oy = if o.center { (page.1 - bh * s) / 2.0 } else { o.margin + o.offset_y };
    let world_m = [10.0 * s, 0.0, 0.0, -10.0 * s, 10.0 * (ox - s * bounds.min_x), 10.0 * (page.1 - oy + s * bounds.min_y)];
    let fits = bw * s <= pw + 1e-6 && bh * s <= ph + 1e-6;
    Ok(PlotLayout { page, bounds, mm_per_unit: s, world_m, fits, w: (page.0 * 10.0).round(), h: (page.1 * 10.0).round() })
}

/// Render the sheet into draw operations (device unit 0.1 mm, y down, origin top-left).
pub fn render_plot(doc: &Drawing, o: &PlotOptions, layout: &PlotLayout) -> Vec<Op> {
    let view = View::new();
    let prefs = Prefs { lw_display: true, lock_fade: 0.0, transparency: true, fill: true, grid: false, ucs_icon: false, ..Prefs::default() };
    let mut rc = Renderer::new(doc, &view, layout.w, layout.h, Theme::PAPER, prefs);
    rc.world = layout.world_m;
    rc.mono = o.mono;
    rc.lw_mode = LwMode::Plot(o.lineweights);
    rc.tol = 0.08;
    for e in &doc.model.entities {
        if plottable(doc, e) {
            rc.draw_entity(e, DrawOpts::default());
        }
    }
    rc.ops
}

pub struct VectorPlot {
    pub data: Vec<u8>,
    pub mime: &'static str,
    pub ext: &'static str,
    pub layout: PlotLayout,
}

/// Vector export (PDF or SVG).
pub fn export_vector(doc: &Drawing, o: &PlotOptions, view_bounds: Option<Bounds>) -> Result<VectorPlot, String> {
    let layout = plot_layout(doc, o, view_bounds)?;
    let ops = render_plot(doc, o, &layout);
    let page = (layout.w, layout.h);
    Ok(match o.format {
        PlotFormat::Svg => VectorPlot { data: write_svg(&ops, page, UNIT_MM, &o.title).into_bytes(), mime: "image/svg+xml", ext: "svg", layout },
        _ => VectorPlot { data: write_pdf(&ops, page, UNIT_MM, &o.title), mime: "application/pdf", ext: "pdf", layout },
    })
}
