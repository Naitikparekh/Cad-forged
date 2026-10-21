//! Boolean operations on 2D areas bounded by bulge-polyline loops (REGION union / subtract / intersect).
//!
//! The boundaries of both operands are split at their mutual intersections. Every piece is kept or dropped
//! according to where it lies relative to the other operand, and the kept pieces are chained back into loops.
//! Lines and arcs stay exact (no tessellation), so a circle subtracted from a rectangle yields true arcs.
use crate::geom::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Op {
    Union,
    Intersect,
    /// `a` minus `b`.
    Subtract,
}

/// One closed boundary loop.
pub type Loop = Vec<Vertex>;
/// An area: its outer loop (counter-clockwise) first, then its holes (clockwise).
pub type Area = Vec<Loop>;

/// Net area of an [`Area`] (outer minus holes).
pub fn area_of(a: &Area) -> f64 {
    a.iter().map(|l| pline_area(l)).sum()
}

/// Loops of an operand as directed segments with the interior on the left: loops nested an even number of times
/// run counter-clockwise, odd ones clockwise (even-odd interior).
fn oriented(loops: &[Loop], tol: f64) -> Vec<Vec<Seg>> {
    let polys: Vec<Vec<Pt>> = loops.iter().map(|l| tess_segs(&pline_segs(l, true), tol)).collect();
    loops
        .iter()
        .enumerate()
        .filter(|(_, l)| l.len() >= 2)
        .map(|(i, l)| {
            let depth = (0..loops.len()).filter(|&j| j != i && polys[j].len() >= 3 && point_in_polygon(l[0].pt(), &polys[j])).count();
            let segs = pline_segs(l, true);
            let ccw = pline_area(l) > 0.0;
            if ccw == (depth % 2 == 0) {
                segs
            } else {
                segs.iter().rev().map(|s| s.reverse()).collect()
            }
        })
        .collect()
}

/// Even-odd containment against exact segments: the majority of three skew rays decides, which makes the result
/// immune to a single ray grazing a vertex.
fn contains(segs: &[Seg], p: Pt, reach: f64) -> bool {
    const DIRS: [f64; 3] = [0.7391, 2.1137, 4.0123];
    let odd = DIRS
        .iter()
        .filter(|a| {
            let ray = Seg::line(p, polar(p, **a, reach));
            segs.iter().map(|s| seg_intersect(&ray, s, false, false).len()).sum::<usize>() % 2 == 1
        })
        .count();
    odd >= 2
}

/// Pieces of `own` after cutting at every intersection with `own` and `other`.
fn split_pieces(own: &[Seg], other: &[Seg], eps: f64) -> Vec<Seg> {
    let (ob, xb): (Vec<Bounds>, Vec<Bounds>) = (own.iter().map(|s| s.bounds().grow(eps)).collect(), other.iter().map(|s| s.bounds()).collect());
    let mut out = vec![];
    for (i, s) in own.iter().enumerate() {
        let mut ts: Vec<f64> = vec![];
        for (j, o) in own.iter().enumerate() {
            if j != i && ob[i].overlaps(&o.bounds()) {
                ts.extend(seg_intersect(s, o, false, false).iter().map(|h| h.t1));
            }
        }
        for (j, o) in other.iter().enumerate() {
            if ob[i].overlaps(&xb[j]) {
                ts.extend(seg_intersect(s, o, false, false).iter().map(|h| h.t1));
            }
        }
        let mut cuts: Vec<f64> = vec![0.0];
        cuts.extend(ts.into_iter().filter(|t| *t > 1e-9 && *t < 1.0 - 1e-9));
        cuts.push(1.0);
        cuts.sort_by(|a, b| a.total_cmp(b));
        for w in cuts.windows(2) {
            if w[1] - w[0] > 1e-9 {
                let piece = s.slice(w[0], w[1]);
                if piece.length() > eps {
                    out.push(piece);
                }
            }
        }
    }
    out
}

/// Nearest distance from `p` to `segs` and the tangent of the nearest segment there.
fn nearest_on(segs: &[Seg], p: Pt) -> (f64, Pt) {
    let mut best = (f64::INFINITY, pt(1.0, 0.0));
    for s in segs {
        let (_, t, d) = s.nearest(p);
        if d < best.0 {
            best = (d, s.tangent(t));
        }
    }
    best
}

