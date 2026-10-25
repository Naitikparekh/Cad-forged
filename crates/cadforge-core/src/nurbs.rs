//! NURBS curves: the exact definition of a spline by control vertices, weights and a knot vector, as AutoCAD stores
//! splines (and as the SPLINE command's control vertex method makes them).
use crate::geom::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Nurbs {
    pub deg: usize,
    /// Control vertices.
    pub cv: Vec<Pt>,
    /// One weight per control vertex; empty when every weight is 1 (not rational).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub w: Vec<f64>,
    /// `cv.len() + deg + 1` non-decreasing knots.
    pub knots: Vec<f64>,
}

impl Nurbs {
    /// A clamped spline through the ends of its control polygon with uniform inner knots.
    pub fn clamped(deg: usize, cv: Vec<Pt>) -> Nurbs {
        let deg = deg.clamp(1, cv.len().saturating_sub(1).max(1));
        let inner = cv.len().saturating_sub(deg + 1);
        let mut knots = vec![0.0; deg + 1];
        knots.extend((1..=inner).map(|i| i as f64));
        knots.extend(vec![(inner + 1) as f64; deg + 1]);
        Nurbs { deg, cv, w: vec![], knots }
    }

    /// From file data; `None` when the counts do not fit together or the knots go backwards.
    pub fn new(deg: usize, cv: Vec<Pt>, w: Vec<f64>, knots: Vec<f64>) -> Option<Nurbs> {
        let ok = deg >= 1
            && cv.len() > deg
            && knots.len() == cv.len() + deg + 1
            && knots.windows(2).all(|k| k[0] <= k[1])
            && knots.iter().chain(w.iter()).all(|v| v.is_finite())
            && knots[deg] < knots[cv.len()]
            && (w.is_empty() || (w.len() == cv.len() && w.iter().all(|x| *x > 0.0)));
        let w = if w.iter().all(|x| (*x - 1.0).abs() < 1e-12) { vec![] } else { w };
        ok.then_some(Nurbs { deg, cv, w, knots })
    }

    pub fn weight(&self, i: usize) -> f64 {
        self.w.get(i).copied().unwrap_or(1.0)
    }

    pub fn rational(&self) -> bool {
        !self.w.is_empty()
    }

    /// The parameter range.
    pub fn domain(&self) -> (f64, f64) {
        (self.knots[self.deg], self.knots[self.cv.len()])
    }

    /// The curve point at parameter `t` (de Boor's algorithm on homogeneous coordinates).
    pub fn at(&self, t: f64) -> Pt {
        let (deg, n, k) = (self.deg, self.cv.len() - 1, &self.knots);
        let (t0, t1) = self.domain();
        let t = t.clamp(t0, t1);
        let mut s = deg;
        while s < n && t >= k[s + 1] {
            s += 1;
        }
        let mut d: Vec<(f64, f64, f64)> = (0..=deg)
            .map(|i| {
                let idx = s - deg + i;
                let w = self.weight(idx);
                (self.cv[idx].x * w, self.cv[idx].y * w, w)
            })
            .collect();
        for r in 1..=deg {
            for j in (r..=deg).rev() {
                let den = k[j + 1 + s - r] - k[j + s - deg];
                let a = if den == 0.0 { 0.0 } else { (t - k[j + s - deg]) / den };
                d[j] = ((1.0 - a) * d[j - 1].0 + a * d[j].0, (1.0 - a) * d[j - 1].1 + a * d[j].1, (1.0 - a) * d[j - 1].2 + a * d[j].2);
            }
        }
        pt(d[deg].0 / d[deg].2, d[deg].1 / d[deg].2)
    }

    /// Points along the curve: `per_span` per knot span (24 to 2000 in all), ending exactly at its end.
    pub fn sample(&self, per_span: usize) -> Vec<Pt> {
        let (t0, t1) = self.domain();
        let mut spans: Vec<f64> = self.knots[self.deg..=self.cv.len()].to_vec();
        spans.dedup();
        let count = ((spans.len().max(2) - 1) * per_span.max(1)).clamp(24, 2000);
        let mut pts: Vec<Pt> = (0..count).map(|i| self.at(t0 + (t1 - t0) * i as f64 / count as f64)).collect();
        pts.push(self.at(t1));
        pts
    }

    /// The curve ends where it starts.
    pub fn closed(&self) -> bool {
        let (t0, t1) = self.domain();
        dist(self.at(t0), self.at(t1)) < 1e-9 * (1.0 + self.cv.iter().map(|p| p.x.abs().max(p.y.abs())).fold(0.0, f64::max))
    }

    /// Apply an affine map to the curve (NURBS are affine-invariant: mapping the control vertices maps the curve).
    pub fn map(&mut self, f: impl Fn(Pt) -> Pt) {
        for p in &mut self.cv {
            *p = f(*p);
        }
    }

    /// The same curve run the other way.
    pub fn reversed(&self) -> Nurbs {
        let (a, b) = (self.knots[0], *self.knots.last().expect("knots"));
        Nurbs {
            deg: self.deg,
            cv: self.cv.iter().rev().copied().collect(),
            w: self.w.iter().rev().copied().collect(),
            knots: self.knots.iter().rev().map(|k| a + b - k).collect(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clamped_curves_pass_through_their_end_vertices() {
        let n = Nurbs::clamped(3, vec![pt(0.0, 0.0), pt(1.0, 2.0), pt(3.0, 2.0), pt(4.0, 0.0), pt(6.0, 1.0)]);
        assert_eq!(n.knots, vec![0.0, 0.0, 0.0, 0.0, 1.0, 2.0, 2.0, 2.0, 2.0]);
        let s = n.sample(8);
        assert_eq!(s[0], pt(0.0, 0.0));
        assert!(dist(*s.last().unwrap(), pt(6.0, 1.0)) < 1e-12);
        assert!(!n.closed());
        let r = n.reversed();
        assert!(dist(r.at(r.domain().0), pt(6.0, 1.0)) < 1e-12);
    }

    #[test]
    fn a_rational_quadratic_is_an_exact_circle_arc() {
        // a quarter circle: weights 1, cos 45°, 1
        let w = std::f64::consts::FRAC_1_SQRT_2;
        let n = Nurbs::new(2, vec![pt(1.0, 0.0), pt(1.0, 1.0), pt(0.0, 1.0)], vec![1.0, w, 1.0], vec![0.0, 0.0, 0.0, 1.0, 1.0, 1.0]).unwrap();
        assert!(n.rational());
        for p in n.sample(16) {
            assert!((p.x.hypot(p.y) - 1.0).abs() < 1e-12, "{p:?}");
        }
        assert!(Nurbs::new(2, vec![pt(0.0, 0.0), pt(1.0, 1.0)], vec![], vec![0.0; 5]).is_none(), "too few vertices");
        assert!(Nurbs::new(1, vec![pt(0.0, 0.0), pt(1.0, 1.0)], vec![], vec![1.0, 0.0, 1.0, 2.0]).is_none(), "knots go backwards");
    }
}
