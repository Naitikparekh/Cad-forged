//! Persistent defaults between command runs (last radius, polygon sides, hatch settings...).
use cadforge_core::style::Color;

#[derive(Clone, Debug, PartialEq)]
pub struct HatchMemory {
    pub pattern: String,
    pub scale: f64,
    pub angle: f64,
    pub color: Option<Color>,
    pub island: String,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ArrayRect {
    pub rows: i64,
    pub cols: i64,
    pub dr: f64,
    pub dc: f64,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ArrayPolar {
    pub n: i64,
    pub fill: f64,
    pub rotate: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Memory {
    pub circle_radius: f64,
    pub polygon_sides: i64,
    pub polygon_mode: String,
    pub donut_in: f64,
    pub donut_out: f64,
    pub rect_fillet: f64,
    pub rect_chamfer: [f64; 2],
    pub fillet_radius: Option<f64>,
    pub hatch: HatchMemory,
    pub array_rect: ArrayRect,
    pub array_polar: ArrayPolar,
    pub text_height: f64,
    pub dim_style: String,
    pub insert_scale: f64,
    pub insert_rot: f64,
    pub qp_open: bool,
    pub last_line_end: Option<cadforge_core::geom::Pt>,
}

impl Default for Memory {
    fn default() -> Memory {
        Memory {
            circle_radius: 5.0,
            polygon_sides: 4,
            polygon_mode: "Inscribed".into(),
            donut_in: 0.5,
            donut_out: 1.0,
            rect_fillet: 0.0,
            rect_chamfer: [0.0, 0.0],
            fillet_radius: None,
            hatch: HatchMemory { pattern: "ANSI31".into(), scale: 1.0, angle: 0.0, color: None, island: "normal".into() },
            array_rect: ArrayRect { rows: 3, cols: 4, dr: 10.0, dc: 10.0 },
            array_polar: ArrayPolar { n: 6, fill: 360.0, rotate: true },
            text_height: 2.5,
            dim_style: "Standard".into(),
            insert_scale: 1.0,
            insert_rot: 0.0,
            qp_open: false,
            last_line_end: None,
        }
    }
}