/// Merge `b` into `a` when they continue each other along the same line or circle.
fn merge(a: &Seg, b: &Seg, eps: f64) -> Option<Seg> {
    match (a, b) {
        (Seg::L { a: a0, b: a1, .. }, Seg::L { b: b1, .. }) => {
            let (d1, d2) = (*a1 - *a0, b.end() - b.start());
            (cross(d1, d2).abs() <= 1e-9 * len(d1) * len(d2) && dot(d1, d2) > 0.0).then(|| Seg::line(*a0, *b1))
        }
        (Seg::A { c: c1, r: r1, a0, sw: s1, .. }, Seg::A { c: c2, r: r2, sw: s2, .. }) => {
            let joined = dist(*c1, *c2) <= eps && (r1 - r2).abs() <= eps && s1.signum() == s2.signum() && dist(a.end(), b.start()) <= eps && s1.abs() + s2.abs() < TAU - 1e-6;
            joined.then(|| Seg::arc(*c1, *r1, *a0, s1 + s2))
        }
        _ => None,
    }
}

/// Join collinear line runs and co-circular arc runs of a closed loop (also across the start point).
fn tidy(segs: Vec<Seg>, eps: f64) -> Vec<Seg> {
    let mut out: Vec<Seg> = vec![];
    for s in segs {
        match out.last().and_then(|l| merge(l, &s, eps)) {
            Some(m) => *out.last_mut().expect("last") = m,
            None => out.push(s),
        }
    }
    while out.len() > 1 {
        match merge(&out[out.len() - 1], &out[0], eps) {
            Some(m) => {
                out[0] = m;
                out.pop();
            }
            None => break,
        }
    }
    out
}

/// Chain directed pieces into closed loops. At a node the next piece is the first one met turning clockwise from
/// the way back, which traces each face separately where boundaries touch.
fn chain(pieces: &[Seg], eps: f64) -> Vec<Vec<Seg>> {
    let mut nodes: Vec<Pt> = vec![];
    let mut node = |p: Pt| match nodes.iter().position(|q| dist(*q, p) <= eps * 4.0) {
        Some(i) => i,
        None => {
            nodes.push(p);
            nodes.len() - 1
        }
    };
    let ends: Vec<(usize, usize)> = pieces.iter().map(|s| (node(s.start()), node(s.end()))).collect();
    let mut used = vec![false; pieces.len()];
    let mut loops = vec![];
    for start in 0..pieces.len() {
        if used[start] {
            continue;
        }
        let (mut cur, mut run) = (start, vec![]);
        for _ in 0..=pieces.len() {
            used[cur] = true;
            run.push(pieces[cur]);
            let back = angle_of(Pt::ZERO, pieces[cur].reverse().tangent(0.0));
            let next = (0..pieces.len())
                .filter(|&k| ends[k].0 == ends[cur].1 && (!used[k] || k == start))
                .min_by(|&x, &y| {
                    let turn = |k: usize| {
                        let t = norm_angle(back - angle_of(Pt::ZERO, pieces[k].tangent(0.0)));
                        if t < 1e-9 {
                            TAU
                        } else {
                            t
                        }
                    };
                    turn(x).total_cmp(&turn(y))
                });
            match next {
                Some(k) if k == start => {
                    loops.push(run);
                    break;
                }
                Some(k) => cur = k,
                None => break,
            }
        }
    }
    loops
}

