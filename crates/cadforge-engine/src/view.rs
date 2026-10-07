//! Orthographic camera shared by 2D drafting (plan view) and 3D inspection (port of `src/engine/view.js`).
//!
//! World z = 0 is the drawing plane. The view looks at `target` from direction (yaw, pitch):
//! pitch = 90° is plan view (looking straight down), yaw rotates the plan, pitch = 0 looks horizontally.
//! Screen x grows right, screen y grows down.
use cadforge_core::drawing::ViewState;
use cadforge_core::geom::{clamp, Bounds, Pt};
use cadforge_core::solid::V3;
use std::f64::consts::{FRAC_PI_2, PI};

pub const DEFAULT_SCALE: f64 = 4.0;

#[derive(Clone, Debug)]
pub struct View {
    pub target: V3,
    /// Pixels per drawing unit.
    pub scale: f64,
    pub yaw: f64,
    pub pitch: f64,
    /// Viewport size in CSS pixels.
    pub w: f64,
    pub h: f64,
    /// ZOOM Previous stack.
    pub prev: Vec<ViewState>,
}

/// Affine matrix `[a b c d e f]` mapping world (x, y, 0) to screen pixels.
pub type ScreenMat = [f64; 6];

/// A projected 3D point: screen x/y and depth (larger = farther from the viewer).
#[derive(Clone, Copy, Debug)]
pub struct Projected {
    pub x: f64,
    pub y: f64,
    pub depth: f64,
}

impl Default for View {
    fn default() -> View {
        View::new()
    }
}

impl View {
    pub fn new() -> View {
        View { target: V3::new(0.0, 0.0, 0.0), scale: DEFAULT_SCALE, yaw: 0.0, pitch: FRAC_PI_2, w: 800.0, h: 600.0, prev: vec![] }
    }

    pub fn state(&self) -> ViewState {
        ViewState { cx: self.target.x, cy: self.target.y, cz: self.target.z, scale: self.scale, yaw: self.yaw, pitch: self.pitch }
    }

    pub fn set_state(&mut self, s: &ViewState) {
        self.target = V3::new(s.cx, s.cy, s.cz);
        self.scale = s.scale;
        self.yaw = s.yaw;
        self.pitch = s.pitch;
    }

    pub fn is_plan(&self) -> bool {
        (self.pitch - FRAC_PI_2).abs() < 1e-6 && self.yaw.abs() < 1e-6
    }

    /// Can points be picked on the z = 0 plane in this orientation? (not edge-on)
    pub fn can_draft(&self) -> bool {
        self.pitch.sin() > 0.2
    }

    pub fn push(&mut self) {
        self.prev.push(self.state());
        if self.prev.len() > 30 {
            self.prev.remove(0);
        }
    }

    pub fn pop_previous(&mut self) -> bool {
        match self.prev.pop() {
            Some(s) => {
                self.set_state(&s);
                true
            }
            None => false,
        }
    }

    pub fn matrix(&self) -> ScreenMat {
        let (cy, sy, sp, s, t) = (self.yaw.cos(), self.yaw.sin(), self.pitch.sin(), self.scale, self.target);
        [s * cy, -s * sp * sy, -s * sy, -s * sp * cy, self.w / 2.0 - s * (cy * t.x - sy * t.y), self.h / 2.0 + s * sp * (sy * t.x + cy * t.y)]
    }

    /// Project a 3D world point.
    pub fn project(&self, p: V3) -> Projected {
        let (x, y, z) = (p.x - self.target.x, p.y - self.target.y, p.z - self.target.z);
        let (cy, sy, cp, sp) = (self.yaw.cos(), self.yaw.sin(), self.pitch.cos(), self.pitch.sin());
        let (x1, y1) = (x * cy - y * sy, x * sy + y * cy);
        let up = y1 * sp + z * cp;
        let depth = -(y1 * cp) + z * sp;
        Projected { x: self.w / 2.0 + x1 * self.scale, y: self.h / 2.0 - up * self.scale, depth: -depth }
    }

    pub fn to_screen(&self, p: Pt) -> Pt {
        let m = self.matrix();
        Pt { x: m[0] * p.x + m[2] * p.y + m[4], y: m[1] * p.x + m[3] * p.y + m[5] }
    }

