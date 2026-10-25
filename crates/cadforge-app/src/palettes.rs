//! Palettes: Properties (docked), Tool Palettes (docked), Layer Properties Manager (floating) and Quick Properties.
use crate::app::CadForgeApp;
use crate::icons::Tint;
use crate::ribbon::{color_rgb_of, swatch};
use cadforge_core::drawing::Drawing;
use cadforge_core::entities::{Entity, Kind};
use cadforge_core::geom::*;
use cadforge_core::style::{describe_color, fmt_num, ltype_display, lw_display, Color, all_linetypes, LINEWEIGHTS};
use cadforge_core::text::HJust;
use cadforge_engine::runtime::Args;
use eframe::egui::{self, vec2, Align2, Color32, CornerRadius, FontFamily, FontId, Margin, Pos2, Rect, RichText, Sense, Stroke, StrokeKind};
use std::collections::HashSet;

// ---------------------------------------------------------------- property schema (Geometry section)

#[derive(Clone, Debug, PartialEq)]
pub enum Val {
    Num(f64),
    Text(String),
    Bool(bool),
}

pub enum FKind {
    Num { min: Option<f64>, max: Option<f64> },
    Ro,
    Bool,
    Select(Vec<String>),
    Text,
}

pub struct Field {
    pub label: &'static str,
    pub unit: Option<&'static str>,
    pub kind: FKind,
    pub get: Box<dyn Fn(&Entity, &Drawing) -> Val>,
    pub set: Option<Box<dyn Fn(&mut Entity, &Val)>>,
}

fn x(label: &'static str, get: impl Fn(&Entity) -> f64 + 'static, set: impl Fn(&mut Entity, f64) + 'static) -> Field {
    Field { label, unit: None, kind: FKind::Num { min: None, max: None }, get: Box::new(move |e, _| Val::Num(get(e))), set: Some(Box::new(move |e, v| if let Val::Num(n) = v { set(e, *n) })) }
}
fn xmin(label: &'static str, min: f64, get: impl Fn(&Entity) -> f64 + 'static, set: impl Fn(&mut Entity, f64) + 'static) -> Field {
    Field { label, unit: None, kind: FKind::Num { min: Some(min), max: None }, get: Box::new(move |e, _| Val::Num(get(e))), set: Some(Box::new(move |e, v| if let Val::Num(n) = v { set(e, *n) })) }
}
fn ro(label: &'static str, get: impl Fn(&Entity, &Drawing) -> Val + 'static) -> Field {
    Field { label, unit: None, kind: FKind::Ro, get: Box::new(get), set: None }
}
fn ro_n(label: &'static str, get: impl Fn(&Entity) -> f64 + 'static) -> Field {
    ro(label, move |e, _| Val::Num(get(e)))
}
fn ang(label: &'static str, get: impl Fn(&Entity) -> f64 + 'static, set: impl Fn(&mut Entity, f64) + 'static) -> Field {
    Field { label, unit: Some("°"), kind: FKind::Num { min: None, max: None }, get: Box::new(move |e, _| Val::Num(deg(get(e)))), set: Some(Box::new(move |e, v| if let Val::Num(n) = v { set(e, rad(*n)) })) }
}
fn boolf(label: &'static str, get: impl Fn(&Entity) -> bool + 'static, set: impl Fn(&mut Entity, bool) + 'static) -> Field {
    Field { label, unit: None, kind: FKind::Bool, get: Box::new(move |e, _| Val::Bool(get(e))), set: Some(Box::new(move |e, v| if let Val::Bool(b) = v { set(e, *b) })) }
}
fn textf(label: &'static str, get: impl Fn(&Entity) -> String + 'static, set: impl Fn(&mut Entity, String) + 'static) -> Field {
    Field { label, unit: None, kind: FKind::Text, get: Box::new(move |e, _| Val::Text(get(e))), set: Some(Box::new(move |e, v| if let Val::Text(s) = v { set(e, s.clone()) })) }
}

macro_rules! pt_field {
    ($label:expr, $variant:ident { $($pat:tt)* }, $field:ident . $axis:ident) => {
        x($label, |e| match &e.kind { Kind::$variant { $field, .. } => $field.$axis, _ => 0.0 }, |e, v| if let Kind::$variant { $field, .. } = &mut e.kind { $field.$axis = v })
    };
}