/// `a` combined with `b`. Each operand is a set of loops with even-odd interior (an outer loop with its holes, or
/// several disjoint regions). Returns the resulting areas: disjoint pieces, each with its holes.
pub fn boolean(a: &[Loop], b: &[Loop], op: Op) -> Vec<Area> {
    let all: Vec<Pt> = a.iter().chain(b.iter()).flatten().map(|v| v.pt()).collect();
    if all.is_empty() {
        return vec![];
    }
    let diag = {
        let bb = Bounds::of_points(&all);
        bb.width().hypot(bb.height())
    };
    let (eps, tol, reach) = (1e-9 * diag.max(1.0) * 100.0, diag.max(1.0) * 1e-4, diag * 4.0 + 1.0);
    let (la, lb) = (oriented(a, tol), oriented(b, tol));
    let (sa, sb): (Vec<Seg>, Vec<Seg>) = (la.concat(), lb.concat());
    let mut kept: Vec<Seg> = vec![];
    for p in split_pieces(&sa, &sb, eps) {
        let m = p.at(0.5);
        let (d, tb) = nearest_on(&sb, m);
        let keep = if d <= eps {
            // shared boundary: the same direction means both interiors lie on the same side
            let same = dot(p.tangent(0.5), tb) > 0.0;
            if op == Op::Subtract {
                !same
            } else {
                same
            }
        } else {
            let inside = contains(&sb, m, reach);
            if op == Op::Intersect {
                inside
            } else {
                !inside
            }
        };
        if keep {
            kept.push(p);
        }
    }
    for p in split_pieces(&sb, &sa, eps) {
        let m = p.at(0.5);
        if nearest_on(&sa, m).0 <= eps {
            continue; // shared boundary was decided from `a`'s side
        }
        let inside = contains(&sa, m, reach);
        match op {
            Op::Union if !inside => kept.push(p),
            Op::Intersect if inside => kept.push(p),
            Op::Subtract if inside => kept.push(p.reverse()),
            _ => {}
        }
    }
    let loops: Vec<Loop> = chain(&kept, eps).into_iter().map(|r| tidy(r, eps)).filter(|r| !r.is_empty()).map(|r| segs_to_verts(&r, true)).filter(|l| l.len() >= 2 && pline_area(l).abs() > eps * diag.max(1.0)).collect();
    let (outer, holes): (Vec<Loop>, Vec<Loop>) = loops.into_iter().partition(|l| pline_area(l) > 0.0);
    let polys = |l: &Loop| tess_segs(&pline_segs(l, true), tol);
    let mut areas: Vec<Area> = outer.into_iter().map(|o| vec![o]).collect();
    for h in holes {
        let probe = h[0].pt();
        let owner = areas.iter().enumerate().filter(|(_, ar)| point_in_polygon(probe, &polys(&ar[0]))).min_by(|x, y| pline_area(&x.1[0]).total_cmp(&pline_area(&y.1[0]))).map(|(i, _)| i);
        if let Some(i) = owner {
            areas[i].push(h);
        }
    }
    areas
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::f64::consts::PI;

    fn rect(x0: f64, y0: f64, x1: f64, y1: f64) -> Loop {
        vec![Vertex::new(x0, y0, 0.0), Vertex::new(x1, y0, 0.0), Vertex::new(x1, y1, 0.0), Vertex::new(x0, y1, 0.0)]
    }
    fn circle(cx: f64, cy: f64, r: f64) -> Loop {
        vec![Vertex::new(cx + r, cy, 1.0), Vertex::new(cx - r, cy, 1.0)]
    }
    /// `a` op `b` for two single-loop operands.
    fn run(a: &Loop, b: &Loop, op: Op) -> Vec<Area> {
        boolean(std::slice::from_ref(a), std::slice::from_ref(b), op)
    }
    fn total(areas: &[Area]) -> f64 {
        areas.iter().map(area_of).sum()
    }
    fn near(a: f64, b: f64) {
        assert!((a - b).abs() < 1e-6, "{a} != {b}");
    }

    #[test]
    fn overlapping_squares() {
        let (a, b) = (rect(0.0, 0.0, 10.0, 10.0), rect(5.0, 5.0, 15.0, 15.0));
        let u = run(&a, &b, Op::Union);
        assert_eq!(u.len(), 1);
        assert_eq!(u[0].len(), 1);
        assert_eq!(u[0][0].len(), 8, "a stepped outline has eight corners");
        near(total(&u), 175.0);
        near(total(&run(&a, &b, Op::Intersect)), 25.0);
        near(total(&run(&a, &b, Op::Subtract)), 75.0);
        near(total(&run(&b, &a, Op::Subtract)), 75.0);
    }

    #[test]
    fn disjoint_and_identical_operands() {
        let (a, b) = (rect(0.0, 0.0, 10.0, 10.0), rect(20.0, 0.0, 30.0, 10.0));
        assert_eq!(run(&a, &b, Op::Union).len(), 2);
        assert!(run(&a, &b, Op::Intersect).is_empty());
        near(total(&run(&a, &b, Op::Subtract)), 100.0);
        near(total(&run(&a, &a, Op::Union)), 100.0);
        assert!(run(&a, &a, Op::Subtract).is_empty());
        near(total(&run(&a, &a, Op::Intersect)), 100.0);
    }

    #[test]
    fn shared_edge_unites_into_one_rectangle() {
        let (a, b) = (rect(0.0, 0.0, 10.0, 10.0), rect(10.0, 0.0, 20.0, 10.0));
        let u = run(&a, &b, Op::Union);
        assert_eq!(u.len(), 1);
        assert_eq!(u[0][0].len(), 4, "collinear pieces merge back into single edges");
        near(total(&u), 200.0);
        assert!(run(&a, &b, Op::Intersect).is_empty());
        near(total(&run(&a, &b, Op::Subtract)), 100.0);
    }

    #[test]
    fn a_contained_operand_becomes_a_hole() {
        let (plate, hole) = (rect(0.0, 0.0, 20.0, 20.0), circle(10.0, 10.0, 4.0));
        let r = run(&plate, &hole, Op::Subtract);
        assert_eq!(r.len(), 1);
        assert_eq!(r[0].len(), 2, "outer loop plus one hole");
        near(total(&r), 400.0 - PI * 16.0);
        near(total(&run(&plate, &hole, Op::Union)), 400.0);
        near(total(&run(&plate, &hole, Op::Intersect)), PI * 16.0);
        // a disc larger than the hole but inside the plate: subtracting the drilled plate leaves the plug
        let plug = boolean(&[circle(10.0, 10.0, 8.0)], &r.concat(), Op::Subtract);
        assert_eq!(plug.len(), 1);
        near(total(&plug), PI * 16.0);
    }

    #[test]
    fn arcs_stay_arcs() {
        let (plate, bite) = (rect(0.0, 0.0, 10.0, 10.0), circle(10.0, 5.0, 3.0));
        let r = run(&plate, &bite, Op::Subtract);
        assert_eq!(r.len(), 1);
        assert!(r[0][0].iter().any(|v| v.b != 0.0), "the bite is a true arc");
        near(total(&r), 100.0 - PI * 9.0 / 2.0);
        near(total(&run(&plate, &bite, Op::Union)), 100.0 + PI * 9.0 / 2.0);
        near(total(&run(&plate, &bite, Op::Intersect)), PI * 9.0 / 2.0);
    }

    #[test]
    fn two_circles() {
        let (a, b) = (circle(0.0, 0.0, 5.0), circle(6.0, 0.0, 5.0));
        let (d, r): (f64, f64) = (6.0, 5.0);
        let lens = 2.0 * r * r * (d / (2.0 * r)).acos() - d / 2.0 * (4.0 * r * r - d * d).sqrt();
        near(total(&run(&a, &b, Op::Intersect)), lens);
        near(total(&run(&a, &b, Op::Union)), 2.0 * PI * 25.0 - lens);
        near(total(&run(&a, &b, Op::Subtract)), PI * 25.0 - lens);
    }

    #[test]
    fn touching_at_a_corner_stays_two_areas() {
        let u = run(&rect(0.0, 0.0, 10.0, 10.0), &rect(10.0, 10.0, 20.0, 20.0), Op::Union);
        assert_eq!(u.len(), 2);
        near(total(&u), 200.0);
    }

    #[test]
    fn operands_with_holes_and_several_areas() {
        // a frame (square with a square hole) united with a block filling the hole gives a solid square
        let frame = vec![rect(0.0, 0.0, 30.0, 30.0), rect(10.0, 10.0, 20.0, 20.0)];
        let u = boolean(&frame, &[rect(10.0, 10.0, 20.0, 20.0)], Op::Union);
        assert_eq!(u.len(), 1);
        assert_eq!(u[0].len(), 1);
        near(total(&u), 900.0);
        // the frame minus a bar across it splits into two C-shapes
        let split = boolean(&frame, &[rect(-5.0, 12.0, 35.0, 18.0)], Op::Subtract);
        assert_eq!(split.len(), 2);
        near(total(&split), 800.0 - 2.0 * 10.0 * 6.0);
    }

    /// Random rectangles, circles and D shapes on a coarse grid, so edges coincide, circles touch and
    /// vertices land on the other shape. Inclusion-exclusion must hold for every pair.
    #[test]
    fn inclusion_exclusion_holds_on_degenerate_grids() {
        let mut seed = 0x2545f491u64;
        let mut next = |n: u64| {
            seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
            (seed >> 33) % n
        };
        let shape = |next: &mut dyn FnMut(u64) -> u64| -> Loop {
            let (x, y) = (next(7) as f64, next(7) as f64);
            match next(3) {
                0 => rect(x, y, x + 1.0 + next(5) as f64, y + 1.0 + next(5) as f64),
                1 => circle(x, y, 1.0 + next(4) as f64),
                _ => {
                    // a D shape: a rectangle with a semicircular right end
                    let (w, h) = (2.0 + next(4) as f64, 2.0 * (1.0 + next(2) as f64));
                    vec![Vertex::new(x, y, 0.0), Vertex::new(x + w, y, 1.0), Vertex::new(x + w, y + h, 0.0), Vertex::new(x, y + h, 0.0)]
                }
            }
        };
        for case in 0..400 {
            let (a, b) = (shape(&mut next), shape(&mut next));
            let (aa, ab) = (pline_area(&a).abs(), pline_area(&b).abs());
            let (u, i, d) = (total(&run(&a, &b, Op::Union)), total(&run(&a, &b, Op::Intersect)), total(&run(&a, &b, Op::Subtract)));
            let msg = format!("case {case}: {a:?} vs {b:?}");
            assert!((u + i - aa - ab).abs() < 1e-6, "union + intersection != sum of areas ({u} + {i} vs {aa} + {ab}); {msg}");
            assert!((d - (aa - i)).abs() < 1e-6, "difference {d} != {aa} - {i}; {msg}");
        }
    }
}
