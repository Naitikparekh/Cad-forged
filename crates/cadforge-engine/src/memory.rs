//! Persistent defaults between command runs (last radius, polygon sides, hatch settings...).
use cadforge_core::style::Color;
use std::cell::RefCell;
use std::rc::Rc;

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
pub struct SolidMemory {
    pub radius: f64,
    pub height: f64,
    pub length: f64,
    pub top: f64,
}

#[derive(Clone, Debug)]
pub struct Memory {
    pub circle_radius: f64,
    pub polygon_sides: i64,
    pub polygon_mode: String,
    pub donut_in: f64,
    pub donut_out: f64,
    pub rect_fillet: f64,
    pub rect_chamfer: [f64; 2],
    pub fillet_radius: Option<f64>,
    /// Shared with the UI's hatch editor so edits show up in the live preview.
    pub hatch: Rc<RefCell<HatchMemory>>,
    pub array_rect: ArrayRect,
    pub array_polar: ArrayPolar,
    pub text_height: f64,
    pub dim_style: String,
    pub insert_scale: f64,
    pub insert_rot: f64,
    pub qp_open: bool,
    pub last_line_end: Option<cadforge_core::geom::Pt>,
    /// Entities removed by the last ERASE (for OOPS).
    pub erased: Option<Vec<cadforge_core::entities::Entity>>,
    pub edge_extend: bool,
    pub fillet_trim: bool,
    pub last_dim: Option<u64>,
    pub last_distance: f64,
    pub group_serial: u32,
    pub solid: SolidMemory,
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
            hatch: Rc::new(RefCell::new(HatchMemory {
                pattern: "ANSI31".into(),
                scale: 1.0,
                angle: 0.0,
                color: None,
                island: "normal".into(),
            })),
            array_rect: ArrayRect {
                rows: 3,
                cols: 4,
                dr: 10.0,
                dc: 10.0,
            },
            array_polar: ArrayPolar {
                n: 6,
                fill: 360.0,
                rotate: true,
            },
            text_height: 2.5,
            dim_style: "Standard".into(),
            insert_scale: 1.0,
            insert_rot: 0.0,
            qp_open: false,
            last_line_end: None,
            erased: None,
            edge_extend: false,
            fillet_trim: true,
            last_dim: None,
            last_distance: 0.0,
            group_serial: 0,
            solid: SolidMemory {
                radius: 10.0,
                height: 20.0,
                length: 30.0,
                top: 0.0,
            },
        }
    }
}