pub fn geom_fields(e: &Entity) -> Vec<Field> {
    match &e.kind {
        Kind::Line { .. } => vec![
            x("Start X", |e| if let Kind::Line { a, .. } = &e.kind { a.x } else { 0.0 }, |e, v| if let Kind::Line { a, .. } = &mut e.kind { a.x = v }),
            x("Start Y", |e| if let Kind::Line { a, .. } = &e.kind { a.y } else { 0.0 }, |e, v| if let Kind::Line { a, .. } = &mut e.kind { a.y = v }),
            x("End X", |e| if let Kind::Line { b, .. } = &e.kind { b.x } else { 0.0 }, |e, v| if let Kind::Line { b, .. } = &mut e.kind { b.x = v }),
            x("End Y", |e| if let Kind::Line { b, .. } = &e.kind { b.y } else { 0.0 }, |e, v| if let Kind::Line { b, .. } = &mut e.kind { b.y = v }),
            ro_n("Delta X", |e| if let Kind::Line { a, b } = &e.kind { b.x - a.x } else { 0.0 }),
            ro_n("Delta Y", |e| if let Kind::Line { a, b } = &e.kind { b.y - a.y } else { 0.0 }),
            ro_n("Length", |e| if let Kind::Line { a, b } = &e.kind { dist(*a, *b) } else { 0.0 }),
            ro("Angle", |e, _| if let Kind::Line { a, b } = &e.kind { Val::Text(format!("{}°", fmt_num(deg(norm_angle(angle_of(*a, *b))), 4, true))) } else { Val::Text(String::new()) }),
        ],
        Kind::Circle { .. } => vec![
            pt_field!("Center X", Circle { c, r }, c.x),
            pt_field!("Center Y", Circle { c, r }, c.y),
            xmin("Radius", 1e-9, |e| if let Kind::Circle { r, .. } = &e.kind { *r } else { 0.0 }, |e, v| if let Kind::Circle { r, .. } = &mut e.kind { *r = v }),
            xmin("Diameter", 1e-9, |e| if let Kind::Circle { r, .. } = &e.kind { *r * 2.0 } else { 0.0 }, |e, v| if let Kind::Circle { r, .. } = &mut e.kind { *r = v / 2.0 }),
            ro_n("Circumference", |e| if let Kind::Circle { r, .. } = &e.kind { TAU * r } else { 0.0 }),
            ro_n("Area", |e| if let Kind::Circle { r, .. } = &e.kind { PI * r * r } else { 0.0 }),
        ],
        Kind::Arc { .. } => vec![
            pt_field!("Center X", Arc { c, r }, c.x),
            pt_field!("Center Y", Arc { c, r }, c.y),
            xmin("Radius", 1e-9, |e| if let Kind::Arc { r, .. } = &e.kind { *r } else { 0.0 }, |e, v| if let Kind::Arc { r, .. } = &mut e.kind { *r = v }),
            ang("Start angle", |e| if let Kind::Arc { a0, .. } = &e.kind { *a0 } else { 0.0 }, |e, v| if let Kind::Arc { a0, .. } = &mut e.kind { *a0 = norm_angle(v) }),
            ang("End angle", |e| if let Kind::Arc { a1, .. } = &e.kind { *a1 } else { 0.0 }, |e, v| if let Kind::Arc { a1, .. } = &mut e.kind { *a1 = norm_angle(v) }),
            ro("Total angle", |e, _| if let Kind::Arc { a0, a1, .. } = &e.kind { Val::Text(format!("{}°", fmt_num(deg(sweep_ccw(*a0, *a1)), 4, true))) } else { Val::Text(String::new()) }),
            ro_n("Arc length", |e| if let Kind::Arc { r, a0, a1, .. } = &e.kind { r * sweep_ccw(*a0, *a1) } else { 0.0 }),
        ],
        Kind::Pline { .. } => vec![
            boolf("Closed", |e| matches!(&e.kind, Kind::Pline { closed: true, .. }), |e, v| if let Kind::Pline { closed, .. } = &mut e.kind { *closed = v }),
            ro_n("Vertices", |e| if let Kind::Pline { v, .. } = &e.kind { v.len() as f64 } else { 0.0 }),
            ro("Length", |e, d| Val::Num(cadforge_core::entities::segs_of(e, d).iter().map(|s| s.length()).sum())),
            ro_n("Area", |e| if let Kind::Pline { v, closed: true } = &e.kind { pline_area(v).abs() } else { 0.0 }),
        ],
        Kind::Ellipse(_) => vec![
            x("Center X", |e| if let Kind::Ellipse(el) = &e.kind { el.c.x } else { 0.0 }, |e, v| if let Kind::Ellipse(el) = &mut e.kind { el.c.x = v }),
            x("Center Y", |e| if let Kind::Ellipse(el) = &e.kind { el.c.y } else { 0.0 }, |e, v| if let Kind::Ellipse(el) = &mut e.kind { el.c.y = v }),
            ro_n("Major radius", |e| if let Kind::Ellipse(el) = &e.kind { len(el.m) } else { 0.0 }),
            ro_n("Minor radius", |e| if let Kind::Ellipse(el) = &e.kind { len(el.m) * el.ratio } else { 0.0 }),
            Field { label: "Radius ratio", unit: None, kind: FKind::Num { min: Some(1e-6), max: Some(1.0) }, get: Box::new(|e, _| Val::Num(if let Kind::Ellipse(el) = &e.kind { el.ratio } else { 0.0 })), set: Some(Box::new(|e, v| if let (Kind::Ellipse(el), Val::Num(n)) = (&mut e.kind, v) { el.ratio = *n })) },
        ],
        Kind::Spline { .. } => vec![boolf("Closed", |e| matches!(&e.kind, Kind::Spline { closed: true, .. }), |e, v| if let Kind::Spline { closed, .. } = &mut e.kind { *closed = v }), ro_n("Fit points", |e| if let Kind::Spline { fit, .. } = &e.kind { fit.len() as f64 } else { 0.0 }), ro_n("Control vertices", |e| if let Kind::Spline { nurbs: Some(n), .. } = &e.kind { n.cv.len() as f64 } else { 0.0 }), ro_n("Degree", |e| if let Kind::Spline { nurbs: Some(n), .. } = &e.kind { n.deg as f64 } else { 3.0 })],
        Kind::Point { .. } => vec![
            x("Position X", |e| if let Kind::Point { p } = &e.kind { p.x } else { 0.0 }, |e, v| if let Kind::Point { p } = &mut e.kind { p.x = v }),
            x("Position Y", |e| if let Kind::Point { p } = &e.kind { p.y } else { 0.0 }, |e, v| if let Kind::Point { p } = &mut e.kind { p.y = v }),
        ],
        Kind::Text(_) => vec![
            textf("Contents", |e| if let Kind::Text(t) = &e.kind { t.s.clone() } else { String::new() }, |e, v| if let Kind::Text(t) = &mut e.kind { t.s = v }),
            xmin("Height", 1e-9, |e| if let Kind::Text(t) = &e.kind { t.h } else { 0.0 }, |e, v| if let Kind::Text(t) = &mut e.kind { t.h = v }),
            ang("Rotation", |e| if let Kind::Text(t) = &e.kind { t.rot } else { 0.0 }, |e, v| if let Kind::Text(t) = &mut e.kind { t.rot = v }),
            x("Position X", |e| if let Kind::Text(t) = &e.kind { t.p.x } else { 0.0 }, |e, v| if let Kind::Text(t) = &mut e.kind { t.p.x = v }),
            x("Position Y", |e| if let Kind::Text(t) = &e.kind { t.p.y } else { 0.0 }, |e, v| if let Kind::Text(t) = &mut e.kind { t.p.y = v }),
            Field {
                label: "Justify",
                unit: None,
                kind: FKind::Select(vec!["left".into(), "center".into(), "right".into()]),
                get: Box::new(|e, _| Val::Text(if let Kind::Text(t) = &e.kind { format!("{:?}", t.jh).to_lowercase() } else { String::new() })),
                set: Some(Box::new(|e, v| {
                    if let (Kind::Text(t), Val::Text(s)) = (&mut e.kind, v) {
                        t.jh = match s.as_str() {
                            "center" => HJust::Center,
                            "right" => HJust::Right,
                            _ => HJust::Left,
                        };
                    }
                })),
            },
        ],
        Kind::Dim(_) => vec![
            ro("Measurement", |e, d| if let Kind::Dim(dd) = &e.kind { Val::Num(cadforge_core::dim::dim_geometry(dd, &d.settings).measured) } else { Val::Text(String::new()) }),
            textf("Text override", |e| if let Kind::Dim(dd) = &e.kind { dd.txt.clone().unwrap_or_default() } else { String::new() }, |e, v| if let Kind::Dim(dd) = &mut e.kind { dd.txt = if v.is_empty() { None } else { Some(v) } }),
        ],
        Kind::Leader { .. } => vec![
            textf("Contents", |e| if let Kind::Leader { s, .. } = &e.kind { s.clone() } else { String::new() }, |e, v| if let Kind::Leader { s, .. } = &mut e.kind { *s = v }),
            xmin("Text height", 1e-9, |e| if let Kind::Leader { h, .. } = &e.kind { *h } else { 0.0 }, |e, v| if let Kind::Leader { h, .. } = &mut e.kind { *h = v }),
        ],
        Kind::Hatch(_) => vec![
            Field {
                label: "Pattern",
                unit: None,
                kind: FKind::Select(cadforge_core::hatch::all_patterns().iter().map(|p| p.name.to_string()).collect()),
                get: Box::new(|e, _| Val::Text(if let Kind::Hatch(h) = &e.kind { h.pattern.clone() } else { String::new() })),
                set: Some(Box::new(|e, v| if let (Kind::Hatch(h), Val::Text(s)) = (&mut e.kind, v) { h.pattern = s.clone() })),
            },
            xmin("Scale", 1e-6, |e| if let Kind::Hatch(h) = &e.kind { h.ps } else { 1.0 }, |e, v| if let Kind::Hatch(h) = &mut e.kind { h.ps = v }),
            ang("Angle", |e| if let Kind::Hatch(h) = &e.kind { h.pa } else { 0.0 }, |e, v| if let Kind::Hatch(h) = &mut e.kind { h.pa = v }),
        ],
        Kind::Insert(_) => vec![
            ro("Block name", |e, _| Val::Text(if let Kind::Insert(i) = &e.kind { i.name.clone() } else { String::new() })),
            x("Position X", |e| if let Kind::Insert(i) = &e.kind { i.p.x } else { 0.0 }, |e, v| if let Kind::Insert(i) = &mut e.kind { i.p.x = v }),
            x("Position Y", |e| if let Kind::Insert(i) = &e.kind { i.p.y } else { 0.0 }, |e, v| if let Kind::Insert(i) = &mut e.kind { i.p.y = v }),
            x("Scale X", |e| if let Kind::Insert(i) = &e.kind { i.sx } else { 1.0 }, |e, v| if let Kind::Insert(i) = &mut e.kind { if v != 0.0 { i.sx = v } }),
            x("Scale Y", |e| if let Kind::Insert(i) = &e.kind { i.sy } else { 1.0 }, |e, v| if let Kind::Insert(i) = &mut e.kind { if v != 0.0 { i.sy = v } }),
            ang("Rotation", |e| if let Kind::Insert(i) = &e.kind { i.rot } else { 0.0 }, |e, v| if let Kind::Insert(i) = &mut e.kind { i.rot = v }),
        ],
        Kind::Xline { .. } | Kind::Ray { .. } => vec![
            x("Base X", |e| match &e.kind { Kind::Xline { p, .. } | Kind::Ray { p, .. } => p.x, _ => 0.0 }, |e, v| if let Kind::Xline { p, .. } | Kind::Ray { p, .. } = &mut e.kind { p.x = v }),
            x("Base Y", |e| match &e.kind { Kind::Xline { p, .. } | Kind::Ray { p, .. } => p.y, _ => 0.0 }, |e, v| if let Kind::Xline { p, .. } | Kind::Ray { p, .. } = &mut e.kind { p.y = v }),
            ro("Angle", |e, _| if let Kind::Xline { d, .. } | Kind::Ray { d, .. } = &e.kind { Val::Text(format!("{}°", fmt_num(deg(norm_angle(d.y.atan2(d.x))), 4, true))) } else { Val::Text(String::new()) }),
        ],
        Kind::Viewport(_) => vec![
            Field {
                label: "Standard scale",
                unit: None,
                kind: FKind::Select(cadforge_render::SCALES.iter().skip(1).map(|(l, _)| l.to_string()).collect()),
                get: Box::new(|e, _| {
                    let Kind::Viewport(v) = &e.kind else { return Val::Text(String::new()) };
                    match v.scale {
                        None => Val::Text("Fit".into()),
                        Some(s) => Val::Text(cadforge_render::SCALES.iter().find(|(_, f)| f.is_some_and(|f| (f - s).abs() < 1e-9)).map_or("Custom".to_string(), |(l, _)| l.to_string())),
                    }
                }),
                set: Some(Box::new(|e, v| {
                    if let (Kind::Viewport(vp), Val::Text(l)) = (&mut e.kind, v) {
                        if let Some((_, Some(f))) = cadforge_render::SCALES.iter().find(|(n, _)| n == l) {
                            vp.scale = Some(*f);
                        }
                    }
                })),
            },
            xmin("Custom scale", 1e-9, |e| if let Kind::Viewport(v) = &e.kind { v.scale.unwrap_or(1.0) } else { 1.0 }, |e, v| if let Kind::Viewport(vp) = &mut e.kind { vp.scale = Some(v) }),
            boolf("Display locked", |e| matches!(&e.kind, Kind::Viewport(v) if v.locked), |e, v| if let Kind::Viewport(vp) = &mut e.kind { vp.locked = v }),
            boolf("On", |e| matches!(&e.kind, Kind::Viewport(v) if !v.off), |e, v| if let Kind::Viewport(vp) = &mut e.kind { vp.off = !v }),
            ro_n("Width", |e| if let Kind::Viewport(v) = &e.kind { v.w } else { 0.0 }),
            ro_n("Height", |e| if let Kind::Viewport(v) = &e.kind { v.h } else { 0.0 }),
        ],
        Kind::Table(_) => vec![
            ro_n("Rows", |e| if let Kind::Table(t) = &e.kind { t.rows.len() as f64 } else { 0.0 }),
            ro_n("Columns", |e| if let Kind::Table(t) = &e.kind { t.cols.len() as f64 } else { 0.0 }),
            ro_n("Table width", |e| if let Kind::Table(t) = &e.kind { t.cols.iter().sum() } else { 0.0 }),
            ro_n("Table height", |e| if let Kind::Table(t) = &e.kind { t.rows.iter().sum() } else { 0.0 }),
            xmin("Text height", 1e-9, |e| if let Kind::Table(t) = &e.kind { t.h } else { 0.0 }, |e, v| if let Kind::Table(t) = &mut e.kind { t.h = v }),
            x("Position X", |e| if let Kind::Table(t) = &e.kind { t.p.x } else { 0.0 }, |e, v| if let Kind::Table(t) = &mut e.kind { t.p.x = v }),
            x("Position Y", |e| if let Kind::Table(t) = &e.kind { t.p.y } else { 0.0 }, |e, v| if let Kind::Table(t) = &mut e.kind { t.p.y = v }),
            ang("Rotation", |e| if let Kind::Table(t) = &e.kind { t.rot } else { 0.0 }, |e, v| if let Kind::Table(t) = &mut e.kind { t.rot = v }),
        ],
        Kind::Image(_) => vec![
            x("Position X", |e| if let Kind::Image(i) = &e.kind { i.p.x } else { 0.0 }, |e, v| if let Kind::Image(i) = &mut e.kind { i.p.x = v }),
            x("Position Y", |e| if let Kind::Image(i) = &e.kind { i.p.y } else { 0.0 }, |e, v| if let Kind::Image(i) = &mut e.kind { i.p.y = v }),
            xmin("Width", 1e-9, |e| if let Kind::Image(i) = &e.kind { i.size().0 } else { 0.0 }, |e, v| {
                // the proportions are kept: width and height scale together
                if let Kind::Image(i) = &mut e.kind {
                    let k = v / i.size().0.max(1e-12);
                    i.u = i.u * k;
                    i.v = i.v * k;
                }
            }),
            xmin("Height", 1e-9, |e| if let Kind::Image(i) = &e.kind { i.size().1 } else { 0.0 }, |e, v| {
                if let Kind::Image(i) = &mut e.kind {
                    let k = v / i.size().1.max(1e-12);
                    i.u = i.u * k;
                    i.v = i.v * k;
                }
            }),
            ro("Rotation", |e, _| if let Kind::Image(i) = &e.kind { Val::Text(format!("{}°", fmt_num(deg(norm_angle(i.rotation())), 4, true))) } else { Val::Text(String::new()) }),
            x("Brightness", |e| if let Kind::Image(i) = &e.kind { i.brightness as f64 } else { 0.0 }, |e, v| if let Kind::Image(i) = &mut e.kind { i.brightness = v.round().clamp(0.0, 100.0) as i32 }),
            x("Contrast", |e| if let Kind::Image(i) = &e.kind { i.contrast as f64 } else { 0.0 }, |e, v| if let Kind::Image(i) = &mut e.kind { i.contrast = v.round().clamp(0.0, 100.0) as i32 }),
            x("Fade", |e| if let Kind::Image(i) = &e.kind { i.fade as f64 } else { 0.0 }, |e, v| if let Kind::Image(i) = &mut e.kind { i.fade = v.round().clamp(0.0, 100.0) as i32 }),
            ro("Image", |e, _| if let Kind::Image(i) = &e.kind { Val::Text(i.name.clone()) } else { Val::Text(String::new()) }),
        ],
        Kind::Wipeout(_) => vec![ro_n("Vertices", |e| if let Kind::Wipeout(w) = &e.kind { w.pts.len() as f64 } else { 0.0 })],
        Kind::Region(_) => vec![
            ro_n("Loops", |e| if let Kind::Region(r) = &e.kind { r.loops.len() as f64 } else { 0.0 }),
            ro_n("Area", |e| if let Kind::Region(r) = &e.kind { r.loops.iter().enumerate().map(|(i, l)| if i == 0 { pline_area(l).abs() } else { -pline_area(l).abs() }).sum() } else { 0.0 }),
            ro_n("Perimeter", |e| if let Kind::Region(r) = &e.kind { r.loops.iter().flat_map(|l| pline_segs(l, true)).map(|s| s.length()).sum() } else { 0.0 }),
        ],
    }
}

