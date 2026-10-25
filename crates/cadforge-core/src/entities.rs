//! Entity model (port of `src/core/entities.js`). Entities serialise to the same JSON as the web application:
//! `{ id, type, layer, color?, ltype?, lw?, lts?, ...geometry }`.
//!
//! All geometric queries go through [`segs_of`], which reduces any entity to line/arc segments.
use crate::dim::{arrow_polygon, dim_geometry, DimData, DimType, DIM_POINT_KEYS};
use crate::drawing::Drawing;
use crate::geom::*;
use crate::hatch::hatch_lines;
use crate::style::Color;
use crate::text::{text_corners, text_layout, HJust, TextData, VJust};
use serde::{Deserialize, Serialize};
use std::rc::Rc;

// ---------------------------------------------------------------- data

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct EllipseData {
    pub c: Pt,
    /// Major-axis vector (centre to the end of the major axis).
    pub m: Pt,
    pub ratio: f64,
    #[serde(default)]
    pub t0: f64,
    #[serde(default = "tau")]
    pub t1: f64,
}

fn pt_is_zero(p: &Pt) -> bool {
    p.x == 0.0 && p.y == 0.0
}
fn tau() -> f64 {
    TAU
}
fn one() -> f64 {
    1.0
}
fn d25() -> f64 {
    2.5
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct HatchData {
    pub loops: Vec<Vec<Vertex>>,
    #[serde(default = "ansi31")]
    pub pattern: String,
    #[serde(default = "one")]
    pub ps: f64,
    #[serde(default)]
    pub pa: f64,
    /// Gradient fill (GRADIENT); when set the pattern is ignored.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub gradient: Option<GradientData>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct GradientData {
    pub c1: [u8; 3],
    pub c2: [u8; 3],
    #[serde(default)]
    pub angle: f64,
    /// `LINEAR`, `CYLINDER`, `INVCYLINDER`, `SPHERICAL`, `HEMISPHERICAL`, `CURVED` ...
    #[serde(default = "linear")]
    pub kind: String,
    #[serde(default)]
    pub one_color: bool,
    #[serde(default = "tint_half")]
    pub tint: f64,
}

fn linear() -> String {
    "LINEAR".into()
}
fn tint_half() -> f64 {
    0.5
}

fn ansi31() -> String {
    "ANSI31".into()
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct InsertData {
    pub name: String,
    pub p: Pt,
    #[serde(default = "one")]
    pub sx: f64,
    #[serde(default = "one")]
    pub sy: f64,
    #[serde(default)]
    pub rot: f64,
    /// Attribute values of this reference (ATTRIB), positioned in world coordinates.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub attribs: Vec<AttribData>,
    /// XCLIP boundary in world coordinates (convex): only what lies inside is displayed and plotted.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub clip: Vec<Pt>,
    /// The boundary is kept but switched off (XCLIP OFF).
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub clip_off: bool,
}

/// One attribute instance of a block reference.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct AttribData {
    pub tag: String,
    pub t: TextData,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub hidden: bool,
}

/// ACAD_TABLE: a grid of text cells. `p` is the top-left corner; rows run downwards.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TableData {
    pub p: Pt,
    pub cols: Vec<f64>,
    pub rows: Vec<f64>,
    /// `cells[row][col]`
    pub cells: Vec<Vec<String>>,
    #[serde(default = "d25")]
    pub h: f64,
    /// Merged ranges `[row0, col0, row1, col1]` (inclusive).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub merges: Vec<[usize; 4]>,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub style: String,
    /// The first row is a title row (larger text).
    #[serde(default)]
    pub title: bool,
    #[serde(default)]
    pub rot: f64,
    /// The CSV file the cells come from (DATALINK); DATALINKUPDATE reads it again or writes the table to it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub link: Option<DataLink>,
}

/// A table's link to a data file.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct DataLink {
    /// The file, as it was named when the link was made.
    pub file: String,
}

/// WIPEOUT: a polygon that masks what is below it.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct WipeoutData {
    pub pts: Vec<Pt>,
}

/// IMAGE: a raster picture (`name` keys [`crate::drawing::Drawing::images`]). `p` is the lower-left corner; `u` runs
/// along the bottom edge for the whole width and `v` along the left edge for the whole height, so rotation, scale and
/// mirroring are all carried by the two vectors.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ImageData {
    pub name: String,
    pub p: Pt,
    pub u: Pt,
    pub v: Pt,
    /// Clipping polygon in world coordinates (IMAGECLIP); empty for the whole picture.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub clip: Vec<Pt>,
    /// The clipping boundary is switched off (IMAGECLIP OFF) while the polygon is kept.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub clip_off: bool,
    #[serde(default = "fifty")]
    pub brightness: i32,
    #[serde(default = "fifty")]
    pub contrast: i32,
    #[serde(default)]
    pub fade: i32,
}

fn fifty() -> i32 {
    50
}

impl ImageData {
    /// A picture of `w` x `h` drawing units with its lower-left corner at `p`, rotated by `rot` radians.
    pub fn new(name: &str, p: Pt, w: f64, h: f64, rot: f64) -> ImageData {
        let (c, s) = (rot.cos(), rot.sin());
        ImageData { name: name.to_string(), p, u: pt(c * w, s * w), v: pt(-s * h, c * h), clip: vec![], clip_off: false, brightness: 50, contrast: 50, fade: 0 }
    }

    /// The four corners, counter-clockwise from the lower-left.
    pub fn corners(&self) -> [Pt; 4] {
        [self.p, self.p + self.u, self.p + self.u + self.v, self.p + self.v]
    }

    /// The visible boundary: the clip polygon (cut to the picture), or the whole picture.
    pub fn boundary(&self) -> Vec<Pt> {
        if self.clip.len() >= 3 && !self.clip_off {
            let cut = crate::picture::clip_to_convex(&self.clip, &self.corners());
            if cut.len() >= 3 {
                return cut;
            }
            self.clip.clone()
        } else {
            self.corners().to_vec()
        }
    }

    /// Width and height in drawing units.
    pub fn size(&self) -> (f64, f64) {
        (len(self.u), len(self.v))
    }

    /// Rotation of the bottom edge, radians.
    pub fn rotation(&self) -> f64 {
        self.u.y.atan2(self.u.x)
    }
}

/// REGION: one or more closed loops forming a 2D area (outer loop plus holes).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct RegionData {
    pub loops: Vec<Vec<Vertex>>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ViewportData {
    /// Centre of the viewport rectangle on the sheet (paper units, mm).
    pub c: Pt,
    pub w: f64,
    pub h: f64,
    /// Paper units per model unit (1:50 = 0.02). `None` = not set yet: fit the model extents.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scale: Option<f64>,
    /// Model-space point shown at the centre of the viewport.
    #[serde(default, skip_serializing_if = "pt_is_zero")]
    pub vc: Pt,
    /// Display locked: zoom and pan inside the viewport do not change it.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub locked: bool,
    /// Viewport switched off: its border and contents are hidden.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub off: bool,
    /// Non-rectangular clip polygon in paper units (VPCLIP / MVIEW Polygonal).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub clip: Option<Vec<Pt>>,
    /// Layer settings that apply in this viewport only (VPLAYER, layer manager "VP" columns).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub layers: Vec<LayerOverride>,
}

/// What one layout viewport does differently from the drawing's layer table for one layer.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct LayerOverride {
    /// Layer name, compared case-insensitively.
    pub layer: String,
    /// Frozen in this viewport only.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub frozen: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub color: Option<crate::style::Color>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ltype: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lw: Option<i32>,
    /// Transparency in percent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub transp: Option<f64>,
}

impl LayerOverride {
    pub fn is_empty(&self) -> bool {
        !self.frozen && self.color.is_none() && self.ltype.is_none() && self.lw.is_none() && self.transp.is_none()
    }
}

impl ViewportData {
    pub fn new(c: Pt, w: f64, h: f64) -> ViewportData {
        ViewportData { c, w, h, scale: None, vc: Pt::ZERO, locked: false, off: false, clip: None, layers: vec![] }
    }

    /// The override for `layer` in this viewport, if any.
    pub fn layer_override(&self, layer: &str) -> Option<&LayerOverride> {
        self.layers.iter().find(|o| o.layer.eq_ignore_ascii_case(layer))
    }

    /// Whether `layer` is frozen in this viewport.
    pub fn is_frozen(&self, layer: &str) -> bool {
        self.layer_override(layer).is_some_and(|o| o.frozen)
    }

    /// Change the override for `layer` (creating it when missing); an override that no longer changes anything is dropped.
    pub fn edit_override(&mut self, layer: &str, f: impl FnOnce(&mut LayerOverride)) {
        let i = match self.layers.iter().position(|o| o.layer.eq_ignore_ascii_case(layer)) {
            Some(i) => i,
            None => {
                self.layers.push(LayerOverride { layer: layer.to_string(), ..Default::default() });
                self.layers.len() - 1
            }
        };
        f(&mut self.layers[i]);
        if self.layers[i].is_empty() {
            self.layers.remove(i);
        }
    }

    /// Freeze or thaw `layer` in this viewport.
    pub fn set_frozen(&mut self, layer: &str, frozen: bool) {
        self.edit_override(layer, |o| o.frozen = frozen);
    }

    /// Back to the drawing's own settings for `layer`.
    pub fn reset_layer(&mut self, layer: &str) {
        self.layers.retain(|o| !o.layer.eq_ignore_ascii_case(layer));
    }

    /// The viewport outline on the sheet (the rectangle or the clip polygon).
    pub fn outline(&self) -> Vec<Pt> {
        match &self.clip {
            Some(p) if p.len() >= 3 => p.clone(),
            _ => vec![pt(self.c.x - self.w / 2.0, self.c.y - self.h / 2.0), pt(self.c.x + self.w / 2.0, self.c.y - self.h / 2.0), pt(self.c.x + self.w / 2.0, self.c.y + self.h / 2.0), pt(self.c.x - self.w / 2.0, self.c.y + self.h / 2.0)],
        }
    }

