//! Colours for the drawing area and a few rendering preferences.
use crate::ops::Rgb;

#[derive(Clone, Copy, Debug)]
pub struct Theme {
    pub dark: bool,
    /// Darken bright ACI colours on light backgrounds.
    pub contrast: bool,
    pub canvas: Rgb,
    pub grid: Rgb,
    pub grid_alpha: f32,
    pub grid_major: Rgb,
    pub grid_major_alpha: f32,
    pub select: Rgb,
    pub hover: Rgb,
    pub cross: Rgb,
}

impl Theme {
    pub const DARK: Theme = Theme { dark: true, contrast: false, canvas: [0x21, 0x28, 0x30], grid: [255, 255, 255], grid_alpha: 0.07, grid_major: [255, 255, 255], grid_major_alpha: 0.10, select: [0x4a, 0xa8, 0xff], hover: [0x9f, 0xd3, 0xff], cross: [255, 255, 255] };
    pub const LIGHT: Theme = Theme { dark: false, contrast: true, canvas: [255, 255, 255], grid: [0, 0, 0], grid_alpha: 0.09, grid_major: [0, 0, 0], grid_major_alpha: 0.12, select: [0x17, 0x69, 0xc2], hover: [0x2a, 0x8b, 0xe0], cross: [0, 0, 0] };
    /// White paper, black ink: used for plots.
    pub const PAPER: Theme = Theme { dark: false, contrast: false, canvas: [255, 255, 255], grid: [0, 0, 0], grid_alpha: 0.0, grid_major: [0, 0, 0], grid_major_alpha: 0.1, select: [0, 0, 0], hover: [0, 0, 0], cross: [0, 0, 0] };
}

/// Drawing-area preferences that affect rendering (a subset of the system variables).
#[derive(Clone, Copy, Debug)]
pub struct Prefs {
    pub lw_display: bool,
    /// LAYLOCKFADECTL 0..100
    pub lock_fade: f64,
    pub transparency: bool,
    pub fill: bool,
    pub grid: bool,
    pub grid_unit_x: f64,
    pub grid_unit_y: f64,
    pub grid_major: i64,
    /// Isometric grid (SNAPSTYL 1): lines at 30°, 90° and 150° whose vertical spacing is `grid_unit_y`.
    pub grid_iso: bool,
    pub ucs_icon: bool,
    /// Draw the UCS icon at the UCS origin (when visible) instead of the lower-left corner.
    pub ucs_origin: bool,
    /// QTEXT: draw text as boxes.
    pub qtext: bool,
}

impl Default for Prefs {
    fn default() -> Prefs {
        Prefs { lw_display: false, lock_fade: 50.0, transparency: true, fill: true, grid: true, grid_unit_x: 10.0, grid_unit_y: 10.0, grid_major: 5, grid_iso: false, ucs_icon: true, ucs_origin: false, qtext: false }
    }
}
