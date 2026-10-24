//! A uniform grid over the bounds of a drawing's entities: "which entities can be near here" without looking at all of them.
//!
//! The grid answers with positions in the entity list of one space, in ascending order, so a caller that loops over the
//! answer sees the entities in drawing order exactly as a loop over the whole list would. Entities without bounds
//! (construction lines, rays) and entities too large to be worth storing in many cells are part of every answer.
use crate::geom::Bounds;
use std::collections::HashMap;

/// Below this many entities a scan is as fast as the grid.
pub const MIN_ENTITIES: usize = 512;
/// An entity that would fill more cells than this is kept in the "always" list instead.
const MAX_CELLS_PER_ENTITY: i64 = 64;
/// Cells per axis, at most.
const MAX_AXIS_CELLS: f64 = 4096.0;

pub struct SpatialIndex {
    n: usize,
    cell: f64,
    ox: f64,
    oy: f64,
    cells: HashMap<(i32, i32), Vec<u32>>,
    always: Vec<u32>,
    /// Extent of the stored cells: a query window beyond it is clamped.
    cx_range: (i32, i32),
    cy_range: (i32, i32),
}

impl SpatialIndex {
    /// Build over `n` entities whose bounds are given by `bounds(i)` (`None` = no bounds).
    pub fn build(n: usize, bounds: impl Fn(usize) -> Option<Bounds>) -> SpatialIndex {
        let all: Vec<Option<Bounds>> = (0..n).map(&bounds).collect();
        let mut ext: Option<Bounds> = None;
        let mut bounded = 0usize;
        for b in all.iter().flatten() {
            ext = Bounds::union_opt(ext, Some(*b));
            bounded += 1;
        }
        let (ox, oy, w, h) = ext.map_or((0.0, 0.0, 1.0, 1.0), |b| {
            (
                b.min_x,
                b.min_y,
                (b.max_x - b.min_x).max(1e-9),
                (b.max_y - b.min_y).max(1e-9),
            )
        });
        // about eight entities per cell on average, but never more than MAX_AXIS_CELLS cells along an axis
        let target_cells = (bounded as f64 / 8.0).max(1.0);
        let mut cell = (w * h / target_cells).sqrt();
        cell = cell.max(w.max(h) / MAX_AXIS_CELLS).max(1e-9);
        let mut idx = SpatialIndex {
            n,
            cell,
            ox,
            oy,
            cells: HashMap::new(),
            always: vec![],
            cx_range: (0, 0),
            cy_range: (0, 0),
        };
        let (mut cx_lo, mut cx_hi, mut cy_lo, mut cy_hi) = (i32::MAX, i32::MIN, i32::MAX, i32::MIN);
        for (i, b) in all.iter().enumerate() {
            let Some(b) = b else {
                idx.always.push(i as u32);
                continue;
            };
            let ((x0, y0), (x1, y1)) =
                (idx.cell_of(b.min_x, b.min_y), idx.cell_of(b.max_x, b.max_y));
            if (x1 as i64 - x0 as i64 + 1) * (y1 as i64 - y0 as i64 + 1) > MAX_CELLS_PER_ENTITY {
                idx.always.push(i as u32);
                continue;
            }
            for cx in x0..=x1 {
                for cy in y0..=y1 {
                    idx.cells.entry((cx, cy)).or_default().push(i as u32);
                }
            }
            cx_lo = cx_lo.min(x0);
            cx_hi = cx_hi.max(x1);
            cy_lo = cy_lo.min(y0);
            cy_hi = cy_hi.max(y1);
        }
        if cx_lo <= cx_hi {
            idx.cx_range = (cx_lo, cx_hi);
            idx.cy_range = (cy_lo, cy_hi);
        }
        idx
    }

    fn cell_of(&self, x: f64, y: f64) -> (i32, i32) {
        let c = |v: f64, o: f64| ((v - o) / self.cell).floor().clamp(-2e9, 2e9) as i32;
        (c(x, self.ox), c(y, self.oy))
    }

    /// Number of entities the index was built for.
    pub fn len(&self) -> usize {
        self.n
    }

    pub fn is_empty(&self) -> bool {
        self.n == 0
    }