// ---------------------------------------------------------------- state

#[derive(Default)]
pub struct PalState {
    pub filter: String,
    pub collapsed: HashSet<&'static str>,
    pub tool_tab: usize,
    pub lm_open: bool,
    pub lm_sel: Vec<String>,
    pub lm_filter_used: bool,
    pub lm_search: String,
    pub lm_edit: Option<(String, String)>,
    pub lm_pick: Option<(&'static str, Vec<String>)>,
    pub width: f32,
}

const TOOL_SETS: [(&str, &[&str]); 4] = [
    ("Draw", &["LINE", "PLINE", "CIRCLE", "ARC", "RECTANG", "POLYGON", "ELLIPSE", "SPLINE", "POINT", "XLINE", "RAY", "DONUT", "HATCH", "BOUNDARY"]),
    ("Modify", &["MOVE", "COPY", "ROTATE", "SCALE", "MIRROR", "OFFSET", "TRIM", "EXTEND", "FILLET", "CHAMFER", "STRETCH", "EXPLODE", "ERASE", "JOIN", "BREAK", "LENGTHEN", "PEDIT", "ARRAYRECT", "ARRAYPOLAR"]),
    ("Annotation", &["MTEXT", "TEXT", "DIMLINEAR", "DIMALIGNED", "DIMANGULAR", "DIMRADIUS", "DIMDIAMETER", "DIMBASELINE", "DIMCONTINUE", "MLEADER"]),
    ("3D", &["BOX", "CYLINDER", "SPHERE", "CONE", "WEDGE", "EXTRUDE", "PRESSPULL", "UNION", "SUBTRACT", "INTERSECT"]),
];

impl CadForgeApp {
    pub fn palettes_ui(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        let t = self.t;
        if self.ui.show_props || self.ui.show_tools {
            let w = if self.pal.width > 0.0 { self.pal.width } else { 300.0 };
            let resp = egui::Panel::left("palette").resizable(true).default_size(w).size_range(220.0..=560.0).show_separator_line(true).frame(egui::Frame::new().fill(t.chrome2).inner_margin(Margin::ZERO)).show(ui, |ui| {
                if self.ui.show_props && !self.ui.show_tools {
                    self.properties_palette(ui, ctx);
                } else {
                    self.tool_palette(ui, ctx);
                }
            });
            self.pal.width = resp.response.rect.width();
        }
        self.layer_manager(ctx);
        self.quick_props(ctx);
    }

    fn palette_header(&mut self, ui: &mut egui::Ui, title: &str, close: impl FnOnce(&mut CadForgeApp)) {
        let t = self.t;
        let (r, _) = ui.allocate_exact_size(vec2(ui.available_width(), 26.0), Sense::hover());
        ui.painter().rect_filled(r, 0.0, t.ribbon);
        ui.painter().line_segment([r.left_bottom(), r.right_bottom()], Stroke::new(1.0, t.border));
        ui.painter().text(r.left_center() + vec2(10.0, 0.0), Align2::LEFT_CENTER, title, FontId::new(12.5, FontFamily::Name("bold".into())), t.text);
        let xr = Rect::from_center_size(r.right_center() - vec2(14.0, 0.0), vec2(18.0, 18.0));
        let resp = ui.interact(xr, ui.id().with(("pal-x", title)), Sense::click());
        if resp.hovered() {
            ui.painter().rect_filled(xr, CornerRadius::same(3), t.hover);
        }
        self.icons.paint(ui.painter(), "x", xr.shrink(5.0), &t, Tint::Mono(t.text2));
        if resp.clicked() {
            close(self);
        }
    }

    // ------------------------------------------------------------ Properties palette