    /// Screen pixel -> world point on the plane z = 0.
    pub fn to_world(&self, sx: f64, sy: f64) -> Pt {
        let m = self.matrix();
        let det = m[0] * m[3] - m[1] * m[2];
        if det.abs() < 1e-12 {
            return Pt { x: self.target.x, y: self.target.y };
        }
        let (dx, dy) = (sx - m[4], sy - m[5]);
        Pt { x: (m[3] * dx - m[2] * dy) / det, y: (-m[1] * dx + m[0] * dy) / det }
    }

    /// Visible world rectangle (plan views only).
    pub fn world_bounds(&self) -> Bounds {
        let c = [(0.0, 0.0), (self.w, 0.0), (self.w, self.h), (0.0, self.h)].map(|(x, y)| self.to_world(x, y));
        Bounds::of_points(&c)
    }

    pub fn pan(&mut self, dx_px: f64, dy_px: f64) {
        let (a, b) = (self.to_world(0.0, 0.0), self.to_world(dx_px, dy_px));
        self.target.x -= b.x - a.x;
        self.target.y -= b.y - a.y;
    }

    /// Zoom keeping the world point under (sx, sy) fixed.
    pub fn zoom_at(&mut self, sx: f64, sy: f64, factor: f64) {
        let before = self.to_world(sx, sy);
        self.scale = clamp(self.scale * factor, 1e-5, 1e7);
        let after = self.to_world(sx, sy);
        self.target.x += before.x - after.x;
        self.target.y += before.y - after.y;
    }

    /// Fit a world rectangle with a margin (fraction of the viewport).
    pub fn fit(&mut self, b: Option<Bounds>, margin: f64) {
        let Some(b) = b else {
            self.target = V3::new(0.0, 0.0, 0.0);
            self.scale = DEFAULT_SCALE;
            return;
        };
        let (w, h) = ((b.max_x - b.min_x).max(1e-6), (b.max_y - b.min_y).max(1e-6));
        self.target = V3::new((b.min_x + b.max_x) / 2.0, (b.min_y + b.max_y) / 2.0, self.target.z);
        // plan fit works in the rotated frame
        let (c, s) = (self.yaw.cos().abs(), self.yaw.sin().abs());
        let (rw, rh) = (w * c + h * s, w * s + h * c);
        self.scale = clamp((self.w * (1.0 - 2.0 * margin) / rw).min(self.h * (1.0 - 2.0 * margin) / (rh * self.pitch.sin().max(0.3))), 1e-5, 1e7);
    }

    pub fn fit_points_3d(&mut self, pts: &[V3], margin: f64) {
        if pts.is_empty() {
            return;
        }
        let (mut lo, mut hi) = (V3::new(f64::INFINITY, f64::INFINITY, f64::INFINITY), V3::new(f64::NEG_INFINITY, f64::NEG_INFINITY, f64::NEG_INFINITY));
        for p in pts {
            lo = V3::new(lo.x.min(p.x), lo.y.min(p.y), lo.z.min(p.z));
            hi = V3::new(hi.x.max(p.x), hi.y.max(p.y), hi.z.max(p.z));
        }
        self.target = V3::new((lo.x + hi.x) / 2.0, (lo.y + hi.y) / 2.0, (lo.z + hi.z) / 2.0);
        self.scale = 1.0;
        let (mut min_x, mut max_x, mut min_y, mut max_y) = (f64::INFINITY, f64::NEG_INFINITY, f64::INFINITY, f64::NEG_INFINITY);
        for p in pts {
            let q = self.project(*p);
            min_x = min_x.min(q.x);
            max_x = max_x.max(q.x);
            min_y = min_y.min(q.y);
            max_y = max_y.max(q.y);
        }
        let (w, h) = ((max_x - min_x).max(1e-6), (max_y - min_y).max(1e-6));
        self.scale = clamp((self.w * (1.0 - 2.0 * margin) / w).min(self.h * (1.0 - 2.0 * margin) / h), 1e-5, 1e7);
    }

    pub fn orbit(&mut self, dyaw: f64, dpitch: f64) {
        self.yaw += dyaw;
        self.pitch = clamp(self.pitch + dpitch, -FRAC_PI_2 + 0.01, FRAC_PI_2);
    }