    /// Positions of the entities that may overlap `win`, ascending and without repeats. May contain entities that do not
    /// overlap it; never leaves one out.
    pub fn candidates(&self, win: &Bounds) -> Vec<usize> {
        let ((x0, y0), (x1, y1)) = (
            self.cell_of(win.min_x, win.min_y),
            self.cell_of(win.max_x, win.max_y),
        );
        let (x0, x1) = (x0.max(self.cx_range.0), x1.min(self.cx_range.1));
        let (y0, y1) = (y0.max(self.cy_range.0), y1.min(self.cy_range.1));
        let mut out: Vec<u32> = self.always.clone();
        if x0 <= x1 && y0 <= y1 {
            let span = (x1 as i64 - x0 as i64 + 1) * (y1 as i64 - y0 as i64 + 1);
            if span as usize > self.cells.len() {
                // a window covering most of the drawing: walk the filled cells instead of every cell number
                for ((cx, cy), v) in &self.cells {
                    if (x0..=x1).contains(cx) && (y0..=y1).contains(cy) {
                        out.extend_from_slice(v);
                    }
                }
            } else {
                for cx in x0..=x1 {
                    for cy in y0..=y1 {
                        if let Some(v) = self.cells.get(&(cx, cy)) {
                            out.extend_from_slice(v);
                        }
                    }
                }
            }
        }
        out.sort_unstable();
        out.dedup();
        out.into_iter().map(|i| i as usize).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rect(x: f64, y: f64, w: f64, h: f64) -> Bounds {
        Bounds::new(x, y, x + w, y + h)
    }

    /// A deterministic scatter of small boxes, some long ones, some without bounds.
    fn scatter(n: usize) -> Vec<Option<Bounds>> {
        let mut seed = 99u64;
        let mut r = || {
            seed = seed
                .wrapping_mul(6364136223846793005)
                .wrapping_add(1442695040888963407);
            ((seed >> 33) as f64) / (1u64 << 31) as f64
        };
        (0..n)
            .map(|i| match i % 97 {
                0 => None,
                1 => Some(rect(0.0, r() * 1000.0, 1000.0, 1.0)), // a long thin box across the drawing
                _ => Some(rect(r() * 1000.0, r() * 1000.0, r() * 6.0, r() * 6.0)),
            })
            .collect()
    }

    #[test]
    fn the_answer_is_every_entity_that_overlaps_in_ascending_order() {
        let b = scatter(5000);
        let idx = SpatialIndex::build(b.len(), |i| b[i]);
        for win in [
            rect(100.0, 100.0, 20.0, 20.0),
            rect(-50.0, -50.0, 30.0, 30.0),
            rect(990.0, 990.0, 100.0, 100.0),
            rect(0.0, 0.0, 1000.0, 1000.0),
            rect(500.0, 500.0, 0.0, 0.0),
            rect(2000.0, 2000.0, 5.0, 5.0),
        ] {
            let got = idx.candidates(&win);
            assert!(got.windows(2).all(|w| w[0] < w[1]), "ascending and unique");
            for (i, bb) in b.iter().enumerate() {
                let needed = bb.map_or(true, |bb| bb.overlaps(&win));
                if needed {
                    assert!(
                        got.contains(&i),
                        "entity {i} ({bb:?}) overlaps {win:?} but was left out"
                    );
                }
            }
            // and the grid really narrows things down for a small window
            if win.max_x - win.min_x < 50.0 {
                assert!(
                    got.len() < 500,
                    "{} candidates for a small window",
                    got.len()
                );
            }
        }
    }

    #[test]
    fn unbounded_and_huge_entities_are_always_candidates() {
        let mut b = scatter(1000);
        b[5] = Some(rect(-1e6, -1e6, 2e6, 2e6)); // would fill millions of cells
        let idx = SpatialIndex::build(b.len(), |i| b[i]);
        let got = idx.candidates(&rect(10.0, 10.0, 1.0, 1.0));
        assert!(
            got.contains(&0) && got.contains(&97),
            "entities without bounds"
        );
        assert!(got.contains(&5), "the huge one");
    }

    #[test]
    fn edge_cases_do_not_panic() {
        let empty = SpatialIndex::build(0, |_| None);
        assert!(empty.is_empty() && empty.candidates(&rect(0.0, 0.0, 1.0, 1.0)).is_empty());
        let none = SpatialIndex::build(3, |_| None);
        assert_eq!(none.candidates(&rect(0.0, 0.0, 1.0, 1.0)), vec![0, 1, 2]);
        let one_point =
            SpatialIndex::build(2, |i| Some(rect(5.0, 5.0, 0.0, 0.0)).filter(|_| i == 0));
        assert_eq!(one_point.candidates(&rect(4.0, 4.0, 2.0, 2.0)), vec![0, 1]);
        let nan_free = SpatialIndex::build(2, |_| Some(rect(1e-12, 1e-12, 0.0, 0.0)));
        assert_eq!(nan_free.candidates(&rect(0.0, 0.0, 1.0, 1.0)), vec![0, 1]);
    }
}