    fn properties_palette(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        let t = self.t;
        self.palette_header(ui, "Properties", |a| a.ui.show_props = false);
        let snapshot = {
            let a = self.h.borrow();
            let Some(ed) = a.editor() else { return };
            (ed.selected().into_iter().cloned().collect::<Vec<Entity>>(), ed.selected_solids().into_iter().cloned().collect::<Vec<_>>())
        };
        let (sel, solids) = snapshot;
        egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
            ui.spacing_mut().item_spacing = vec2(6.0, 3.0);
            ui.add_space(4.0);
            if sel.is_empty() && !solids.is_empty() {
                self.properties_solids(ui, ctx, &solids);
                return;
            }
            let types: Vec<&'static str> = {
                let mut v: Vec<&'static str> = vec![];
                for e in &sel {
                    if !v.contains(&e.label()) {
                        v.push(e.label());
                    }
                }
                v
            };
            if self.pal.filter != "all" && !types.iter().any(|t| *t == self.pal.filter) {
                self.pal.filter = "all".into();
            }
            // selection filter + quick select
            ui.horizontal(|ui| {
                let label = if sel.is_empty() { "No selection".to_string() } else if self.pal.filter == "all" { format!("All ({})", sel.len()) } else { format!("{} ({})", self.pal.filter, sel.iter().filter(|e| e.label() == self.pal.filter).count()) };
                egui::ComboBox::from_id_salt("pal-filter").selected_text(label).width(ui.available_width() - 34.0).show_ui(ui, |ui| {
                    ui.selectable_value(&mut self.pal.filter, "all".to_string(), format!("All ({})", sel.len()));
                    for ty in &types {
                        ui.selectable_value(&mut self.pal.filter, ty.to_string(), format!("{ty} ({})", sel.iter().filter(|e| e.label() == *ty).count()));
                    }
                });
                let (r, resp) = ui.allocate_exact_size(vec2(24.0, 24.0), Sense::click());
                if resp.hovered() {
                    ui.painter().rect_filled(r, CornerRadius::same(3), t.hover);
                }
                self.icons.paint(ui.painter(), "qselect", r.shrink(4.0), &t, Tint::Normal);
                if resp.on_hover_text("Quick Select").clicked() {
                    self.run_command("QSELECT", Args::none());
                }
            });
            let targets: Vec<Entity> = sel.iter().filter(|e| self.pal.filter == "all" || e.label() == self.pal.filter).cloned().collect();
            let ids: Vec<u64> = targets.iter().map(|e| e.id).collect();
            // ---- General
            self.pal_section(ui, "General", |app, ui| {
                if targets.is_empty() {
                    app.general_current(ui);
                } else {
                    app.general_selected(ui, &targets, &ids);
                }
            });
            // ---- Geometry
            if !targets.is_empty() {
                let one_type = targets.iter().all(|e| e.label() == targets[0].label());
                if one_type {
                    let fields = geom_fields(&targets[0]);
                    if !fields.is_empty() {
                        let ids2 = ids.clone();
                        let tg = targets.clone();
                        self.pal_section(ui, "Geometry", |app, ui| {
                            for (i, f) in fields.iter().enumerate() {
                                app.geom_field(ui, i, f, &tg, &ids2);
                            }
                        });
                    }
                } else {
                    self.pal_section(ui, "Geometry", |_, ui| {
                        ui.label(RichText::new("Select a single object type to edit its geometry.").small().color(t.text3));
                    });
                }
            } else {
                // view / drawing info
                let (cx, cy, vh, vw, prec, name, nobj, nlayers, units) = {
                    let a = self.h.borrow();
                    let ed = a.editor().expect("editor");
                    let d = &ed.doc;
                    (ed.view.target.x, ed.view.target.y, ed.view.h / ed.view.scale, ed.view.w / ed.view.scale, d.settings.units.precision.max(0) as usize, d.name.clone(), d.entities().len(), d.layers.len(), d.settings.units.ins_units)
                };
                self.pal_section(ui, "View", |_, ui| {
                    for (k, v) in [("Center X", cx), ("Center Y", cy), ("Height", vh), ("Width", vw)] {
                        ro_row(ui, &t, k, &fmt_num(v, prec, true));
                    }
                });
                self.pal_section(ui, "Misc", |_, ui| {
                    ro_row(ui, &t, "Drawing", &name);
                    ro_row(ui, &t, "Objects", &nobj.to_string());
                    ro_row(ui, &t, "Layers", &nlayers.to_string());
                    ro_row(ui, &t, "Units", ["Unitless", "Inches", "Feet", "", "Millimeters", "Centimeters", "Meters"].get(units as usize).copied().unwrap_or("Unitless"));
                });
            }
        });
    }

    fn pal_section(&mut self, ui: &mut egui::Ui, title: &'static str, add: impl FnOnce(&mut CadForgeApp, &mut egui::Ui)) {
        let t = self.t;
        let open = !self.pal.collapsed.contains(title);
        let (r, resp) = ui.allocate_exact_size(vec2(ui.available_width(), 22.0), Sense::click());
        ui.painter().rect_filled(r, 0.0, t.ribbon_title);
        crate::chrome::chevron(ui.painter(), r.left_center() + vec2(12.0, 0.0), 3.5, !open, t.text2);
        ui.painter().text(r.left_center() + vec2(24.0, 0.0), Align2::LEFT_CENTER, title, FontId::new(12.0, FontFamily::Name("bold".into())), t.text);
        if resp.clicked() {
            if open {
                self.pal.collapsed.insert(title);
            } else {
                self.pal.collapsed.remove(title);
            }
        }
        if open {
            egui::Frame::new().inner_margin(Margin::symmetric(8, 4)).show(ui, |ui| {
                ui.set_width(ui.available_width());
                add(self, ui);
            });
        }
    }

    fn color_button(&self, ui: &mut egui::Ui, value: Option<Option<Color>>) -> egui::Response {
        let t = self.t;
        let (r, resp) = ui.allocate_exact_size(vec2(ui.available_width().max(60.0), 22.0), Sense::click());
        ui.painter().rect_filled(r, CornerRadius::same(3), t.input);
        ui.painter().rect_stroke(r, CornerRadius::same(3), Stroke::new(1.0, if resp.hovered() { t.accent } else { t.border }), StrokeKind::Inside);
        let (rgb, text) = match value {
            Some(c) => (color_rgb_of(c, &None, t.dark).or(if c.is_none() { None } else { Some([136, 136, 136]) }), describe_color(c)),
            None => (Some([136, 136, 136]), "*VARIES*".to_string()),
        };
        swatch(ui.painter(), Rect::from_center_size(r.left_center() + vec2(14.0, 0.0), vec2(12.0, 12.0)), rgb, &t);
        ui.painter().text(r.left_center() + vec2(28.0, 0.0), Align2::LEFT_CENTER, text, FontId::proportional(12.0), t.text);
        resp
    }

    fn general_current(&mut self, ui: &mut egui::Ui) {
        let t = self.t;
        let (cur, layers, ltscale) = {
            let a = self.h.borrow();
            let d = a.doc().expect("doc");
            (d.cur.clone(), d.layers.iter().map(|l| l.name.clone()).collect::<Vec<_>>(), d.settings.ltscale)
        };
        pal_row(self.t, ui, "Color", |ui| {
            if self.color_button(ui, Some(cur.color)).clicked() {
                self.dialog = Some(crate::dialogs::Dialog::color_for(cur.color, true, crate::dialogs::ColorTarget::Property));
            }
        });
        let mut layer = cur.layer.clone();
        pal_row(self.t, ui, "Layer", |ui| {
            egui::ComboBox::from_id_salt("pal-cur-layer").selected_text(&layer).width(ui.available_width() - 4.0).show_ui(ui, |ui| {
                for l in &layers {
                    ui.selectable_value(&mut layer, l.clone(), l);
                }
            });
        });
        if layer != cur.layer {
            self.choose_layer(&layer);
        }
        pal_row(self.t, ui, "Linetype", |ui| {
            ui.label(RichText::new(cur.ltype.as_deref().map_or("ByLayer".to_string(), ltype_display)).color(t.text));
        });
        pal_row(self.t, ui, "Lineweight", |ui| {
            ui.label(RichText::new(lw_display(cur.lw)).color(t.text));
        });
        pal_row(self.t, ui, "Linetype scale", |ui| {
            ui.label(RichText::new(fmt_num(ltscale, 6, true)).color(t.text));
        });
    }

    fn general_selected(&mut self, ui: &mut egui::Ui, targets: &[Entity], _ids: &[u64]) {
        let layers: Vec<String> = self.h.borrow().doc().map(|d| d.layers.iter().map(|l| l.name.clone()).collect()).unwrap_or_default();
        let same = |f: &dyn Fn(&Entity) -> String| -> Option<String> {
            let first = f(&targets[0]);
            if targets.iter().all(|e| f(e) == first) {
                Some(first)
            } else {
                None
            }
        };
        // colour
        let colors: Vec<Option<Color>> = targets.iter().map(|e| e.color).collect();
        let cval = if colors.iter().all(|c| *c == colors[0]) { Some(colors[0]) } else { None };
        pal_row(self.t, ui, "Color", |ui| {
            if self.color_button(ui, cval).clicked() {
                self.dialog = Some(crate::dialogs::Dialog::color_for(cval.flatten(), true, crate::dialogs::ColorTarget::Property));
            }
        });
        // layer
        let lv = same(&|e| if e.layer.is_empty() { "0".into() } else { e.layer.clone() });
        let mut pick: Option<String> = None;
        pal_row(self.t, ui, "Layer", |ui| {
            egui::ComboBox::from_id_salt("pal-layer").selected_text(lv.clone().unwrap_or_else(|| "*VARIES*".into())).width(ui.available_width() - 4.0).show_ui(ui, |ui| {
                for l in &layers {
                    if ui.selectable_label(lv.as_deref() == Some(l.as_str()), l).clicked() {
                        pick = Some(l.clone());
                    }
                }
            });
        });
        if let Some(l) = pick {
            self.set_property("layer", None, None, Some(l));
        }
        // linetype
        let ltv = same(&|e| e.ltype.clone().map_or("ByLayer".into(), |l| if l == "BYBLOCK" { "ByBlock".into() } else { ltype_display(&l) }));
        let mut pick: Option<Option<String>> = None;
        pal_row(self.t, ui, "Linetype", |ui| {
            egui::ComboBox::from_id_salt("pal-lt").selected_text(ltv.clone().unwrap_or_else(|| "*VARIES*".into())).width(ui.available_width() - 4.0).show_ui(ui, |ui| {
                if ui.selectable_label(false, "ByLayer").clicked() {
                    pick = Some(None);
                }
                if ui.selectable_label(false, "ByBlock").clicked() {
                    pick = Some(Some("BYBLOCK".into()));
                }
                for l in all_linetypes().iter() {
                    if ui.selectable_label(false, ltype_display(l.name)).clicked() {
                        pick = Some(Some(l.name.to_string()));
                    }
                }
            });
        });
        if let Some(l) = pick {
            self.set_property("ltype", None, None, l);
        }
        // lineweight
        let lwv = same(&|e| lw_display(e.lw));
        let mut pick: Option<Option<i32>> = None;
        pal_row(self.t, ui, "Lineweight", |ui| {
            egui::ComboBox::from_id_salt("pal-lw").selected_text(lwv.clone().unwrap_or_else(|| "*VARIES*".into())).width(ui.available_width() - 4.0).show_ui(ui, |ui| {
                if ui.selectable_label(false, "ByLayer").clicked() {
                    pick = Some(None);
                }
                if ui.selectable_label(false, "ByBlock").clicked() {
                    pick = Some(Some(-2));
                }
                if ui.selectable_label(false, "Default").clicked() {
                    pick = Some(Some(-3));
                }
                for w in LINEWEIGHTS {
                    if ui.selectable_label(false, lw_display(Some(w))).clicked() {
                        pick = Some(Some(w));
                    }
                }
            });
        });
        if let Some(l) = pick {
            self.set_property("lw", None, l, None);
        }
        // annotative (text, leaders, dimensions, hatches and block references)
        if targets.iter().all(|e| cadforge_core::annotative::size_of(e).is_some()) {
            let av = same(&|e| if e.anno.is_some() { "Yes".into() } else { "No".into() });
            let mut pick: Option<bool> = None;
            pal_row(self.t, ui, "Annotative", |ui| {
                egui::ComboBox::from_id_salt("pal-anno").selected_text(av.clone().unwrap_or_else(|| "*VARIES*".into())).width(ui.available_width() - 4.0).show_ui(ui, |ui| {
                    for (label, on) in [("Yes", true), ("No", false)] {
                        if ui.selectable_label(av.as_deref() == Some(label), label).clicked() {
                            pick = Some(on);
                        }
                    }
                });
            });
            if let Some(on) = pick {
                let ids: Vec<u64> = targets.iter().map(|e| e.id).collect();
                self.edit_doc("Annotative", move |d| {
                    for id in &ids {
                        d.set_annotative(*id, on);
                    }
                });
            }
        }
    }

    fn geom_field(&mut self, ui: &mut egui::Ui, idx: usize, f: &Field, targets: &[Entity], ids: &[u64]) {
        let t = self.t;
        let doc_prec = self.h.borrow().doc().map_or(4, |d| d.settings.units.precision.max(0) as usize);
        let vals: Vec<Val> = {
            let a = self.h.borrow();
            let d = a.doc().expect("doc");
            targets.iter().map(|e| (f.get)(e, d)).collect()
        };
        let same = vals.iter().all(|v| match (v, &vals[0]) {
            (Val::Num(a), Val::Num(b)) => (a - b).abs() < 1e-9,
            (a, b) => a == b,
        });
        let shown = if same { Some(vals[0].clone()) } else { None };
        let label = match f.unit {
            Some(u) => format!("{} ({u})", f.label),
            None => f.label.to_string(),
        };
        let fmt = |v: &Val| match v {
            Val::Num(n) => fmt_num(*n, doc_prec, true),
            Val::Text(s) => s.clone(),
            Val::Bool(b) => if *b { "Yes".into() } else { "No".into() },
        };
        let id = ui.id().with(("geom", idx, f.label));
        let mut applied: Option<Val> = None;
        pal_row(self.t, ui, &label, |ui| match &f.kind {
            FKind::Ro => {
                ui.label(RichText::new(shown.as_ref().map_or("*VARIES*".to_string(), fmt)).color(t.text));
            }
            FKind::Bool => {
                let cur = matches!(shown, Some(Val::Bool(true)));
                egui::ComboBox::from_id_salt(id).selected_text(shown.as_ref().map_or("*VARIES*".to_string(), fmt)).width(ui.available_width() - 4.0).show_ui(ui, |ui| {
                    if ui.selectable_label(cur, "Yes").clicked() {
                        applied = Some(Val::Bool(true));
                    }
                    if ui.selectable_label(!cur && shown.is_some(), "No").clicked() {
                        applied = Some(Val::Bool(false));
                    }
                });
            }
            FKind::Select(opts) => {
                egui::ComboBox::from_id_salt(id).selected_text(shown.as_ref().map_or("*VARIES*".to_string(), fmt)).width(ui.available_width() - 4.0).show_ui(ui, |ui| {
                    for o in opts {
                        if ui.selectable_label(false, o).clicked() {
                            applied = Some(Val::Text(o.clone()));
                        }
                    }
                });
            }
            FKind::Text | FKind::Num { .. } => {
                let is_num = matches!(f.kind, FKind::Num { .. });
                let base = shown.as_ref().map(|v| fmt(v).replace('\n', " ⏎ ")).unwrap_or_default();
                let mut buf: String = ui.data_mut(|d| d.get_temp::<String>(id)).unwrap_or_else(|| base.clone());
                let had_focus = ui.memory(|m| m.has_focus(id));
                if !had_focus {
                    buf = base.clone();
                }
                let r = ui.add(egui::TextEdit::singleline(&mut buf).id(id).desired_width(ui.available_width() - 4.0).hint_text(if shown.is_none() { "*VARIES*" } else { "" }).font(if is_num { FontId::monospace(12.0) } else { FontId::proportional(12.0) }));
                ui.data_mut(|d| d.insert_temp(id, buf.clone()));
                if r.lost_focus() && !ui.input(|i| i.key_pressed(egui::Key::Escape)) && buf != base {
                    if is_num {
                        if let (Ok(v), FKind::Num { min, max }) = (buf.trim().parse::<f64>(), &f.kind) {
                            if v.is_finite() && min.map_or(true, |m| v >= m) && max.map_or(true, |m| v <= m) {
                                applied = Some(Val::Num(v));
                            }
                        }
                    } else {
                        applied = Some(Val::Text(buf.replace(" ⏎ ", "\n")));
                    }
                }
                if r.lost_focus() {
                    ui.data_mut(|d| d.remove::<String>(id));
                }
            }
        });
        if let (Some(v), Some(set)) = (applied, f.set.as_ref()) {
            let ids = ids.to_vec();
            self.edit_doc("Properties", |d| {
                d.will_change();
                for id in &ids {
                    if d.by_id(*id).is_some_and(|e| d.is_locked(e)) {
                        continue;
                    }
                    if let Some(e) = d.by_id_mut(*id) {
                        set(e, &v);
                    }
                }
                d.touch();
            });
        }
    }

    fn properties_solids(&mut self, ui: &mut egui::Ui, _ctx: &egui::Context, solids: &[cadforge_core::solid::Solid]) {
        use cadforge_core::solid::mesh::{mesh_area, mesh_bounds, mesh_centroid, mesh_volume};
        let t = self.t;
        let prec = self.h.borrow().doc().map_or(4, |d| d.settings.units.precision.max(0) as usize);
        ui.horizontal(|ui| {
            egui::ComboBox::from_id_salt("pal-solid-sel").selected_text(format!("Solid ({})", solids.len())).show_ui(ui, |_| {});
        });
        let layers: Vec<String> = self.h.borrow().doc().map(|d| d.layers.iter().map(|l| l.name.clone()).collect()).unwrap_or_default();
        let ids: Vec<u64> = solids.iter().map(|s| s.id).collect();
        let colors: Vec<Option<Color>> = solids.iter().map(|s| s.color).collect();
        let cval = if colors.iter().all(|c| *c == colors[0]) { Some(colors[0]) } else { None };
        self.pal_section(ui, "General", |app, ui| {
            pal_row(app.t, ui, "Color", |ui| {
                if app.color_button(ui, cval).clicked() {
                    app.dialog = Some(crate::dialogs::Dialog::color_for(cval.flatten(), true, crate::dialogs::ColorTarget::Property));
                }
            });
            let lv = {
                let f = if solids[0].layer.is_empty() { "0".to_string() } else { solids[0].layer.clone() };
                if solids.iter().all(|s| (if s.layer.is_empty() { "0" } else { &s.layer }) == f) {
                    Some(f)
                } else {
                    None
                }
            };
            let mut pick: Option<String> = None;
            pal_row(app.t, ui, "Layer", |ui| {
                egui::ComboBox::from_id_salt("pal-solid-layer").selected_text(lv.clone().unwrap_or_else(|| "*VARIES*".into())).width(ui.available_width() - 4.0).show_ui(ui, |ui| {
                    for l in &layers {
                        if ui.selectable_label(false, l).clicked() {
                            pick = Some(l.clone());
                        }
                    }
                });
            });
            if let Some(l) = pick {
                let ids = ids.clone();
                app.edit_doc("Properties", move |d| {
                    d.will_change();
                    for id in &ids {
                        if let Some(s) = d.solid_by_id_mut(*id) {
                            s.layer = l.clone();
                        }
                    }
                    d.touch();
                });
            }
        });
        if solids.len() == 1 {
            let s = &solids[0];
            let m = s.mesh();
            let b = mesh_bounds(&s.vertices);
            let c = mesh_centroid(&m);
            let f = |v: f64| fmt_num(v, prec, true);
            let rows = vec![
                ("Name", if s.name.is_empty() { "Solid".to_string() } else { s.name.clone() }),
                ("Volume", f(mesh_volume(&m))),
                ("Surface area", f(mesh_area(&m))),
                ("Triangles", s.faces.len().to_string()),
                ("Length (X)", f(b.max.x - b.min.x)),
                ("Width (Y)", f(b.max.y - b.min.y)),
                ("Height (Z)", f(b.max.z - b.min.z)),
                ("Centroid X", f(c.x)),
                ("Centroid Y", f(c.y)),
                ("Centroid Z", f(c.z)),
                ("History", s.program.as_ref().map_or("None".to_string(), |p| format!("{} extrusion feature{}", p.features.len(), if p.features.len() == 1 { "" } else { "s" }))),
            ];
            self.pal_section(ui, "Geometry", |_, ui| {
                for (k, v) in &rows {
                    ro_row(ui, &t, k, v);
                }
            });
        }
    }

    // ------------------------------------------------------------ Tool palettes

    fn tool_palette(&mut self, ui: &mut egui::Ui, _ctx: &egui::Context) {
        let t = self.t;
        self.palette_header(ui, "Tool Palettes", |a| a.ui.show_tools = false);
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 1.0;
            // the last tab lists the blocks of the drawing (BLOCKPALETTE)
            let tabs: Vec<&str> = TOOL_SETS.iter().map(|(n, _)| *n).chain(std::iter::once("Blocks")).collect();
            for (i, name) in tabs.iter().enumerate() {
                let active = self.pal.tool_tab.min(tabs.len() - 1) == i;
                let b = egui::Button::new(RichText::new(*name).color(if active { t.text } else { t.text2 })).fill(if active { t.raised } else { Color32::TRANSPARENT }).stroke(Stroke::new(1.0, if active { t.sep } else { Color32::TRANSPARENT }));
                if ui.add(b).clicked() {
                    self.pal.tool_tab = i;
                }
            }
        });
        self.pal.tool_tab = self.pal.tool_tab.min(TOOL_SETS.len());
        if self.pal.tool_tab >= TOOL_SETS.len() {
            self.block_tab(ui);
            return;
        }
        let names = TOOL_SETS[self.pal.tool_tab.min(TOOL_SETS.len() - 1)].1;
        egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
            ui.add_space(4.0);
            let w = ui.available_width();
            let cols = ((w / 92.0) as usize).max(1);
            let cw = w / cols as f32;
            for (row, chunk) in names.chunks(cols).enumerate() {
                let y0 = ui.cursor().top();
                let (rr, _) = ui.allocate_exact_size(vec2(w, 62.0), Sense::hover());
                let _ = (row, y0);
                for (i, name) in chunk.iter().enumerate() {
                    let Some(def) = self.h.borrow().registry.get(name) else { continue };
                    let r = Rect::from_min_size(rr.min + vec2(i as f32 * cw + 2.0, 0.0), vec2(cw - 4.0, 60.0));
                    let resp = ui.interact(r, ui.id().with(("tool", *name)), Sense::click());
                    if resp.hovered() {
                        ui.painter().rect_filled(r, CornerRadius::same(3), t.hover);
                        ui.painter().rect_stroke(r, CornerRadius::same(3), Stroke::new(1.0, t.sep), StrokeKind::Inside);
                    }
                    let icon = if def.icon.is_empty() { name.to_lowercase() } else { def.icon.clone() };
                    self.icons.paint(ui.painter(), &icon, Rect::from_center_size(Pos2::new(r.center().x, r.top() + 22.0), vec2(26.0, 26.0)), &t, Tint::Normal);
                    ui.painter().text(Pos2::new(r.center().x, r.top() + 46.0), Align2::CENTER_CENTER, &def.title, FontId::proportional(11.0), t.text);
                    let resp = self.cmd_tip(resp, name, None);
                    if resp.clicked() {
                        self.run_command(name, Args::none());
                    }
                }
            }
        });
    }

    /// The block palette: every block of the drawing, a click inserts it.
    fn block_tab(&mut self, ui: &mut egui::Ui) {
        let t = self.t;
        let blocks: Vec<(String, usize, bool)> = {
            let a = self.h.borrow();
            a.doc().map(|d| d.blocks.values().filter(|b| !b.name.starts_with('*')).map(|b| (b.name.clone(), b.entities.len(), b.xref.is_some())).collect()).unwrap_or_default()
        };
        if blocks.is_empty() {
            ui.add_space(8.0);
            ui.label(RichText::new("This drawing has no blocks yet. Create one with BLOCK or attach a drawing with XATTACH.").color(t.text3));
            return;
        }
        egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
            ui.add_space(4.0);
            for (name, n, xref) in &blocks {
                let (r, resp) = ui.allocate_exact_size(vec2(ui.available_width(), 34.0), Sense::click());
                if resp.hovered() {
                    ui.painter().rect_filled(r, CornerRadius::same(3), t.hover);
                    ui.painter().rect_stroke(r, CornerRadius::same(3), Stroke::new(1.0, t.sep), StrokeKind::Inside);
                }
                self.icons.paint(ui.painter(), if *xref { "xref" } else { "block" }, Rect::from_center_size(Pos2::new(r.left() + 20.0, r.center().y), vec2(24.0, 24.0)), &t, Tint::Normal);
                ui.painter().text(Pos2::new(r.left() + 40.0, r.center().y - 7.0), Align2::LEFT_CENTER, name, FontId::proportional(12.0), t.text);
                ui.painter().text(Pos2::new(r.left() + 40.0, r.center().y + 8.0), Align2::LEFT_CENTER, format!("{n} object{}{}", if *n == 1 { "" } else { "s" }, if *xref { " · external reference" } else { "" }), FontId::proportional(10.0), t.text3);
                if resp.on_hover_text("Click to insert this block").clicked() {
                    self.run_command("INSERT", Args::none().with("name", cadforge_engine::runtime::ArgVal::Str(name.clone())));
                }
            }
        });
    }

    // ------------------------------------------------------------ Quick properties

    fn quick_props(&mut self, ctx: &egui::Context) {
        let show = self.sv("QPMODE") == 1;
        if !show {
            return;
        }
        let t = self.t;
        let (sel, prec): (Vec<Entity>, usize) = {
            let a = self.h.borrow();
            let Some(ed) = a.editor() else { return };
            (ed.selected().into_iter().cloned().collect(), ed.doc.settings.units.precision.max(0) as usize)
        };
        if sel.is_empty() {
            return;
        }
        let Some(vp) = self.vp.rect else { return };
        let e = &sel[0];
        let mut types: Vec<&str> = vec![];
        for s in &sel {
            if !types.contains(&s.label()) {
                types.push(s.label());
            }
        }
        let title = if types.len() == 1 { format!("{}{}", types[0], if sel.len() > 1 { format!(" ({})", sel.len()) } else { String::new() }) } else { format!("All ({})", sel.len()) };
        let mut rows: Vec<(String, String)> = vec![("Layer".into(), if e.layer.is_empty() { "0".into() } else { e.layer.clone() }), ("Color".into(), describe_color(e.color)), ("Linetype".into(), e.ltype.as_deref().map_or("ByLayer".into(), ltype_display))];
        if sel.len() == 1 {
            let a = self.h.borrow();
            if let Some(d) = a.doc() {
                for f in geom_fields(e).into_iter().take(6) {
                    let v = (f.get)(e, d);
                    rows.push((f.label.to_string(), match v {
                        Val::Num(n) => fmt_num(n, prec, true),
                        Val::Text(s) => s,
                        Val::Bool(b) => if b { "Yes".into() } else { "No".into() },
                    }));
                }
            }
        }
        let mut close = false;
        egui::Area::new(egui::Id::new("quickprops")).order(egui::Order::Foreground).fixed_pos(Pos2::new(vp.left() + 12.0, vp.top() + 44.0)).show(ctx, |ui| {
            egui::Frame::new().fill(t.chrome2).stroke(Stroke::new(1.0, t.border)).corner_radius(CornerRadius::same(4)).inner_margin(Margin::symmetric(0, 0)).shadow(egui::epaint::Shadow { offset: [0, 4], blur: 14, spread: 0, color: Color32::from_black_alpha(110) }).show(ui, |ui| {
                ui.set_width(210.0);
                ui.spacing_mut().item_spacing = vec2(0.0, 0.0);
                let (hr, _) = ui.allocate_exact_size(vec2(210.0, 24.0), Sense::hover());
                ui.painter().rect_filled(hr, CornerRadius { nw: 4, ne: 4, sw: 0, se: 0 }, t.ribbon);
                ui.painter().text(hr.left_center() + vec2(8.0, 0.0), Align2::LEFT_CENTER, &title, FontId::new(12.0, FontFamily::Name("bold".into())), t.text);
                let xr = Rect::from_center_size(hr.right_center() - vec2(14.0, 0.0), vec2(18.0, 18.0));
                let resp = ui.interact(xr, ui.id().with("qp-x"), Sense::click());
                self.icons.paint(ui.painter(), "x", xr.shrink(5.0), &t, Tint::Mono(t.text2));
                if resp.clicked() {
                    close = true;
                }
                for (k, v) in &rows {
                    let (r, _) = ui.allocate_exact_size(vec2(210.0, 21.0), Sense::hover());
                    ui.painter().text(r.left_center() + vec2(8.0, 0.0), Align2::LEFT_CENTER, k, FontId::proportional(11.5), t.text2);
                    ui.painter().text(r.right_center() - vec2(8.0, 0.0), Align2::RIGHT_CENTER, v, FontId::new(11.5, FontFamily::Name("bold".into())), t.text);
                }
            });
        });
        if close {
            self.set_sv("QPMODE", -1.0);
        }
    }

    // ------------------------------------------------------------ Layer Properties Manager

    fn layer_manager(&mut self, ctx: &egui::Context) {
        if !self.pal.lm_open {
            return;
        }
        let t = self.t;
        // results of colour / linetype / lineweight pickers started from this window
        if let Some((kind, names)) = self.pal.lm_pick.clone() {
            match kind {
                "ltype" => {
                    if let Some(("linetype", v)) = self.ret_text.take().as_ref().map(|(k, v)| (k.as_str(), v.clone())) {
                        self.pal.lm_pick = None;
                        let (names, v) = (names.clone(), v);
                        self.edit_doc("Layer linetype", move |d| {
                            d.will_change();
                            for n in &names {
                                if let Some(l) = d.layer_mut(n) {
                                    l.ltype = v.clone();
                                }
                            }
                            d.touch();
                        });
                    }
                }
                "lw" => {
                    if let Some(v) = self.ret_lw.take() {
                        self.pal.lm_pick = None;
                        if let Some(v) = v.filter(|v| *v >= 0) {
                            let names = names.clone();
                            self.edit_doc("Layer lineweight", move |d| {
                                d.will_change();
                                for n in &names {
                                    if let Some(l) = d.layer_mut(n) {
                                        l.lw = v;
                                    }
                                }
                                d.touch();
                            });
                        }
                    }
                }
                _ => {}
            }
        }
        let (layers, cur): (Vec<cadforge_core::style::Layer>, String) = {
            let a = self.h.borrow();
            let Some(d) = a.doc() else { return };
            let mut v: Vec<_> = d.layers.iter().filter(|l| !self.pal.lm_filter_used || d.layer_usage(&l.name) || l.name == d.cur.layer).cloned().collect();
            let q = self.pal.lm_search.to_lowercase();
            v.retain(|l| l.name.to_lowercase().contains(&q));
            v.sort_by(|a, b| if a.name == "0" { std::cmp::Ordering::Less } else if b.name == "0" { std::cmp::Ordering::Greater } else { a.name.to_lowercase().cmp(&b.name.to_lowercase()) });
            (v, d.cur.layer.clone())
        };
        let (total, used): (usize, HashSet<String>) = {
            let a = self.h.borrow();
            let d = a.doc().expect("doc");
            (d.layers.len(), d.layers.iter().filter(|l| d.layer_usage(&l.name)).map(|l| l.name.clone()).collect())
        };
        let mut open = true;
        let mut action: Option<LmAction> = None;
        egui::Window::new("Layer Properties Manager").id(egui::Id::new("layerman")).open(&mut open).default_pos(Pos2::new(120.0, 200.0)).default_size(vec2(860.0, 360.0)).min_size(vec2(520.0, 220.0)).resizable(true).collapsible(false).frame(egui::Frame::window(&ctx.global_style()).fill(t.chrome2).stroke(Stroke::new(1.0, t.border)).inner_margin(Margin::ZERO)).show(ctx, |ui| {
            ui.spacing_mut().item_spacing = vec2(6.0, 4.0);
            // toolbar
            ui.horizontal(|ui| {
                ui.add_space(6.0);
                for (icon, tip, act) in [("layers", "New Layer (Alt+N)", LmAction::New), ("trash", "Delete Layer (Alt+D)", LmAction::Delete), ("laycur", "Set Current (Alt+C)", LmAction::SetCurrent)] {
                    let (r, resp) = ui.allocate_exact_size(vec2(26.0, 26.0), Sense::click());
                    if resp.hovered() {
                        ui.painter().rect_filled(r, CornerRadius::same(3), t.hover);
                    }
                    self.icons.paint(ui.painter(), icon, r.shrink(4.0), &t, Tint::Normal);
                    if resp.on_hover_text(tip).clicked() {
                        action = Some(act);
                    }
                }
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.add_space(6.0);
                    ui.add(egui::TextEdit::singleline(&mut self.pal.lm_search).hint_text("Search for layer").desired_width(180.0));
                });
            });
            ui.horizontal_top(|ui| {
                ui.set_min_height(256.0);
                // tree
                ui.vertical(|ui| {
                    ui.set_width(130.0);
                    for (used_only, label) in [(false, "All"), (true, "All Used Layers")] {
                        let (r, resp) = ui.allocate_exact_size(vec2(124.0, 22.0), Sense::click());
                        let sel = self.pal.lm_filter_used == used_only;
                        if sel {
                            ui.painter().rect_filled(r, CornerRadius::same(2), t.accent_soft);
                        } else if resp.hovered() {
                            ui.painter().rect_filled(r, CornerRadius::same(2), t.hover);
                        }
                        self.icons.paint(ui.painter(), "layers", Rect::from_center_size(r.left_center() + vec2(12.0, 0.0), vec2(14.0, 14.0)), &t, Tint::Normal);
                        ui.painter().text(r.left_center() + vec2(26.0, 0.0), Align2::LEFT_CENTER, label, FontId::proportional(12.0), t.text);
                        if resp.clicked() {
                            self.pal.lm_filter_used = used_only;
                        }
                    }
                });
                // table
                egui::ScrollArea::both().auto_shrink([false, false]).min_scrolled_height(250.0).max_height(260.0).show(ui, |ui| {
                    ui.with_layout(egui::Layout::top_down(egui::Align::Min), |ui| {
                    ui.spacing_mut().item_spacing = vec2(0.0, 0.0);
                    let cols: [(&str, f32); 10] = [("S", 24.0), ("Name", 150.0), ("On", 36.0), ("Freeze", 46.0), ("Lock", 40.0), ("Color", 100.0), ("Linetype", 100.0), ("Lineweight", 90.0), ("Plot", 36.0), ("Description", 120.0)];
                    let total_w: f32 = cols.iter().map(|c| c.1).sum();
                    let (hr, _) = ui.allocate_exact_size(vec2(total_w, 22.0), Sense::hover());
                    ui.painter().rect_filled(hr, 0.0, t.ribbon);
                    let mut x = hr.left();
                    for (name, w) in cols {
                        ui.painter().text(Pos2::new(x + 6.0, hr.center().y), Align2::LEFT_CENTER, name, FontId::proportional(11.5), t.text2);
                        x += w;
                    }
                    for l in &layers {
                        let (rr, resp) = ui.allocate_exact_size(vec2(total_w, 24.0), Sense::click());
                        let sel = self.pal.lm_sel.contains(&l.name);
                        if sel {
                            ui.painter().rect_filled(rr, 0.0, t.accent_soft);
                        } else if resp.hovered() {
                            ui.painter().rect_filled(rr, 0.0, t.hover);
                        }
                        let mut x = rr.left();
                        // S
                        if l.name == cur {
                            self.icons.paint(ui.painter(), "check", Rect::from_center_size(Pos2::new(x + 12.0, rr.center().y), vec2(13.0, 13.0)), &t, Tint::Mono(t.ok));
                        } else if used.contains(&l.name) {
                            ui.painter().circle_filled(Pos2::new(x + 12.0, rr.center().y), 3.0, t.text3);
                        }
                        x += cols[0].1;
                        // name (inline rename)
                        let name_r = Rect::from_min_size(Pos2::new(x, rr.top()), vec2(cols[1].1, 24.0));
                        if self.pal.lm_edit.as_ref().is_some_and(|e| e.0 == l.name) {
                            let mut commit = false;
                            if let Some((old, buf)) = self.pal.lm_edit.as_mut() {
                                let _ = old;
                                let r = ui.put(name_r.shrink2(vec2(2.0, 2.0)), egui::TextEdit::singleline(buf));
                                r.request_focus();
                                if r.lost_focus() {
                                    commit = true;
                                }
                            }
                            if commit {
                                action = Some(LmAction::RenameDone);
                            }
                        } else {
                            ui.painter().text(Pos2::new(x + 6.0, rr.center().y), Align2::LEFT_CENTER, &l.name, FontId::proportional(12.0), t.text);
                        }
                        x += cols[1].1;
                        // toggles
                        for (k, (icon, w)) in [(if l.on { "eye" } else { "eyeoff" }, cols[2].1), (if l.frozen { "snow" } else { "sun" }, cols[3].1), (if l.locked { "lock" } else { "unlock" }, cols[4].1)].into_iter().enumerate() {
                            let br = Rect::from_center_size(Pos2::new(x + w / 2.0, rr.center().y), vec2(20.0, 20.0));
                            let bresp = ui.interact(br, ui.id().with(("lm-t", &l.name, k)), Sense::click());
                            if bresp.hovered() {
                                ui.painter().rect_filled(br, CornerRadius::same(2), t.hover);
                            }
                            self.icons.paint(ui.painter(), icon, br.shrink(2.0), &t, Tint::Normal);
                            if bresp.clicked() {
                                action = Some(LmAction::Toggle(l.name.clone(), k as u8));
                            }
                            x += w;
                        }
                        // colour
                        let cr = Rect::from_min_size(Pos2::new(x + 2.0, rr.top() + 2.0), vec2(cols[5].1 - 4.0, 20.0));
                        let cresp = ui.interact(cr, ui.id().with(("lm-c", &l.name)), Sense::click());
                        if cresp.hovered() {
                            ui.painter().rect_stroke(cr, CornerRadius::same(2), Stroke::new(1.0, t.sep), StrokeKind::Inside);
                        }
                        swatch(ui.painter(), Rect::from_center_size(cr.left_center() + vec2(10.0, 0.0), vec2(12.0, 12.0)), color_rgb_of(Some(l.color), &None, t.dark), &t);
                        ui.painter().text(cr.left_center() + vec2(22.0, 0.0), Align2::LEFT_CENTER, describe_color(Some(l.color)), FontId::proportional(11.5), t.text);
                        if cresp.clicked() {
                            action = Some(LmAction::Color(l.name.clone(), l.color));
                        }
                        x += cols[5].1;
                        // linetype
                        let lr = Rect::from_min_size(Pos2::new(x + 2.0, rr.top() + 2.0), vec2(cols[6].1 - 4.0, 20.0));
                        let lresp = ui.interact(lr, ui.id().with(("lm-l", &l.name)), Sense::click());
                        if lresp.hovered() {
                            ui.painter().rect_stroke(lr, CornerRadius::same(2), Stroke::new(1.0, t.sep), StrokeKind::Inside);
                        }
                        ui.painter().text(lr.left_center() + vec2(6.0, 0.0), Align2::LEFT_CENTER, ltype_display(if l.ltype.is_empty() { "Continuous" } else { &l.ltype }), FontId::proportional(11.5), t.text);
                        if lresp.clicked() {
                            action = Some(LmAction::Linetype(l.name.clone(), l.ltype.clone()));
                        }
                        x += cols[6].1;
                        // lineweight
                        let wr = Rect::from_min_size(Pos2::new(x + 2.0, rr.top() + 2.0), vec2(cols[7].1 - 4.0, 20.0));
                        let wresp = ui.interact(wr, ui.id().with(("lm-w", &l.name)), Sense::click());
                        if wresp.hovered() {
                            ui.painter().rect_stroke(wr, CornerRadius::same(2), Stroke::new(1.0, t.sep), StrokeKind::Inside);
                        }
                        ui.painter().text(wr.left_center() + vec2(6.0, 0.0), Align2::LEFT_CENTER, lw_display(Some(l.lw)), FontId::proportional(11.5), t.text);
                        if wresp.clicked() {
                            action = Some(LmAction::Lweight(l.name.clone(), l.lw));
                        }
                        x += cols[7].1;
                        // plot
                        let pr = Rect::from_center_size(Pos2::new(x + cols[8].1 / 2.0, rr.center().y), vec2(20.0, 20.0));
                        let presp = ui.interact(pr, ui.id().with(("lm-p", &l.name)), Sense::click());
                        self.icons.paint(ui.painter(), "printer", pr.shrink(2.0), &t, Tint::Mono(if l.plot { t.text } else { t.text3 }));
                        if !l.plot {
                            ui.painter().line_segment([pr.left_top() + vec2(3.0, 3.0), pr.right_bottom() - vec2(3.0, 3.0)], Stroke::new(1.5, t.err));
                        }
                        if presp.clicked() {
                            action = Some(LmAction::Plot(l.name.clone()));
                        }
                        x += cols[8].1;
                        ui.painter().text(Pos2::new(x + 6.0, rr.center().y), Align2::LEFT_CENTER, &l.desc, FontId::proportional(11.5), t.text3);
                        // row selection
                        if resp.clicked() && action.is_none() {
                            let m = ui.input(|i| i.modifiers);
                            if m.command || m.ctrl {
                                if let Some(i) = self.pal.lm_sel.iter().position(|n| *n == l.name) {
                                    self.pal.lm_sel.remove(i);
                                } else {
                                    self.pal.lm_sel.push(l.name.clone());
                                }
                            } else if m.shift && !self.pal.lm_sel.is_empty() {
                                let names: Vec<&String> = layers.iter().map(|x| &x.name).collect();
                                let a = names.iter().position(|n| **n == *self.pal.lm_sel.last().unwrap()).unwrap_or(0);
                                let b = names.iter().position(|n| **n == l.name).unwrap_or(0);
                                self.pal.lm_sel = names[a.min(b)..=a.max(b)].iter().map(|s| (*s).clone()).collect();
                            } else {
                                self.pal.lm_sel = vec![l.name.clone()];
                            }
                        }
                        if resp.double_clicked() && action.is_none() {
                            if name_r.contains(ui.input(|i| i.pointer.interact_pos().unwrap_or_default())) && l.name != "0" {
                                action = Some(LmAction::Rename(l.name.clone()));
                            } else {
                                action = Some(LmAction::SetCurrentNamed(l.name.clone()));
                            }
                        }
                    }
                    });
                });
            });
            let (sr, _) = ui.allocate_exact_size(vec2(ui.available_width(), 20.0), Sense::hover());
            ui.painter().text(sr.left_center() + vec2(10.0, 0.0), Align2::LEFT_CENTER, format!("{}: {} layers displayed of {} total layers", if self.pal.lm_filter_used { "Used" } else { "All" }, layers.len(), total), FontId::proportional(11.5), t.text3);
        });
        if !open {
            self.pal.lm_open = false;
        }
        if let Some(a) = action {
            self.lm_action(ctx, a);
        }
    }

    fn lm_action(&mut self, _ctx: &egui::Context, a: LmAction) {
        match a {
            LmAction::New => {
                let (name, base) = {
                    let h = self.h.borrow();
                    let d = h.doc().expect("doc");
                    let mut i = 1;
                    while d.layer(&format!("Layer{i}")).is_some() {
                        i += 1;
                    }
                    (format!("Layer{i}"), self.pal.lm_sel.first().and_then(|n| d.layer(n).cloned()))
                };
                let n2 = name.clone();
                self.edit_doc("New layer", move |d| {
                    let mut l = cadforge_core::style::Layer::new(&n2);
                    if let Some(b) = base {
                        l.color = b.color;
                        l.ltype = b.ltype;
                        l.lw = b.lw;
                    }
                    let _ = d.add_layer_with(l);
                });
                self.pal.lm_sel = vec![name.clone()];
                self.pal.lm_edit = Some((name.clone(), name));
            }
            LmAction::Delete => {
                let names = self.pal.lm_sel.clone();
                let mut errs = vec![];
                for n in names {
                    let mut err = None;
                    self.edit_doc("Delete layer", |d| {
                        if let Err(e) = d.delete_layer(&n) {
                            err = Some(e);
                        }
                    });
                    if let Some(e) = err {
                        errs.push(e);
                    }
                }
                if let Some(e) = errs.first() {
                    self.toast(e);
                }
                self.pal.lm_sel.clear();
            }
            LmAction::SetCurrent => {
                if let Some(n) = self.pal.lm_sel.first().cloned() {
                    self.choose_layer(&n);
                }
            }
            LmAction::SetCurrentNamed(n) => self.choose_layer(&n),
            LmAction::Rename(n) => self.pal.lm_edit = Some((n.clone(), n)),
            LmAction::RenameDone => {
                if let Some((old, new)) = self.pal.lm_edit.take() {
                    let new = new.trim().to_string();
                    if !new.is_empty() && new != old {
                        let mut err = None;
                        let (o2, n2) = (old.clone(), new.clone());
                        self.edit_doc("Rename layer", |d| {
                            if let Err(e) = d.rename_layer(&o2, &n2) {
                                err = Some(e);
                            }
                        });
                        match err {
                            Some(e) => self.toast(&e),
                            None => self.pal.lm_sel = vec![new],
                        }
                    }
                }
            }
            LmAction::Toggle(n, k) => self.layer_toggle(&n, k),
            LmAction::Plot(n) => {
                self.edit_doc("Layer plot", move |d| {
                    d.will_change();
                    if let Some(l) = d.layer_mut(&n) {
                        l.plot = !l.plot;
                    }
                    d.touch();
                });
            }
            LmAction::Color(n, c) => {
                self.dialog = Some(crate::dialogs::Dialog::color_for(Some(c), false, crate::dialogs::ColorTarget::Layer(n)));
            }
            LmAction::Linetype(n, v) => {
                let names = if self.pal.lm_sel.contains(&n) { self.pal.lm_sel.clone() } else { vec![n] };
                self.pal.lm_pick = Some(("ltype", names));
                self.dialog = Some(crate::dialogs::Dialog::linetype_pick(Some(v)));
            }
            LmAction::Lweight(n, v) => {
                let names = if self.pal.lm_sel.contains(&n) { self.pal.lm_sel.clone() } else { vec![n] };
                self.pal.lm_pick = Some(("lw", names));
                self.dialog = Some(crate::dialogs::Dialog::lweight_pick(Some(v)));
            }
        }
    }
}

enum LmAction {
    New,
    Delete,
    SetCurrent,
    SetCurrentNamed(String),
    Rename(String),
    RenameDone,
    Toggle(String, u8),
    Plot(String),
    Color(String, Color),
    Linetype(String, String),
    Lweight(String, i32),
}

fn pal_row(t: crate::theme::Tokens, ui: &mut egui::Ui, label: &str, add: impl FnOnce(&mut egui::Ui)) {
    ui.horizontal(|ui| {
        let w = ui.available_width();
        ui.add_sized([w * 0.42, 20.0], egui::Label::new(RichText::new(label).color(t.text2)).truncate());
        add(ui);
    });
}

fn ro_row(ui: &mut egui::Ui, t: &crate::theme::Tokens, k: &str, v: &str) {
    ui.horizontal(|ui| {
        let w = ui.available_width();
        ui.add_sized([w * 0.42, 20.0], egui::Label::new(RichText::new(k).color(t.text2)).truncate());
        ui.label(RichText::new(v).color(t.text));
    });
}