    /// Standard view directions (names as in the ViewCube). Returns false for an unknown name.
    pub fn set_preset(&mut self, name: &str) -> bool {
        let d = PI / 180.0;
        let (yaw, pitch) = match name {
            "top" => (0.0, 90.0),
            "bottom" => (0.0, -90.0),
            "front" => (0.0, 0.0),
            "back" => (180.0, 0.0),
            "left" => (90.0, 0.0),
            "right" => (-90.0, 0.0),
            "swiso" => (45.0, 35.264),
            "seiso" => (-45.0, 35.264),
            "neiso" => (-135.0, 35.264),
            "nwiso" => (135.0, 35.264),
            _ => return false,
        };
        self.yaw = yaw * d;
        self.pitch = pitch * d;
        true
    }

    /// Closest standard view name for the current orientation.
    pub fn preset_name(&self) -> &'static str {
        let d = 180.0 / PI;
        let yaw = ((self.yaw * d % 360.0) + 360.0) % 360.0;
        let pitch = self.pitch * d;
        if (pitch - 90.0).abs() < 0.5 {
            return if yaw.abs() < 0.5 || (yaw - 360.0).abs() < 0.5 { "Top" } else { "Custom View" };
        }
        if (pitch + 90.0).abs() < 0.5 {
            return "Bottom";
        }
        if pitch.abs() < 0.5 {
            for (n, a) in [("Front", 0.0), ("Left", 90.0), ("Back", 180.0), ("Right", 270.0)] {
                if (yaw - a).abs() < 0.5 {
                    return n;
                }
            }
        }
        if (pitch - 35.264).abs() < 0.5 {
            for (n, a) in [("SW Isometric", 45.0), ("NW Isometric", 135.0), ("NE Isometric", 225.0), ("SE Isometric", 315.0)] {
                if (yaw - a).abs() < 0.5 {
                    return n;
                }
            }
        }
        "Custom View"
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cadforge_core::geom::pt;

    #[test]
    fn screen_world_round_trip() {
        let mut v = View::new();
        v.scale = 10.0;
        v.target = V3::new(5.0, 7.0, 0.0);
        v.yaw = 0.3;
        v.pitch = 1.0;
        let p = pt(12.0, -3.0);
        let s = v.to_screen(p);
        let w = v.to_world(s.x, s.y);
        assert!((w.x - p.x).abs() < 1e-9 && (w.y - p.y).abs() < 1e-9);
    }

    #[test]
    fn plan_view_orientation_and_zoom() {
        let mut v = View::new();
        v.w = 800.0;
        v.h = 600.0;
        v.scale = 2.0;
        assert!(v.is_plan());
        let c = v.to_screen(pt(0.0, 0.0));
        assert_eq!((c.x, c.y), (400.0, 300.0));
        assert!(v.to_screen(pt(0.0, 10.0)).y < c.y, "world +y is up the screen");
        let under = v.to_world(100.0, 100.0);
        v.zoom_at(100.0, 100.0, 2.0);
        let after = v.to_world(100.0, 100.0);
        assert!((under.x - after.x).abs() < 1e-9 && (under.y - after.y).abs() < 1e-9);
        v.fit(Some(Bounds::new(0.0, 0.0, 100.0, 50.0)), 0.06);
        assert!((v.target.x - 50.0).abs() < 1e-9 && (v.target.y - 25.0).abs() < 1e-9);
        let b = v.world_bounds();
        assert!(b.min_x < 0.0 && b.max_x > 100.0);
    }

    #[test]
    fn presets_and_names() {
        let mut v = View::new();
        assert_eq!(v.preset_name(), "Top");
        assert!(v.set_preset("swiso"));
        assert_eq!(v.preset_name(), "SW Isometric");
        assert!(v.set_preset("front"));
        assert_eq!(v.preset_name(), "Front");
        assert!(!v.set_preset("nope"));
        v.orbit(0.0, 10.0);
        assert!(v.pitch <= FRAC_PI_2);
    }

    #[test]
    fn previous_stack() {
        let mut v = View::new();
        v.push();
        v.scale = 99.0;
        assert!(v.pop_previous());
        assert_eq!(v.scale, DEFAULT_SCALE);
        assert!(!v.pop_previous());
    }
}