    /// Model centre and paper-per-model scale of the viewport; a viewport without a scale fits the model extents.
    pub fn model_view(&self, doc: &Drawing) -> (Pt, f64) {
        if let Some(s) = self.scale {
            if s > 0.0 {
                return (self.vc, s);
            }
        }
        let mut b: Option<Bounds> = None;
        for e in &doc.model.entities {
            if doc.is_visible_in_sheet(e) && !self.is_frozen(&doc.layer_of(e).name) {
                b = Bounds::union_opt(b, bounds_of(e, doc));
            }
        }
        for sd in &doc.solids {
            if doc.solid_visible(sd) {
                let mb = crate::solid::mesh::mesh_bounds(&sd.vertices);
                b = Bounds::union_opt(b, Some(Bounds::new(mb.min.x, mb.min.y, mb.max.x, mb.max.y)));
            }
        }
        match b {
            Some(b) if b.max_x - b.min_x > 1e-9 || b.max_y - b.min_y > 1e-9 => {
                let (bw, bh) = ((b.max_x - b.min_x).max(1e-9), (b.max_y - b.min_y).max(1e-9));
                (b.center(), (self.w / bw).min(self.h / bh) * 0.9)
            }
            Some(b) => (b.center(), 1.0),
            None => (Pt::ZERO, 1.0),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum Kind {
    Line { a: Pt, b: Pt },
    Circle { c: Pt, r: f64 },
    Arc { c: Pt, r: f64, a0: f64, a1: f64 },
    Pline { v: Vec<Vertex>, #[serde(default)] closed: bool },
    Ellipse(EllipseData),
    /// A spline through fit points, or (with `nurbs`) defined exactly by control vertices, weights and knots; `fit`
    /// then holds the fit points a file gave, if any.
    Spline { fit: Vec<Pt>, #[serde(default)] closed: bool, #[serde(default, skip_serializing_if = "Option::is_none")] nurbs: Option<Box<crate::nurbs::Nurbs>> },
    Point { p: Pt },
    Text(TextData),
    Dim(DimData),
    Leader { pts: Vec<Pt>, #[serde(default)] s: String, #[serde(default = "d25")] h: f64 },
    Hatch(HatchData),
    Insert(InsertData),
    Xline { p: Pt, d: Pt },
    Ray { p: Pt, d: Pt },
    Viewport(ViewportData),
    Table(TableData),
    Wipeout(WipeoutData),
    Region(RegionData),
    Image(ImageData),
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Entity {
    #[serde(default)]
    pub id: u64,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub layer: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub color: Option<Color>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ltype: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lw: Option<i32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lts: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub transp: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub grp: Option<String>,
    /// Annotative objects keep their paper size and are sized for the annotation scale (see `annotative.rs`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub anno: Option<crate::annotative::Annotative>,
    #[serde(flatten)]
    pub kind: Kind,
}

impl Entity {
    pub fn new(kind: Kind) -> Entity {
        Entity { id: 0, layer: String::new(), color: None, ltype: None, lw: None, lts: None, transp: None, grp: None, anno: None, kind }
    }
    pub fn line(a: Pt, b: Pt) -> Entity {
        Entity::new(Kind::Line { a, b })
    }
    pub fn circle(c: Pt, r: f64) -> Entity {
        Entity::new(Kind::Circle { c, r })
    }
    pub fn arc(c: Pt, r: f64, a0: f64, a1: f64) -> Entity {
        Entity::new(Kind::Arc { c, r, a0: norm_angle(a0), a1: norm_angle(a1) })
    }
    /// Arc entity from a directed segment; negative sweeps are flipped to CCW.
    pub fn arc_seg(s: &Seg) -> Entity {
        match *s {
            Seg::A { c, r, a0, sw, .. } => {
                if sw >= 0.0 {
                    Entity::arc(c, r, a0, a0 + sw)
                } else {
                    Entity::arc(c, r, a0 + sw, a0)
                }
            }
            Seg::L { a, b, .. } => Entity::line(a, b),
        }
    }
    pub fn pline(v: Vec<Vertex>, closed: bool) -> Entity {
        Entity::new(Kind::Pline { v, closed })
    }
    pub fn pline_pts(pts: &[Pt], closed: bool) -> Entity {
        Entity::pline(pts.iter().map(|p| Vertex::from(*p)).collect(), closed)
    }
    pub fn ellipse(c: Pt, m: Pt, ratio: f64, t0: f64, t1: f64) -> Entity {
        Entity::new(Kind::Ellipse(EllipseData { c, m, ratio, t0, t1 }))
    }
    pub fn spline(fit: Vec<Pt>, closed: bool) -> Entity {
        Entity::new(Kind::Spline { fit, closed, nurbs: None })
    }
    /// A spline defined by its control vertices (and weights and knots).
    pub fn nurbs(n: crate::nurbs::Nurbs) -> Entity {
        let closed = n.closed();
        Entity::new(Kind::Spline { fit: vec![], closed, nurbs: Some(Box::new(n)) })
    }
    pub fn point(p: Pt) -> Entity {
        Entity::new(Kind::Point { p })
    }
    pub fn text(p: Pt, h: f64, s: &str) -> Entity {
        Entity::new(Kind::Text(TextData::new(p, h, s)))
    }
    pub fn mtext(p: Pt, h: f64, s: &str, w: f64) -> Entity {
        let mut t = TextData::new(p, h, s);
        t.w = Some(w);
        t.multi = true;
        t.jv = VJust::Top;
        Entity::new(Kind::Text(t))
    }
    pub fn dim(d: DimData) -> Entity {
        Entity::new(Kind::Dim(d))
    }
    pub fn leader(pts: Vec<Pt>, s: &str, h: f64) -> Entity {
        Entity::new(Kind::Leader { pts, s: s.into(), h })
    }
    pub fn hatch(loops: Vec<Vec<Vertex>>, pattern: &str) -> Entity {
        Entity::new(Kind::Hatch(HatchData { loops, pattern: pattern.into(), ps: 1.0, pa: 0.0, gradient: None }))
    }
    pub fn insert(name: &str, p: Pt) -> Entity {
        Entity::new(Kind::Insert(InsertData { name: name.into(), p, sx: 1.0, sy: 1.0, rot: 0.0, attribs: vec![], clip: vec![], clip_off: false }))
    }
    pub fn xline(p: Pt, d: Pt) -> Entity {
        Entity::new(Kind::Xline { p, d: unit(d) })
    }
    pub fn ray(p: Pt, d: Pt) -> Entity {
        Entity::new(Kind::Ray { p, d: unit(d) })
    }

    pub fn on_layer(mut self, layer: &str) -> Entity {
        self.layer = layer.into();
        self
    }
    pub fn with_color(mut self, c: Color) -> Entity {
        self.color = Some(c);
        self
    }
    pub fn with_ltype(mut self, l: &str) -> Entity {
        self.ltype = Some(l.into());
        self
    }
    pub fn with_lw(mut self, lw: i32) -> Entity {
        self.lw = Some(lw);
        self
    }

    /// Copy of the entity without its id (for previews and COPY).
    pub fn cloned_noid(&self) -> Entity {
        let mut c = self.clone();
        c.id = 0;
        c
    }

    pub fn type_name(&self) -> &'static str {
        match &self.kind {
            Kind::Line { .. } => "line",
            Kind::Circle { .. } => "circle",
            Kind::Arc { .. } => "arc",
            Kind::Pline { .. } => "pline",
            Kind::Ellipse(_) => "ellipse",
            Kind::Spline { .. } => "spline",
            Kind::Point { .. } => "point",
            Kind::Text(_) => "text",
            Kind::Dim(_) => "dim",
            Kind::Leader { .. } => "leader",
            Kind::Hatch(_) => "hatch",
            Kind::Insert(_) => "insert",
            Kind::Xline { .. } => "xline",
            Kind::Ray { .. } => "ray",
            Kind::Viewport(_) => "viewport",
            Kind::Table(_) => "table",
            Kind::Wipeout(_) => "wipeout",
            Kind::Region(_) => "region",
            Kind::Image(_) => "image",
        }
    }

    /// Display name ("Line", "Multiline Text"...).
    pub fn label(&self) -> &'static str {
        match &self.kind {
            Kind::Text(t) if t.multi => "Multiline Text",
            Kind::Line { .. } => "Line",
            Kind::Circle { .. } => "Circle",
            Kind::Arc { .. } => "Arc",
            Kind::Pline { .. } => "Polyline",
            Kind::Ellipse(_) => "Ellipse",
            Kind::Spline { .. } => "Spline",
            Kind::Point { .. } => "Point",
            Kind::Text(_) => "Text",
            Kind::Dim(_) => "Dimension",
            Kind::Leader { .. } => "Leader",
            Kind::Hatch(_) => "Hatch",
            Kind::Insert(_) => "Block Reference",
            Kind::Xline { .. } => "Construction Line",
            Kind::Ray { .. } => "Ray",
            Kind::Viewport(_) => "Viewport",
            Kind::Table(_) => "Table",
            Kind::Wipeout(_) => "Wipeout",
            Kind::Region(_) => "Region",
            Kind::Image(_) => "Image",
        }
    }

    pub fn is_curve(&self) -> bool {
        matches!(self.kind, Kind::Line { .. } | Kind::Circle { .. } | Kind::Arc { .. } | Kind::Pline { .. } | Kind::Ellipse(_) | Kind::Spline { .. })
    }
}

// ---------------------------------------------------------------- ellipse / spline helpers

pub fn ellipse_point(e: &EllipseData, t: f64) -> Pt {
    let n = pt(-e.m.y * e.ratio, e.m.x * e.ratio);
    pt(e.c.x + e.m.x * t.cos() + n.x * t.sin(), e.c.y + e.m.y * t.cos() + n.y * t.sin())
}
pub fn ellipse_sweep(e: &EllipseData) -> f64 {
    let s = sweep_ccw(e.t0, e.t1);
    if s < 1e-12 {
        TAU
    } else {
        s
    }
}
pub fn ellipse_is_full(e: &EllipseData) -> bool {
    ellipse_sweep(e) >= TAU - 1e-9
}

pub fn ellipse_points(e: &EllipseData, n: usize) -> Vec<Pt> {
    let sw = ellipse_sweep(e);
    let steps = 8usize.max((n as f64 * sw / TAU).ceil() as usize);
    (0..=steps).map(|i| ellipse_point(e, e.t0 + sw * i as f64 / steps as f64)).collect()
}

/// Points along a spline: its NURBS when it has one, otherwise the curve through its fit points.
pub fn spline_curve(fit: &[Pt], closed: bool, nurbs: Option<&crate::nurbs::Nurbs>, per_span: usize) -> Vec<Pt> {
    match nurbs {
        Some(n) => n.sample(per_span),
        None => spline_points(fit, closed, per_span),
    }
}

/// Centripetal Catmull-Rom through the fit points; returns a dense point list.
pub fn spline_points(fit: &[Pt], closed: bool, per_span: usize) -> Vec<Pt> {
    let n = fit.len();
    if n < 2 {
        return fit.to_vec();
    }
    let count = if closed { n } else { n - 1 };
    let at = |i: isize| -> Pt {
        if closed {
            fit[((i % n as isize) + n as isize) as usize % n]
        } else {
            fit[(i.clamp(0, n as isize - 1)) as usize]
        }
    };
    let knot = |a: Pt, b: Pt| dist(a, b).sqrt().max(1e-6);
    let mut out = vec![];
    for i in 0..count as isize {
        let p0 = if closed { at(i - 1) } else if i == 0 { pt(2.0 * fit[0].x - fit[1].x, 2.0 * fit[0].y - fit[1].y) } else { at(i - 1) };
        let (p1, p2) = (at(i), at(i + 1));
        let p3 = if closed {
            at(i + 2)
        } else if (i + 2) as usize >= n {
            pt(2.0 * fit[n - 1].x - fit[n - 2].x, 2.0 * fit[n - 1].y - fit[n - 2].y)
        } else {
            at(i + 2)
        };
        let t0 = 0.0;
        let t1 = t0 + knot(p0, p1);
        let t2 = t1 + knot(p1, p2);
        let t3 = t2 + knot(p2, p3);
        for s in 0..per_span {
            let t = t1 + (t2 - t1) * s as f64 / per_span as f64;
            let a1 = lerp(p0, p1, (t - t0) / (t1 - t0));
            let a2 = lerp(p1, p2, (t - t1) / (t2 - t1));
            let a3 = lerp(p2, p3, (t - t2) / (t3 - t2));
            let b1 = lerp(a1, a2, (t - t0) / (t2 - t0));
            let b2 = lerp(a2, a3, (t - t1) / (t3 - t1));
            out.push(lerp(b1, b2, (t - t1) / (t2 - t1)));
        }
    }
    out.push(if closed { fit[0] } else { fit[n - 1] });
    out
}

/// One cubic Bezier piece of a spline.
#[derive(Clone, Copy, Debug)]
pub struct Bezier {
    pub p0: Pt,
    pub c1: Pt,
    pub c2: Pt,
    pub p1: Pt,
}

/// Exact cubic Bezier form of the spline (used for DXF export).
pub fn spline_beziers(fit: &[Pt], closed: bool) -> Vec<Bezier> {
    let n = fit.len();
    if n < 2 {
        return vec![];
    }
    let count = if closed { n } else { n - 1 };
    let at = |i: isize| -> Pt {
        if closed {
            fit[((i % n as isize) + n as isize) as usize % n]
        } else {
            fit[(i.clamp(0, n as isize - 1)) as usize]
        }
    };
    let knot = |a: Pt, b: Pt| dist(a, b).sqrt().max(1e-6);
    let mut out = vec![];
    for i in 0..count as isize {
        let p0 = if closed { at(i - 1) } else if i == 0 { pt(2.0 * fit[0].x - fit[1].x, 2.0 * fit[0].y - fit[1].y) } else { at(i - 1) };
        let (p1, p2) = (at(i), at(i + 1));
        let p3 = if closed {
            at(i + 2)
        } else if (i + 2) as usize >= n {
            pt(2.0 * fit[n - 1].x - fit[n - 2].x, 2.0 * fit[n - 1].y - fit[n - 2].y)
        } else {
            at(i + 2)
        };
        let (d01, d12, d23) = (knot(p0, p1), knot(p1, p2), knot(p2, p3));
        let (t1, t2, t3) = (d01, d01 + d12, d01 + d12 + d23);
        let d = |a: Pt, b: Pt, dt: f64| pt((b.x - a.x) / dt, (b.y - a.y) / dt);
        let m1 = {
            let (u, v, w) = (d(p0, p1, t1), d(p0, p2, t2), d(p1, p2, t2 - t1));
            pt(u.x - v.x + w.x, u.y - v.y + w.y)
        };
        let m2 = {
            let (u, v, w) = (d(p1, p2, t2 - t1), d(p1, p3, t3 - t1), d(p2, p3, t3 - t2));
            pt(u.x - v.x + w.x, u.y - v.y + w.y)
        };
        out.push(Bezier { p0: p1, c1: pt(p1.x + m1.x * d12 / 3.0, p1.y + m1.y * d12 / 3.0), c2: pt(p2.x - m2.x * d12 / 3.0, p2.y - m2.y * d12 / 3.0), p1: p2 });
    }
    out
}

fn lines_from_pts(pts: &[Pt], closed: bool) -> Vec<Seg> {
    let mut out: Vec<Seg> = (0..pts.len().saturating_sub(1)).map(|i| Seg::L { a: pts[i], b: pts[i + 1], approx: true }).collect();
    if closed && pts.len() > 2 {
        out.push(Seg::L { a: pts[pts.len() - 1], b: pts[0], approx: true });
    }
    out
}

// ---------------------------------------------------------------- block references

pub fn insert_matrix(e: &InsertData, base: Pt) -> Mat {
    mat::mul(&mat::mul(&mat::translate(e.p.x, e.p.y), &mat::rotate(e.rot, Pt::ZERO)), &mat::mul(&mat::scale(e.sx, e.sy, Pt::ZERO), &mat::translate(-base.x, -base.y)))
}

fn transform_seg_general(s: &Seg, m: &Mat) -> Option<Seg> {
    if s.is_line() || mat::is_similarity(m) {
        Some(transform_seg(s, m))
    } else {
        None
    }
}

fn transform_segs(segs: &[Seg], m: &Mat) -> Vec<Seg> {
    let mut out = vec![];
    for s in segs {
        match transform_seg_general(s, m) {
            Some(t) => out.push(t),
            None => {
                let r = match s {
                    Seg::A { r, .. } => *r,
                    _ => 1.0,
                };
                let pts: Vec<Pt> = tess_seg(s, 0.01 * 1f64.max(r * 0.01)).into_iter().map(|p| mat::apply(m, p)).collect();
                out.extend(lines_from_pts(&pts, false));
            }
        }
    }
    out
}

const MAX_BLOCK_DEPTH: usize = 12;

// ---------------------------------------------------------------- segments

/// Line/arc segments describing an entity's pickable curve geometry (world coordinates).
pub fn segs_of(e: &Entity, doc: &Drawing) -> Rc<Vec<Seg>> {
    segs_of_depth(e, doc, 0)
}

fn segs_of_depth(e: &Entity, doc: &Drawing, depth: usize) -> Rc<Vec<Seg>> {
    if e.id != 0 {
        if let Some(v) = doc.cache_get_segs(e.id) {
            return v;
        }
    }
    let v = Rc::new(compute_segs(e, doc, depth));
    if e.id != 0 {
        doc.cache_put_segs(e.id, v.clone());
    }
    v
}

fn compute_segs(e: &Entity, doc: &Drawing, depth: usize) -> Vec<Seg> {
    match &e.kind {
        Kind::Line { a, b } => vec![Seg::line(*a, *b)],
        Kind::Circle { c, r } => vec![Seg::A { c: *c, r: *r, a0: 0.0, sw: TAU, full: true }],
        Kind::Arc { c, r, a0, a1 } => {
            let sw = sweep_ccw(*a0, *a1);
            vec![Seg::arc(*c, *r, *a0, if sw < 1e-12 { TAU } else { sw })]
        }
        Kind::Pline { v, closed } => pline_segs(v, *closed),
        Kind::Ellipse(el) => lines_from_pts(&ellipse_points(el, 96), ellipse_is_full(el)),
        Kind::Spline { fit, closed, nurbs } => lines_from_pts(&spline_curve(fit, *closed, nurbs.as_deref(), 14), false),
        Kind::Point { .. } | Kind::Text(_) | Kind::Hatch(_) => vec![],
        Kind::Dim(d) => dim_geometry(d, &doc.settings).segs,
        Kind::Leader { pts, .. } => lines_from_pts(pts, false),
        Kind::Insert(ins) => {
            let def = match doc.blocks.get(&ins.name) {
                Some(d) if depth <= MAX_BLOCK_DEPTH => d,
                _ => return vec![],
            };
            let m = insert_matrix(ins, def.base);
            def.entities.iter().flat_map(|be| transform_segs(&segs_of_depth(be, doc, depth + 1), &m)).collect()
        }
        Kind::Xline { p, d } => vec![Seg::line(*p + *d * -1e6, *p + *d * 1e6)],
        Kind::Ray { p, d } => vec![Seg::line(*p, *p + *d * 1e6)],
        Kind::Viewport(v) => {
            let (c, w, h) = (v.c, v.w, v.h);
            vec![
                Seg::line(pt(c.x - w / 2.0, c.y - h / 2.0), pt(c.x + w / 2.0, c.y - h / 2.0)),
                Seg::line(pt(c.x + w / 2.0, c.y - h / 2.0), pt(c.x + w / 2.0, c.y + h / 2.0)),
                Seg::line(pt(c.x + w / 2.0, c.y + h / 2.0), pt(c.x - w / 2.0, c.y + h / 2.0)),
                Seg::line(pt(c.x - w / 2.0, c.y + h / 2.0), pt(c.x - w / 2.0, c.y - h / 2.0)),
            ]
        }
        Kind::Table(t) => table_geometry(t).0,
        Kind::Wipeout(w) => {
            let n = w.pts.len();
            (0..n).filter(|_| n >= 2).map(|i| Seg::line(w.pts[i], w.pts[(i + 1) % n])).collect()
        }
        Kind::Region(r) => r.loops.iter().flat_map(|l| pline_segs(l, true)).collect(),
        Kind::Image(im) => {
            let b = im.boundary();
            let n = b.len();
            (0..n).map(|i| Seg::line(b[i], b[(i + 1) % n])).collect()
        }
    }
}

/// Grid lines and cell texts of a table (rotation about the insertion point applied).
pub fn table_geometry(t: &TableData) -> (Vec<Seg>, Vec<TextData>) {
    let mut xs = vec![0.0];
    for w in &t.cols {
        xs.push(xs.last().copied().unwrap_or(0.0) + w);
    }
    let mut ys = vec![0.0];
    for h in &t.rows {
        ys.push(ys.last().copied().unwrap_or(0.0) + h);
    }
    let (nr, nc) = (t.rows.len(), t.cols.len());
    let (cs, sn) = (t.rot.cos(), t.rot.sin());
    let w = |x: f64, y: f64| {
        let (dx, dy) = (x, -y);
        pt(t.p.x + dx * cs - dy * sn, t.p.y + dx * sn + dy * cs)
    };
    let hidden_h = |i: usize, c: usize| t.merges.iter().any(|m| m[0] < i && i <= m[2] && m[1] <= c && c <= m[3]);
    let hidden_v = |j: usize, r: usize| t.merges.iter().any(|m| m[1] < j && j <= m[3] && m[0] <= r && r <= m[2]);
    let mut segs = vec![];
    for i in 0..=nr {
        let mut start: Option<usize> = None;
        for c in 0..=nc {
            let vis = c < nc && !hidden_h(i, c);
            match (vis, start) {
                (true, None) => start = Some(c),
                (false, Some(s)) => {
                    segs.push(Seg::line(w(xs[s], ys[i]), w(xs[c], ys[i])));
                    start = None;
                }
                _ => {}
            }
        }
    }
    for j in 0..=nc {
        let mut start: Option<usize> = None;
        for r in 0..=nr {
            let vis = r < nr && !hidden_v(j, r);
            match (vis, start) {
                (true, None) => start = Some(r),
                (false, Some(s)) => {
                    segs.push(Seg::line(w(xs[j], ys[s]), w(xs[j], ys[r])));
                    start = None;
                }
                _ => {}
            }
        }
    }
    let mut texts = vec![];
    // formulas show their values
    let shown = crate::table_calc::display(t);
    for r in 0..nr {
        for c in 0..nc {
            let s = shown.get(r).and_then(|row| row.get(c)).map(String::as_str).unwrap_or("");
            if s.is_empty() {
                continue;
            }
            if t.merges.iter().any(|m| (m[0] != r || m[1] != c) && m[0] <= r && r <= m[2] && m[1] <= c && c <= m[3]) {
                continue;
            }
            let (r1, c1) = t.merges.iter().find(|m| m[0] == r && m[1] == c).map(|m| (m[2], m[3])).unwrap_or((r, c));
            let ctr = w((xs[c] + xs[c1 + 1]) / 2.0, (ys[r] + ys[r1 + 1]) / 2.0);
            let h = if t.title && r == 0 { t.h * 1.25 } else { t.h };
            let mut td = TextData::new(ctr, h, s).with_just(HJust::Center, VJust::Middle);
            td.rot = t.rot;
            texts.push(td);
        }
    }
    (segs, texts)
}

/// Text-like entities visible inside a block reference, dimension or leader (for bounds / rendering).
pub fn texts_of(e: &Entity, doc: &Drawing) -> Vec<TextData> {
    texts_of_depth(e, doc, 0)
}

fn texts_of_depth(e: &Entity, doc: &Drawing, depth: usize) -> Vec<TextData> {
    match &e.kind {
        Kind::Text(t) => vec![t.clone()],
        Kind::Dim(d) => vec![dim_geometry(d, &doc.settings).text],
        Kind::Leader { pts, s, h } if !s.is_empty() => vec![leader_text(pts, s, *h)],
        Kind::Table(t) => table_geometry(t).1,
        Kind::Insert(ins) => {
            let def = match doc.blocks.get(&ins.name) {
                Some(d) if depth <= MAX_BLOCK_DEPTH => d,
                _ => return vec![],
            };
            let m = insert_matrix(ins, def.base);
            let (k, ang) = (mat::scale_factor(&m), mat::angle(&m));
            // attribute templates are not shown inside the reference: its ATTRIB values replace them
            let mut out: Vec<TextData> = def
                .entities
                .iter()
                .filter(|be| !matches!(&be.kind, Kind::Text(t) if t.att.is_some()))
                .flat_map(|be| texts_of_depth(be, doc, depth + 1))
                .map(|mut t| {
                    t.p = mat::apply(&m, t.p);
                    t.h *= k;
                    t.rot += ang;
                    t.w = t.w.map(|w| w * k);
                    t
                })
                .collect();
            out.extend(ins.attribs.iter().filter(|a| !a.hidden && !a.t.s.is_empty()).map(|a| a.t.clone()));
            out
        }
        _ => vec![],
    }
}

pub fn leader_text(pts: &[Pt], s: &str, h: f64) -> TextData {
    let last = *pts.last().unwrap_or(&Pt::ZERO);
    let prev = if pts.len() >= 2 { pts[pts.len() - 2] } else { last };
    let right = last.x >= prev.x;
    TextData::new(pt(last.x + if right { 1.0 } else { -1.0 } * h * 0.4, last.y), h, s).with_just(if right { HJust::Left } else { HJust::Right }, VJust::Middle)
}

// ---------------------------------------------------------------- bounds

pub fn bounds_of(e: &Entity, doc: &Drawing) -> Option<Bounds> {
    if e.id != 0 {
        if let Some(b) = doc.cache_get_bounds(e.id) {
            return b;
        }
    }
    let b = compute_bounds(e, doc);
    if e.id != 0 {
        doc.cache_put_bounds(e.id, b);
    }
    b
}

fn compute_bounds(e: &Entity, doc: &Drawing) -> Option<Bounds> {
    if matches!(e.kind, Kind::Xline { .. } | Kind::Ray { .. }) {
        return None;
    }
    if let Kind::Point { p } = &e.kind {
        return Some(Bounds::new(p.x, p.y, p.x, p.y));
    }
    let mut b: Option<Bounds> = None;
    for s in segs_of(e, doc).iter() {
        b = Bounds::union_opt(b, Some(s.bounds()));
    }
    if let Kind::Hatch(h) = &e.kind {
        for l in &h.loops {
            for s in pline_segs(l, true) {
                b = Bounds::union_opt(b, Some(s.bounds()));
            }
        }
    }
    for t in texts_of(e, doc) {
        b = Bounds::union_opt(b, Some(Bounds::of_points(&text_corners(&t))));
    }
    if let Kind::Dim(d) = &e.kind {
        for a in dim_geometry(d, &doc.settings).arrows {
            b = Bounds::union_opt(b, Some(Bounds::of_points(&[a.tip])));
        }
    }
    if let Kind::Pline { v, closed } = &e.kind {
        for poly in pline_outline(v, *closed) {
            b = Bounds::union_opt(b, Some(Bounds::of_points(&poly)));
        }
    }
    b
}

// ---------------------------------------------------------------- hit testing helpers

pub fn text_polygons(e: &Entity, doc: &Drawing) -> Vec<[Pt; 4]> {
    texts_of(e, doc).iter().map(text_corners).collect()
}

/// Distance from p to the pickable geometry of e (0 inside text boxes / filled hatches).
pub fn pick_distance(e: &Entity, p: Pt, doc: &Drawing) -> f64 {
    if let Kind::Point { p: q } = &e.kind {
        return dist(*q, p);
    }
    let mut best = f64::INFINITY;
    for s in segs_of(e, doc).iter() {
        best = best.min(s.nearest(p).2);
    }
    for poly in text_polygons(e, doc) {
        if point_in_polygon(p, &poly) {
            return 0.0;
        }
        for i in 0..4 {
            best = best.min(Seg::line(poly[i], poly[(i + 1) % 4]).nearest(p).2);
        }
    }
    if let Kind::Hatch(h) = &e.kind {
        best = best.min(hatch_pick_distance(e, h, p, doc));
    }
    if let Kind::Image(im) = &e.kind {
        // a picture is picked anywhere on its visible area, not only on the frame
        if point_in_polygon(p, &im.boundary()) {
            return 0.0;
        }
    }
    if let Kind::Pline { v, closed } = &e.kind {
        // a wide polyline is picked anywhere on its drawn area
        if pline_outline(v, *closed).iter().any(|poly| point_in_polygon(p, poly)) {
            return 0.0;
        }
    }
    best
}

fn hatch_pick_distance(e: &Entity, h: &HatchData, p: Pt, doc: &Drawing) -> f64 {
    if h.pattern == "SOLID" {
        return if point_in_hatch(h, p) { 0.0 } else { f64::INFINITY };
    }
    let mut best = f64::INFINITY;
    if let Some(lines) = hatch_segments(e, doc) {
        for l in lines.iter() {
            best = best.min(Seg::line(pt(l[0], l[1]), pt(l[2], l[3])).nearest(p).2);
        }
    }
    best
}

pub fn point_in_hatch(h: &HatchData, p: Pt) -> bool {
    let mut inside = false;
    for l in &h.loops {
        if point_in_polygon(p, &tess_segs(&pline_segs(l, true), 0.05)) {
            inside = !inside;
        }
    }
    inside
}

/// Hatch pattern lines (cached per document revision); `None` when the pattern would be too dense.
pub fn hatch_segments(e: &Entity, doc: &Drawing) -> Option<Rc<Vec<[f64; 4]>>> {
    let h = match &e.kind {
        Kind::Hatch(h) => h,
        _ => return Some(Rc::new(vec![])),
    };
    if e.id != 0 {
        if let Some(v) = doc.cache_get_hatch(e.id) {
            return v;
        }
    }
    let diag = bounds_of(e, doc).map(|b| (b.max_x - b.min_x).hypot(b.max_y - b.min_y)).unwrap_or(1.0);
    let tol = 0.01f64.max(0.002 * diag);
    let loops: Vec<Vec<Pt>> = h.loops.iter().map(|l| tess_segs(&pline_segs(l, true), tol)).collect();
    let r = hatch_lines(&loops, &h.pattern, if h.ps == 0.0 { 1.0 } else { h.ps }, deg(h.pa)).map(Rc::new);
    if e.id != 0 {
        doc.cache_put_hatch(e.id, r.clone());
    }
    r
}

pub fn hatch_loops_tess(h: &HatchData) -> Vec<Vec<Pt>> {
    h.loops.iter().map(|l| tess_segs(&pline_segs(l, true), 0.02)).collect()
}

// ---------------------------------------------------------------- selection by rectangle

/// Window selection: entity fully inside the rect.
pub fn in_window(e: &Entity, r: &Bounds, doc: &Drawing) -> bool {
    if matches!(e.kind, Kind::Xline { .. } | Kind::Ray { .. }) {
        return false;
    }
    bounds_of(e, doc).map(|b| b.inside(r)).unwrap_or(false)
}

/// Crossing selection: entity touches the rect.
pub fn crosses_rect(e: &Entity, r: &Bounds, doc: &Drawing) -> bool {
    if let Kind::Point { p } = &e.kind {
        return r.contains(*p);
    }
    let b = bounds_of(e, doc);
    let infinite = matches!(e.kind, Kind::Xline { .. } | Kind::Ray { .. });
    if let Some(b) = &b {
        if !b.overlaps(r) && !infinite {
            return false;
        }
    }
    if let Kind::Hatch(h) = &e.kind {
        if h.pattern == "SOLID" && b.map(|b| b.overlaps(r)).unwrap_or(false) && point_in_hatch(h, r.center()) {
            return true;
        }
    }
    let edges: Vec<Seg> = match &e.kind {
        Kind::Hatch(h) => h.loops.iter().flat_map(|l| pline_segs(l, true)).collect(),
        _ => segs_of(e, doc).to_vec(),
    };
    if edges.iter().any(|s| seg_touches_rect(s, r)) {
        return true;
    }
    let corner = pt(r.min_x, r.min_y);
    for poly in text_polygons(e, doc) {
        if poly.iter().any(|q| r.contains(*q)) || point_in_polygon(corner, &poly) {
            return true;
        }
        for i in 0..4 {
            if seg_touches_rect(&Seg::line(poly[i], poly[(i + 1) % 4]), r) {
                return true;
            }
        }
    }
    false
}

// ---------------------------------------------------------------- transforms

pub struct Decomp {
    pub k: f64,
    pub theta: f64,
    pub flip: bool,
}

/// Decompose a similarity matrix into uniform scale, rotation and reflection flag.
pub fn decompose(m: &Mat) -> Decomp {
    Decomp { k: mat::scale_factor(m), theta: mat::angle(m), flip: mat::det(m) < 0.0 }
}

fn flip_verts(v: &mut [Vertex], m: &Mat, flip: bool) {
    let k = mat::scale_factor(m);
    for q in v.iter_mut() {
        let p = mat::apply(m, q.pt());
        q.x = p.x;
        q.y = p.y;
        q.b = if flip { -q.b } else { q.b };
        q.sw *= k;
        q.ew *= k;
    }
}

/// Mutate an entity by an affine similarity transform.
pub fn transform_entity(e: &mut Entity, m: &Mat) {
    let Decomp { k, theta, flip } = decompose(m);
    let mp = |p: Pt| mat::apply(m, p);
    match &mut e.kind {
        Kind::Line { a, b } => {
            *a = mp(*a);
            *b = mp(*b);
        }
        Kind::Circle { c, r } => {
            *c = mp(*c);
            *r *= k;
        }
        Kind::Arc { c, r, a0, a1 } => {
            let sw = sweep_ccw(*a0, *a1);
            let src = Seg::arc(*c, *r, *a0, if sw == 0.0 { TAU } else { sw });
            if let Seg::A { c: nc, r: nr, a0: na0, sw: nsw, .. } = transform_seg(&src, m) {
                let start = if nsw < 0.0 { na0 + nsw } else { na0 };
                let sweep = nsw.abs();
                *c = nc;
                *r = nr;
                *a0 = norm_angle(start);
                *a1 = norm_angle(start + sweep);
            }
        }
        Kind::Pline { v, .. } => flip_verts(v, m, flip),
        Kind::Ellipse(el) => {
            el.c = mp(el.c);
            el.m = mat::apply_vec(m, el.m);
            if flip {
                let (t0, t1) = (-el.t1, -el.t0);
                el.t0 = norm_angle(t0);
                el.t1 = norm_angle(t1);
            }
        }
        Kind::Spline { fit, nurbs, .. } => {
            for p in fit.iter_mut() {
                *p = mp(*p);
            }
            if let Some(n) = nurbs {
                n.map(&mp);
            }
        }
        Kind::Point { p } => *p = mp(*p),
        Kind::Text(t) => {
            t.p = mp(t.p);
            t.h *= k;
            if let Some(w) = t.w.as_mut() {
                if *w != 0.0 {
                    *w *= k;
                }
            }
            if flip {
                let v = mat::apply_vec(m, pt(t.rot.cos(), t.rot.sin()));
                let mut a = v.y.atan2(v.x);
                if a.cos() < -1e-9 || (a.cos().abs() <= 1e-9 && a.sin() < 0.0) {
                    a += PI;
                }
                t.rot = norm_angle(a);
            } else {
                t.rot = norm_angle(t.rot + theta);
            }
        }
        Kind::Dim(d) => {
            for key in DIM_POINT_KEYS {
                if let Some(slot) = d.point_mut(key) {
                    if let Some(p) = slot {
                        *p = mp(*p);
                    }
                }
            }
            if d.kind == DimType::Lin {
                let a = d.ang.unwrap_or(0.0);
                let v = mat::apply_vec(m, pt(a.cos(), a.sin()));
                d.ang = Some(v.y.atan2(v.x));
            }
        }
        Kind::Leader { pts, .. } => {
            for p in pts.iter_mut() {
                *p = mp(*p);
            }
        }
        Kind::Hatch(h) => {
            for l in h.loops.iter_mut() {
                flip_verts(l, m, flip);
            }
            h.pa = if flip { theta - h.pa } else { h.pa + theta };
            h.ps *= k;
        }
        Kind::Insert(i) => {
            i.p = mp(i.p);
            for q in i.clip.iter_mut() {
                *q = mp(*q);
            }
            if flip {
                i.rot = norm_angle(theta - i.rot);
                i.sx *= k;
                i.sy = -i.sy * k;
            } else {
                i.rot = norm_angle(i.rot + theta);
                i.sx *= k;
                i.sy *= k;
            }
            for a in i.attribs.iter_mut() {
                transform_text(&mut a.t, m);
            }
        }
        Kind::Xline { p, d } | Kind::Ray { p, d } => {
            *p = mp(*p);
            *d = unit(mat::apply_vec(m, *d));
        }
        Kind::Viewport(v) => v.c = mp(v.c),
        Kind::Table(t) => {
            t.p = mp(t.p);
            t.rot = norm_angle(t.rot + theta);
            for c in t.cols.iter_mut().chain(t.rows.iter_mut()) {
                *c *= k;
            }
            t.h *= k;
        }
        Kind::Wipeout(w) => {
            for q in w.pts.iter_mut() {
                *q = mp(*q);
            }
        }
        Kind::Region(r) => {
            for l in r.loops.iter_mut() {
                flip_verts(l, m, flip);
            }
        }
        Kind::Image(im) => {
            im.p = mp(im.p);
            im.u = mat::apply_vec(m, im.u);
            im.v = mat::apply_vec(m, im.v);
            for q in im.clip.iter_mut() {
                *q = mp(*q);
            }
        }
    }
}

fn transform_text(t: &mut TextData, m: &Mat) {
    let Decomp { k, theta, flip } = decompose(m);
    t.p = mat::apply(m, t.p);
    t.h *= k;
    if let Some(w) = t.w.as_mut() {
        if *w != 0.0 {
            *w *= k;
        }
    }
    if flip {
        let v = mat::apply_vec(m, pt(t.rot.cos(), t.rot.sin()));
        let mut a = v.y.atan2(v.x);
        if a.cos() < -1e-9 || (a.cos().abs() <= 1e-9 && a.sin() < 0.0) {
            a += PI;
        }
        t.rot = norm_angle(a);
    } else {
        t.rot = norm_angle(t.rot + theta);
    }
}

// ---------------------------------------------------------------- stretch (STRETCH command)

/// Move the parts of an entity that lie inside `win` by `d`. Returns whether anything changed.
pub fn stretch_entity(e: &mut Entity, win: &Bounds, d: Pt, doc: &Drawing) -> bool {
    let inside = |p: Pt| win.contains(p);
    let mv = |p: &mut Pt| {
        p.x += d.x;
        p.y += d.y;
    };
    let fully = |e: &Entity| bounds_of(e, doc).map(|b| b.inside(win)).unwrap_or(false);
    let snapshot = e.clone();
    match &mut e.kind {
        Kind::Line { a, b } => {
            let mut c = false;
            if inside(*a) {
                mv(a);
                c = true;
            }
            if inside(*b) {
                mv(b);
                c = true;
            }
            c
        }
        Kind::Pline { v, .. } => {
            let mut c = false;
            for q in v.iter_mut() {
                if inside(q.pt()) {
                    q.x += d.x;
                    q.y += d.y;
                    c = true;
                }
            }
            c
        }
        Kind::Spline { nurbs: Some(n), fit, .. } => {
            let mut c = false;
            for q in n.cv.iter_mut().chain(fit.iter_mut()) {
                if inside(*q) {
                    mv(q);
                    c = true;
                }
            }
            c
        }
        Kind::Spline { fit: pts, .. } | Kind::Leader { pts, .. } => {
            let mut c = false;
            for q in pts.iter_mut() {
                if inside(*q) {
                    mv(q);
                    c = true;
                }
            }
            c
        }
        Kind::Arc { c, r, a0, a1 } => {
            let sw = sweep_ccw(*a0, *a1);
            let s = polar(*c, *a0, *r);
            let en = polar(*c, *a0 + sw, *r);
            let (si, ei) = (inside(s), inside(en));
            if si && ei {
                mv(c);
                return true;
            }
            if !si && !ei {
                return false;
            }
            let sw = if sw == 0.0 { TAU } else { sw };
            let mid_pt = polar(*c, *a0 + sw / 2.0, *r);
            let ns = if si { s + d } else { s };
            let ne = if ei { en + d } else { en };
            match arc_through(ns, mid_pt + d * 0.5, ne) {
                Some(Seg::A { c: nc, r: nr, a0: na0, sw: nsw, .. }) => {
                    *c = nc;
                    *r = nr;
                    *a0 = norm_angle(if nsw >= 0.0 { na0 } else { na0 + nsw });
                    *a1 = norm_angle(if nsw >= 0.0 { na0 + nsw } else { na0 });
                    true
                }
                _ => false,
            }
        }
        Kind::Circle { c, .. } => {
            if inside(*c) {
                mv(c);
                true
            } else {
                false
            }
        }
        Kind::Ellipse(el) => {
            if inside(el.c) {
                mv(&mut el.c);
                true
            } else {
                false
            }
        }
        Kind::Point { p } | Kind::Text(TextData { p, .. }) | Kind::Xline { p, .. } | Kind::Ray { p, .. } => {
            if inside(*p) {
                mv(p);
                true
            } else {
                false
            }
        }
        Kind::Insert(i) => {
            if inside(i.p) {
                mv(&mut i.p);
                for q in i.clip.iter_mut() {
                    mv(q);
                }
                true
            } else {
                false
            }
        }
        Kind::Dim(dd) => {
            let mut c = false;
            for key in DIM_POINT_KEYS {
                if let Some(slot) = dd.point_mut(key) {
                    if let Some(p) = slot {
                        if inside(*p) {
                            mv(p);
                            c = true;
                        }
                    }
                }
            }
            c
        }
        Kind::Hatch(h) => {
            if fully(&snapshot) {
                for l in h.loops.iter_mut() {
                    for v in l.iter_mut() {
                        v.x += d.x;
                        v.y += d.y;
                    }
                }
                true
            } else {
                false
            }
        }
        Kind::Viewport(_) => false,
        Kind::Table(t) => {
            if inside(t.p) {
                mv(&mut t.p);
                true
            } else {
                false
            }
        }
        Kind::Wipeout(w) => {
            let mut c = false;
            for q in w.pts.iter_mut() {
                if inside(*q) {
                    mv(q);
                    c = true;
                }
            }
            c
        }
        Kind::Image(im) => {
            if inside(im.p) {
                mv(&mut im.p);
                for q in im.clip.iter_mut() {
                    mv(q);
                }
                true
            } else {
                false
            }
        }
        Kind::Region(r) => {
            if fully(&snapshot) {
                for l in r.loops.iter_mut() {
                    for v in l.iter_mut() {
                        v.x += d.x;
                        v.y += d.y;
                    }
                }
                true
            } else {
                false
            }
        }
    }
}

// ---------------------------------------------------------------- grips

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GripKind {
    Stretch,
    Move,
    Radius,
}

#[derive(Clone, Debug)]
pub struct Grip {
    pub id: String,
    pub pt: Pt,
    pub kind: GripKind,
}

fn grip(id: &str, pt: Pt, kind: GripKind) -> Grip {
    Grip { id: id.to_string(), pt, kind }
}

/// Grip descriptors for an entity: stretch points, move handles and radius handles.
pub fn grips_of(e: &Entity, doc: &Drawing) -> Vec<Grip> {
    use GripKind::*;
    match &e.kind {
        Kind::Line { a, b } => vec![grip("a", *a, Stretch), grip("m", mid(*a, *b), Move), grip("b", *b, Stretch)],
        Kind::Circle { c, r } => {
            let mut v = vec![grip("c", *c, Move)];
            for i in 0..4 {
                v.push(grip(&format!("q{}", i), polar(*c, i as f64 * PI / 2.0, *r), Radius));
            }
            v
        }
        Kind::Arc { c, r, a0, a1 } => {
            let sw = { let s = sweep_ccw(*a0, *a1); if s == 0.0 { TAU } else { s } };
            vec![grip("s", polar(*c, *a0, *r), Stretch), grip("m", polar(*c, *a0 + sw / 2.0, *r), Stretch), grip("e", polar(*c, *a0 + sw, *r), Stretch), grip("c", *c, Move)]
        }
        Kind::Pline { v, closed } => {
            let mut out: Vec<Grip> = v.iter().enumerate().map(|(i, q)| grip(&format!("v{}", i), q.pt(), Stretch)).collect();
            for (i, s) in pline_segs(v, *closed).iter().enumerate() {
                out.push(grip(&format!("m{}", i), s.at(0.5), if s.is_line() { Move } else { Stretch }));
            }
            out
        }
        Kind::Ellipse(el) => {
            let n = pt(-el.m.y * el.ratio, el.m.x * el.ratio);
            vec![grip("c", el.c, Move), grip("a0", el.c + el.m, Stretch), grip("a1", el.c - el.m, Stretch), grip("b0", el.c + n, Stretch), grip("b1", el.c - n, Stretch)]
        }
        // a spline made from control vertices shows them; one with fit points shows those
        Kind::Spline { fit, nurbs: Some(n), .. } if fit.is_empty() => n.cv.iter().enumerate().map(|(i, p)| grip(&format!("v{}", i), *p, Stretch)).collect(),
        Kind::Spline { fit, .. } => fit.iter().enumerate().map(|(i, p)| grip(&format!("f{}", i), *p, Stretch)).collect(),
        Kind::Point { p } | Kind::Xline { p, .. } | Kind::Ray { p, .. } => vec![grip("p", *p, Move)],
        Kind::Text(t) => vec![grip("p", t.p, Move)],
        Kind::Insert(i) => vec![grip("p", i.p, Move)],
        Kind::Leader { pts, .. } => pts.iter().enumerate().map(|(i, p)| grip(&format!("l{}", i), *p, Stretch)).collect(),
        Kind::Hatch(_) => bounds_of(e, doc).map(|b| vec![grip("c", b.center(), Move)]).unwrap_or_default(),
        Kind::Dim(d) => {
            let mut out = vec![];
            let geo = dim_geometry(d, &doc.settings);
            let mut add = |id: &str, p: Option<Pt>| {
                if let Some(p) = p {
                    out.push(grip(id, p, Stretch));
                }
            };
            match d.kind {
                DimType::Lin | DimType::Ali => {
                    add("p1", d.p1);
                    add("p2", d.p2);
                    add("dp", geo.dim_line.as_ref().map(|l| mid(l.a, l.b)).or(d.dp));
                }
                DimType::Ang => {
                    add("p1", d.p1);
                    add("p2", d.p2);
                    add("dp", d.dp);
                }
                _ => {
                    add("pt", d.pt);
                    add("dp", d.dp);
                }
            }
            add("tp", Some(geo.text.p));
            out
        }
        Kind::Table(t) => vec![grip("p", t.p, Move)],
        Kind::Wipeout(w) => {
            let mut out: Vec<Grip> = w.pts.iter().enumerate().map(|(i, q)| grip(&format!("v{}", i), *q, Stretch)).collect();
            if let Some(b) = bounds_of(e, doc) {
                out.push(grip("c", b.center(), Move));
            }
            out
        }
        Kind::Region(_) => bounds_of(e, doc).map(|b| vec![grip("c", b.center(), Move)]).unwrap_or_default(),
        Kind::Image(im) => {
            let cs = im.corners();
            let mut out = vec![grip("c", pt((cs[0].x + cs[2].x) / 2.0, (cs[0].y + cs[2].y) / 2.0), Move)];
            out.extend(cs.iter().enumerate().map(|(i, q)| grip(&format!("k{i}"), *q, Stretch)));
            out
        }
        Kind::Viewport(v) => {
            let mut out = vec![grip("c", v.c, Move)];
            for (i, (sx, sy)) in [(-1.0, -1.0), (1.0, -1.0), (1.0, 1.0), (-1.0, 1.0)].iter().enumerate() {
                out.push(grip(&format!("k{}", i), pt(v.c.x + sx * v.w / 2.0, v.c.y + sy * v.h / 2.0), Stretch));
            }
            out
        }
    }
}

/// Modified clone of `orig` with grip `id` dragged to `to`.
pub fn grip_edit(orig: &Entity, id: &str, to: Pt, doc: &Drawing) -> Entity {
    let mut e = orig.clone();
    let g = match grips_of(orig, doc).into_iter().find(|q| q.id == id) {
        Some(g) => g,
        None => return e,
    };
    let delta = to - g.pt;
    if g.kind == GripKind::Move {
        transform_entity(&mut e, &mat::translate(delta.x, delta.y));
        return e;
    }
    let idx = |s: &str| s[1..].parse::<usize>().unwrap_or(0);
    match (&mut e.kind, &orig.kind) {
        (Kind::Line { a, b }, _) => {
            if id == "a" {
                *a = to;
            } else {
                *b = to;
            }
        }
        (Kind::Circle { c, r }, _) => *r = dist(*c, to).max(1e-9),
        (Kind::Arc { c, r, a0, a1 }, Kind::Arc { c: oc, r: or, a0: oa0, a1: oa1 }) => {
            let sw = { let s = sweep_ccw(*oa0, *oa1); if s == 0.0 { TAU } else { s } };
            let s = polar(*oc, *oa0, *or);
            let m = polar(*oc, *oa0 + sw / 2.0, *or);
            let en = polar(*oc, *oa0 + sw, *or);
            let tri = match id {
                "s" => [to, m, en],
                "e" => [s, m, to],
                _ => [s, to, en],
            };
            if let Some(Seg::A { c: nc, r: nr, a0: na0, sw: nsw, .. }) = arc_through(tri[0], tri[1], tri[2]) {
                *c = nc;
                *r = nr;
                *a0 = norm_angle(if nsw >= 0.0 { na0 } else { na0 + nsw });
                *a1 = norm_angle(if nsw >= 0.0 { na0 + nsw } else { na0 });
            }
        }
        (Kind::Pline { v, .. }, Kind::Pline { v: ov, closed }) => {
            if id.starts_with('v') {
                let i = idx(id);
                v[i].x = to.x;
                v[i].y = to.y;
            } else {
                let i = idx(id);
                let n = v.len();
                let j = (i + 1) % n;
                let segs = pline_segs(ov, *closed);
                if segs[i].is_line() {
                    v[i].x += delta.x;
                    v[i].y += delta.y;
                    v[j].x += delta.x;
                    v[j].y += delta.y;
                } else if let Some(Seg::A { sw, .. }) = arc_through(ov[i].pt(), to, ov[j].pt()) {
                    v[i].b = (sw / 4.0).tan();
                }
            }
        }
        (Kind::Ellipse(el), Kind::Ellipse(oe)) => {
            if id == "a0" || id == "a1" {
                let sign = if id == "a0" { 1.0 } else { -1.0 };
                let m = (to - el.c) * sign;
                let (l, l0) = (len(m), len(oe.m));
                if l > 1e-9 {
                    el.m = m;
                    el.ratio = oe.ratio * l0 / l;
                }
            } else {
                let n = unit(pt(-oe.m.y, oe.m.x));
                el.ratio = (dot(to - el.c, n).abs() / len(oe.m)).clamp(1e-6, 1.0);
            }
        }
        (Kind::Spline { nurbs: Some(n), .. }, _) if id.starts_with('v') => {
            if let Some(q) = n.cv.get_mut(idx(id)) {
                *q = to;
            }
        }
        (Kind::Spline { fit, nurbs, .. }, _) => {
            // moving a fit point refits the curve through the fit points
            if let Some(q) = fit.get_mut(idx(id)) {
                *q = to;
                *nurbs = None;
            }
        }
        (Kind::Wipeout(w), _) => {
            if let Some(q) = w.pts.get_mut(idx(id)) {
                *q = to;
            }
        }
        (Kind::Image(im), Kind::Image(o)) => {
            // a corner grip resizes the picture about the opposite corner, keeping its proportions
            let cs = o.corners();
            let i = idx(id) % 4;
            let (anchor, moved) = (cs[(i + 2) % 4], cs[i]);
            let diag = moved - anchor;
            let k = (dot(to - anchor, diag) / dot(diag, diag).max(1e-18)).max(0.001);
            im.u = o.u * k;
            im.v = o.v * k;
            im.p = anchor + (o.p - anchor) * k;
            for (q, oq) in im.clip.iter_mut().zip(&o.clip) {
                *q = anchor + (*oq - anchor) * k;
            }
        }
        (Kind::Leader { pts, .. }, _) => pts[idx(id)] = to,
        (Kind::Dim(d), _) => {
            if id == "tp" {
                d.tp = Some(to);
            } else if let Some(slot) = d.point_mut(id) {
                *slot = Some(to);
            }
        }
        (Kind::Viewport(vp), Kind::Viewport(o)) => {
            let i = idx(id);
            let (sx, sy) = ([-1.0, 1.0, 1.0, -1.0][i], [-1.0, -1.0, 1.0, 1.0][i]);
            let opp = pt(o.c.x - sx * o.w / 2.0, o.c.y - sy * o.h / 2.0);
            vp.c = mid(opp, to);
            vp.w = (to.x - opp.x).abs();
            vp.h = (to.y - opp.y).abs();
        }
        _ => {}
    }
    e
}

// ---------------------------------------------------------------- object snap points

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum SnapType {
    End,
    Mid,
    Cen,
    Qua,
    Nod,
    Ins,
}

#[derive(Clone, Copy, Debug)]
pub struct SnapPt {
    pub pt: Pt,
    pub kind: SnapType,
}

/// Snap candidates intrinsic to an entity: END, MID, CEN, QUA, NOD, INS.
pub fn snap_points_of(e: &Entity, doc: &Drawing) -> Rc<Vec<SnapPt>> {
    if e.id != 0 {
        if let Some(v) = doc.cache_get_snaps(e.id) {
            return v;
        }
    }
    let v = Rc::new(compute_snaps(e, doc));
    if e.id != 0 {
        doc.cache_put_snaps(e.id, v.clone());
    }
    v
}

fn compute_snaps(e: &Entity, doc: &Drawing) -> Vec<SnapPt> {
    let mut out: Vec<SnapPt> = vec![];
    let mut push = |pt: Pt, kind: SnapType| out.push(SnapPt { pt, kind });
    match &e.kind {
        Kind::Line { a, b } => {
            push(*a, SnapType::End);
            push(*b, SnapType::End);
            push(mid(*a, *b), SnapType::Mid);
            out
        }
        Kind::Circle { c, r } => {
            push(*c, SnapType::Cen);
            for i in 0..4 {
                push(polar(*c, i as f64 * PI / 2.0, *r), SnapType::Qua);
            }
            out
        }
        Kind::Ellipse(el) => {
            push(el.c, SnapType::Cen);
            let n = pt(-el.m.y * el.ratio, el.m.x * el.ratio);
            if ellipse_is_full(el) {
                for q in [el.c + el.m, el.c - el.m, el.c + n, el.c - n] {
                    push(q, SnapType::Qua);
                }
            } else {
                push(ellipse_point(el, el.t0), SnapType::End);
                push(ellipse_point(el, el.t1), SnapType::End);
            }
            out
        }
        Kind::Spline { fit, nurbs: Some(n), .. } if fit.is_empty() => {
            let (t0, t1) = n.domain();
            push(n.at(t0), SnapType::End);
            push(n.at(t1), SnapType::End);
            out
        }
        Kind::Spline { fit, .. } => {
            if let (Some(f), Some(l)) = (fit.first(), fit.last()) {
                push(*f, SnapType::End);
                push(*l, SnapType::End);
                for p in fit.iter().skip(1).take(fit.len().saturating_sub(2)) {
                    push(*p, SnapType::Nod);
                }
            }
            out
        }
        Kind::Point { p } => {
            push(*p, SnapType::Nod);
            out
        }
        Kind::Text(t) => {
            push(t.p, SnapType::Ins);
            out
        }
        Kind::Insert(i) => {
            push(i.p, SnapType::Ins);
            for s in segs_of(e, doc).iter() {
                seg_snaps(s, &mut push);
            }
            out
        }
        Kind::Dim(d) => {
            let geo = dim_geometry(d, &doc.settings);
            for s in &geo.segs {
                if let Seg::L { a, b, .. } = s {
                    push(*a, SnapType::End);
                    push(*b, SnapType::End);
                }
            }
            push(geo.text.p, SnapType::Ins);
            out
        }
        Kind::Leader { pts, .. } => {
            for p in pts {
                push(*p, SnapType::End);
            }
            out
        }
        Kind::Hatch(_) => out,
        Kind::Xline { p, .. } | Kind::Ray { p, .. } => {
            push(*p, SnapType::End);
            out
        }
        _ => {
            for s in segs_of(e, doc).iter() {
                seg_snaps(s, &mut push);
            }
            dedupe_snaps(out)
        }
    }
}

fn seg_snaps(s: &Seg, push: &mut impl FnMut(Pt, SnapType)) {
    match *s {
        Seg::L { a, b, approx } => {
            if approx {
                return;
            }
            push(a, SnapType::End);
            push(b, SnapType::End);
            push(mid(a, b), SnapType::Mid);
        }
        Seg::A { c, r, a0, sw, full } => {
            if full {
                push(c, SnapType::Cen);
                for i in 0..4 {
                    push(polar(c, i as f64 * PI / 2.0, r), SnapType::Qua);
                }
                return;
            }
            push(s.start(), SnapType::End);
            push(s.end(), SnapType::End);
            push(s.at(0.5), SnapType::Mid);
            push(c, SnapType::Cen);
            for i in 0..4 {
                if angle_in_arc(i as f64 * PI / 2.0, a0, sw, 1e-9) {
                    push(polar(c, i as f64 * PI / 2.0, r), SnapType::Qua);
                }
            }
        }
    }
}

fn dedupe_snaps(list: Vec<SnapPt>) -> Vec<SnapPt> {
    let mut out: Vec<SnapPt> = vec![];
    for s in list {
        if !out.iter().any(|o| o.kind == s.kind && same_pt(o.pt, s.pt, 1e-9)) {
            out.push(s);
        }
    }
    out
}

// ---------------------------------------------------------------- explode

fn inherit(src: &Entity, mut o: Entity) -> Entity {
    o.layer = src.layer.clone();
    o.color = src.color;
    o.ltype = src.ltype.clone();
    o.lw = src.lw;
    o
}

/// Entities replacing `e` when exploded, or `None` if the entity cannot be exploded.
pub fn explode_entity(e: &Entity, doc: &Drawing) -> Option<Vec<Entity>> {
    match &e.kind {
        Kind::Pline { v, closed } => Some(pline_segs(v, *closed).iter().map(|s| inherit(e, Entity::arc_seg(s))).collect()),
        Kind::Insert(ins) => {
            let def = match doc.blocks.get(&ins.name) {
                Some(d) => d,
                None => return Some(vec![]),
            };
            let m = insert_matrix(ins, def.base);
            let non_uniform = !mat::is_similarity(&m);
            let mut out = vec![];
            for be in &def.entities {
                let mut c = be.cloned_noid();
                if non_uniform && matches!(c.kind, Kind::Circle { .. } | Kind::Arc { .. }) {
                    c = circle_to_ellipse(&c, &m);
                } else {
                    transform_entity(&mut c, &m);
                }
                if c.layer.is_empty() || c.layer == "0" {
                    c.layer = e.layer.clone();
                }
                if c.color == Some(Color::ByBlock) {
                    c.color = e.color;
                }
                out.push(c);
            }
            Some(out)
        }
        Kind::Dim(d) => {
            let geo = dim_geometry(d, &doc.settings);
            let mut out = vec![];
            for s in &geo.segs {
                out.push(inherit(e, Entity::arc_seg(s)));
            }
            for a in &geo.arrows {
                if let Some(poly) = arrow_polygon(a) {
                    out.push(inherit(e, Entity::hatch(vec![poly.iter().map(|p| Vertex::from(*p)).collect()], "SOLID")));
                }
            }
            let t = &geo.text;
            let mut te = Entity::text(t.p, t.h, &t.s);
            if let Kind::Text(td) = &mut te.kind {
                td.rot = t.rot;
                td.jh = t.jh;
                td.jv = t.jv;
            }
            out.push(inherit(e, te));
            Some(out)
        }
        Kind::Leader { pts, s, h } => {
            let mut out = vec![];
            for i in 0..pts.len().saturating_sub(1) {
                out.push(inherit(e, Entity::line(pts[i], pts[i + 1])));
            }
            if !s.is_empty() {
                let t = leader_text(pts, s, *h);
                out.push(inherit(e, Entity::new(Kind::Text(t))));
            }
            Some(out)
        }
        Kind::Text(t) if t.multi => {
            let lines = text_layout(t).lines;
            let m = mat::mul(&mat::translate(t.p.x, t.p.y), &mat::rotate(t.rot, Pt::ZERO));
            Some(
                lines
                    .into_iter()
                    .map(|l| {
                        let mut te = Entity::text(mat::apply(&m, pt(l.x, l.y)), t.h, &l.s);
                        if let Kind::Text(td) = &mut te.kind {
                            td.rot = t.rot;
                        }
                        inherit(e, te)
                    })
                    .collect(),
            )
        }
        _ => None,
    }
}

/// Image of a circle/arc entity under an arbitrary affine map: an ellipse (arc) entity. The principal axes come
/// from the singular value decomposition of the map's linear part.
pub fn circle_to_ellipse(c: &Entity, m: &Mat) -> Entity {
    let (cc, r, arc) = match &c.kind {
        Kind::Circle { c, r } => (*c, *r, None),
        Kind::Arc { c, r, a0, a1 } => (*c, *r, Some((*a0, *a1))),
        _ => return c.clone(),
    };
    let (a, b, cm, d) = (m[0], m[1], m[2], m[3]);
    let (e_, f, g_) = (a * a + b * b, a * cm + b * d, cm * cm + d * d);
    let psi = 0.5 * (2.0 * f).atan2(e_ - g_);
    let v1 = pt(psi.cos(), psi.sin());
    let v2 = pt(-v1.y, v1.x);
    let mv = mat::apply_vec(m, v1) * r;
    let nv = mat::apply_vec(m, v2) * r;
    let (l1, l2) = (len(mv), len(nv));
    let flip = mat::det(m) < 0.0;
    let swap_axes = l2 > l1;
    let maj = if swap_axes { nv } else { mv };
    let ratio = l1.min(l2) / l1.max(l2).max(1e-12);
    let shift = if swap_axes { -PI / 2.0 } else { 0.0 };
    let t = |alpha: f64| alpha - psi + shift;
    let (mut t0, mut t1) = (0.0, TAU);
    if let Some((a0, a1)) = arc {
        t0 = t(a0);
        t1 = t(a1);
        if flip {
            let (n0, n1) = (-t1, -t0);
            t0 = n0;
            t1 = n1;
        }
    }
    let mut out = Entity::ellipse(mat::apply(m, cc), maj, ratio, norm_angle(t0), if arc.is_some() { norm_angle(t1) } else { TAU });
    out.layer = c.layer.clone();
    out.color = c.color;
    out.ltype = c.ltype.clone();
    out.lw = c.lw;
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use approx::assert_relative_eq;

    fn doc() -> Drawing {
        Drawing::new("t")
    }

    #[test]
    fn json_round_trip_matches_the_web_format() {
        let src = r##"[
          {"type":"line","a":{"x":0,"y":0},"b":{"x":10,"y":5},"id":3,"layer":"0"},
          {"type":"pline","v":[{"x":0,"y":0,"b":0.5},{"x":4,"y":0,"b":0}],"closed":false,"layer":"Outline","color":"#ff8800","lw":70,"ltype":"DASHED"},
          {"type":"text","p":{"x":1,"y":2},"h":2.5,"s":"Hello","rot":0,"jh":"center","jv":"middle","layer":"0"},
          {"type":"dim","kind":"lin","p1":{"x":0,"y":0},"p2":{"x":10,"y":0},"dp":{"x":5,"y":-3},"ang":0,"layer":"Dimensions"},
          {"type":"hatch","loops":[[{"x":0,"y":0,"b":0},{"x":1,"y":0,"b":0},{"x":1,"y":1,"b":0}]],"pattern":"ANSI31","ps":1.2,"pa":0,"layer":"Hatch"},
          {"type":"insert","name":"BOLT","p":{"x":4,"y":6},"sx":2,"sy":2,"rot":0.5,"layer":"Holes"},
          {"type":"ellipse","c":{"x":0,"y":0},"m":{"x":5,"y":0},"ratio":0.5,"t0":0,"t1":6.283185307179586,"layer":"0"},
          {"type":"leader","pts":[{"x":0,"y":0},{"x":3,"y":4}],"s":"NOTE","h":3.5,"layer":"Notes"},
          {"type":"xline","p":{"x":0,"y":0},"d":{"x":0.7,"y":0.7},"layer":"0"},
          {"type":"spline","fit":[{"x":0,"y":0},{"x":1,"y":2},{"x":3,"y":1}],"closed":false,"layer":"0"}
        ]"##;
        let ents: Vec<Entity> = serde_json::from_str(src).unwrap();
        assert_eq!(ents.len(), 10);
        assert_eq!(ents[1].color, Some(Color::Rgb(255, 136, 0)));
        assert_eq!(ents[3].label(), "Dimension");
        let back = serde_json::to_string(&ents).unwrap();
        let again: Vec<Entity> = serde_json::from_str(&back).unwrap();
        assert_eq!(ents, again);
        assert!(back.contains("\"type\":\"pline\"") && back.contains("\"lw\":70") && !back.contains("null"));
    }

    #[test]
    fn segments_bounds_and_snaps() {
        let d = doc();
        let c = Entity::circle(pt(5.0, 5.0), 3.0);
        let s = segs_of(&c, &d);
        assert!(s[0].is_full());
        let b = bounds_of(&c, &d).unwrap();
        assert_relative_eq!(b.min_x, 2.0, epsilon = 1e-9);
        assert_relative_eq!(b.max_y, 8.0, epsilon = 1e-9);
        let sn = snap_points_of(&c, &d);
        assert_eq!(sn.iter().filter(|p| p.kind == SnapType::Qua).count(), 4);
        assert!(sn.iter().any(|p| p.kind == SnapType::Cen && same(p.pt, pt(5.0, 5.0))));
        let pl = Entity::pline(vec![Vertex::new(0.0, 0.0, 0.0), Vertex::new(10.0, 0.0, 1.0), Vertex::new(10.0, 10.0, 0.0)], false);
        assert_eq!(segs_of(&pl, &d).len(), 2);
        let sn = snap_points_of(&pl, &d);
        assert!(sn.iter().filter(|p| p.kind == SnapType::Mid).count() == 2 && sn.iter().any(|p| p.kind == SnapType::Cen));
        let el = Entity::ellipse(Pt::ZERO, pt(10.0, 0.0), 0.5, 0.0, TAU);
        let b = bounds_of(&el, &d).unwrap();
        assert!((b.max_x - 10.0).abs() < 0.05 && (b.max_y - 5.0).abs() < 0.05);
        assert!(bounds_of(&Entity::xline(Pt::ZERO, pt(1.0, 0.0)), &d).is_none());
    }

    #[test]
    fn pick_window_and_crossing() {
        let d = doc();
        let l = Entity::line(pt(0.0, 0.0), pt(10.0, 0.0));
        assert_relative_eq!(pick_distance(&l, pt(5.0, 2.0), &d), 2.0, epsilon = 1e-12);
        let win = Bounds::new(-1.0, -1.0, 11.0, 1.0);
        assert!(in_window(&l, &win, &d) && crosses_rect(&l, &win, &d));
        let half = Bounds::new(4.0, -1.0, 6.0, 1.0);
        assert!(!in_window(&l, &half, &d) && crosses_rect(&l, &half, &d));
        let t = Entity::text(pt(0.0, 0.0), 2.0, "HELLO");
        assert_eq!(pick_distance(&t, pt(0.5, 0.5), &d), 0.0, "inside the text box");
        let solid = Entity::hatch(vec![vec![Vertex::new(0.0, 0.0, 0.0), Vertex::new(10.0, 0.0, 0.0), Vertex::new(10.0, 10.0, 0.0), Vertex::new(0.0, 10.0, 0.0)]], "SOLID");
        assert_eq!(pick_distance(&solid, pt(5.0, 5.0), &d), 0.0);
        assert!(crosses_rect(&solid, &Bounds::new(4.0, 4.0, 6.0, 6.0), &d), "a crossing window wholly inside a solid hatch selects it");
    }

    #[test]
    fn transforms() {
        let mut l = Entity::line(pt(1.0, 0.0), pt(2.0, 0.0));
        transform_entity(&mut l, &mat::rotate(PI / 2.0, Pt::ZERO));
        if let Kind::Line { a, b } = l.kind {
            assert!(dist(a, pt(0.0, 1.0)) < 1e-12 && dist(b, pt(0.0, 2.0)) < 1e-12);
        }
        let mut a = Entity::arc(Pt::ZERO, 2.0, 0.0, PI / 2.0);
        transform_entity(&mut a, &mat::mirror(Pt::ZERO, pt(1.0, 0.0)));
        if let Kind::Arc { a0, a1, .. } = a.kind {
            assert_relative_eq!(a0, 3.0 * PI / 2.0, epsilon = 1e-12);       // mirrored arcs stay CCW: now from -90° to 0°
            assert!(a1.abs() < 1e-9 || (a1 - TAU).abs() < 1e-9, "end angle wraps to 0, got {a1}");
        }
        let mut p = Entity::pline(vec![Vertex::new(0.0, 0.0, 0.5), Vertex::new(4.0, 0.0, 0.0)], false);
        transform_entity(&mut p, &mat::mirror(Pt::ZERO, pt(1.0, 0.0)));
        if let Kind::Pline { v, .. } = &p.kind {
            assert_relative_eq!(v[0].b, -0.5, epsilon = 1e-12);
        }
        let mut t = Entity::text(pt(1.0, 1.0), 2.0, "x");
        transform_entity(&mut t, &mat::scale(2.0, 2.0, Pt::ZERO));
        if let Kind::Text(td) = &t.kind {
            assert_relative_eq!(td.h, 4.0, epsilon = 1e-12);
            assert!(dist(td.p, pt(2.0, 2.0)) < 1e-12);
        }
    }

    #[test]
    fn stretch_and_grips() {
        let d = doc();
        let win = Bounds::new(9.0, -1.0, 11.0, 1.0);
        let mut l = Entity::line(pt(0.0, 0.0), pt(10.0, 0.0));
        assert!(stretch_entity(&mut l, &win, pt(5.0, 0.0), &d));
        if let Kind::Line { a, b } = l.kind {
            assert!(dist(a, Pt::ZERO) < 1e-12 && dist(b, pt(15.0, 0.0)) < 1e-12);
        }
        let mut far = Entity::line(pt(0.0, 5.0), pt(10.0, 5.0));
        assert!(!stretch_entity(&mut far, &win, pt(5.0, 0.0), &d));
        let c = Entity::circle(pt(5.0, 5.0), 2.0);
        assert_eq!(grips_of(&c, &d).len(), 5);
        let e = grip_edit(&c, "q0", pt(10.0, 5.0), &d);
        if let Kind::Circle { r, .. } = e.kind {
            assert_relative_eq!(r, 5.0, epsilon = 1e-12);
        }
        let l = Entity::line(pt(0.0, 0.0), pt(10.0, 0.0));
        let e = grip_edit(&l, "m", pt(5.0, 3.0), &d);
        if let Kind::Line { a, b } = e.kind {
            assert!(dist(a, pt(0.0, 3.0)) < 1e-12 && dist(b, pt(10.0, 3.0)) < 1e-12);
        }
        let arc = Entity::arc(Pt::ZERO, 5.0, 0.0, PI);
        let e = grip_edit(&arc, "m", pt(0.0, 6.0), &d);
        if let Kind::Arc { r, .. } = e.kind {
            assert!(r > 5.0);
        }
    }

    #[test]
    fn explode_and_circle_to_ellipse() {
        let mut d = doc();
        let pl = Entity::pline(vec![Vertex::new(0.0, 0.0, 0.0), Vertex::new(10.0, 0.0, 1.0), Vertex::new(10.0, 10.0, 0.0)], true).on_layer("A");
        let parts = explode_entity(&pl, &d).unwrap();
        assert_eq!(parts.len(), 3);
        assert!(matches!(parts[1].kind, Kind::Arc { .. }) && parts[1].layer == "A");
        // block insert with non-uniform scale turns circles into ellipses
        d.define_block("B", pt(0.0, 0.0), vec![Entity::circle(pt(0.0, 0.0), 1.0)]);
        let mut ins = Entity::insert("B", pt(10.0, 10.0));
        if let Kind::Insert(i) = &mut ins.kind {
            i.sx = 3.0;
            i.sy = 1.0;
        }
        let parts = explode_entity(&ins, &d).unwrap();
        match &parts[0].kind {
            Kind::Ellipse(el) => {
                assert_relative_eq!(len(el.m), 3.0, epsilon = 1e-9);
                assert_relative_eq!(el.ratio, 1.0 / 3.0, epsilon = 1e-9);
            }
            k => panic!("{:?}", k),
        }
        assert!(explode_entity(&Entity::line(Pt::ZERO, pt(1.0, 1.0)), &d).is_none());
        let mt = Entity::mtext(Pt::ZERO, 2.0, "one\ntwo", 0.0);
        assert_eq!(explode_entity(&mt, &d).unwrap().len(), 2);
    }

    #[test]
    fn spline_bezier_matches_dense_curve() {
        let fit = [pt(0.0, 0.0), pt(10.0, 8.0), pt(25.0, 3.0), pt(40.0, 15.0), pt(50.0, 0.0)];
        for closed in [false, true] {
            let dense = spline_points(&fit, closed, 800);
            let mut worst = 0.0f64;
            for bz in spline_beziers(&fit, closed) {
                for k in 0..=10 {
                    let t = k as f64 / 10.0;
                    let u = 1.0 - t;
                    let p = bz.p0 * (u * u * u) + bz.c1 * (3.0 * u * u * t) + bz.c2 * (3.0 * u * t * t) + bz.p1 * (t * t * t);
                    let best = dense.windows(2).map(|w| Seg::line(w[0], w[1]).nearest(p).2).fold(f64::INFINITY, f64::min);
                    worst = worst.max(best);
                }
            }
            assert!(worst < 1e-3, "{}", worst);
        }
    }
}
