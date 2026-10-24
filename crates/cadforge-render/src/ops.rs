//! Backend-independent draw operations (the Rust counterpart of the recorded canvas operations in the web app).
//! Geometry is flattened to polylines in *device space* (screen pixels for the viewport, 0.1 mm for plots).
use cadforge_core::geom::Mat;
use cadforge_core::picture::Decoded;
use std::rc::Rc;

pub type Rgb = [u8; 3];

#[derive(Clone, Debug, PartialEq)]
pub struct Poly {
    pub pts: Vec<[f64; 2]>,
    pub closed: bool,
}

/// Shared decoded pixels of a picture (compared by identity, not by content).
#[derive(Clone, Debug)]
pub struct Pix(pub Rc<Decoded>);

impl PartialEq for Pix {
    fn eq(&self, o: &Pix) -> bool {
        Rc::ptr_eq(&self.0, &o.0)
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum Op {
    Stroke {
        paths: Vec<Poly>,
        color: Rgb,
        alpha: f32,
        width: f64,
        dash: Vec<f64>,
        round: bool,
    },
    Fill {
        paths: Vec<Poly>,
        color: Rgb,
        alpha: f32,
        evenodd: bool,
    },
    /// A line of text. `m` maps text-local coordinates (x along the baseline, y downwards, units of the em box) to
    /// device space; `(x, y)` is the start of the baseline in local coordinates and `em` the font size in local units.
    Text {
        s: String,
        x: f64,
        y: f64,
        m: Mat,
        em: f64,
        color: Rgb,
        alpha: f32,
    },
    /// A raster picture. `m` maps pixel coordinates (origin at the top-left corner, y down) to device space; `clip` is
    /// the visible boundary in device space (empty = the whole picture); `adj` is brightness / contrast / fade, mixed
    /// toward `bg`; `smooth` selects bilinear sampling over nearest-neighbour.
    Image {
        pix: Pix,
        adj: [i32; 3],
        bg: Rgb,
        m: Mat,
        clip: Vec<[f64; 2]>,
        alpha: f32,
        smooth: bool,
    },
}

impl Op {
    /// Device-space position of the start of a text op's baseline.
    pub fn text_origin(&self) -> Option<[f64; 2]> {
        match self {
            Op::Text { x, y, m, .. } => {
                Some([m[0] * x + m[2] * y + m[4], m[1] * x + m[3] * y + m[5]])
            }
            _ => None,
        }
    }
}

/// Font size in device units and baseline rotation (radians, y-down device space) of a text op.
pub fn text_metrics(m: &Mat, em: f64) -> (f64, f64) {
    ((m[0].hypot(m[1])) * em, m[1].atan2(m[0]))
}

/// Helvetica advance widths (1/1000 em) for printable ASCII; the standard PDF base-14 font, used for layout so that
/// screen, PDF and SVG output agree.
const HELV: [u16; 95] = [
    278, 278, 355, 556, 556, 889, 667, 191, 333, 333, 389, 584, 278, 333, 278, 278, 556, 556, 556,
    556, 556, 556, 556, 556, 556, 556, 278, 278, 584, 584, 584, 556, 1015, 667, 667, 722, 722, 667,
    611, 778, 722, 278, 500, 667, 556, 833, 722, 778, 667, 778, 722, 667, 611, 722, 667, 944, 667,
    667, 611, 278, 278, 278, 469, 556, 333, 556, 556, 500, 556, 556, 278, 556, 556, 222, 222, 500,
    222, 833, 556, 556, 556, 556, 333, 500, 278, 556, 500, 722, 500, 500, 500, 334, 260, 334, 584,
];

/// Width of `s` in drawing units for text of cap height `h` (Helvetica metrics; the cap height is 0.72 em).
pub fn helvetica_width(s: &str, h: f64) -> f64 {
    let em = h / cadforge_core::text::CAP_RATIO;
    s.chars()
        .map(|c| {
            if (' '..='~').contains(&c) {
                HELV[(c as usize) - 32] as f64
            } else {
                556.0
            }
        })
        .sum::<f64>()
        / 1000.0
        * em
}
