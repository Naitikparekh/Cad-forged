//! Parametric paths: an entity's segments in order, addressed by `u = segment index + t`.
use crate::drawing::Drawing;
use crate::entities::{segs_of, Entity, Kind};
use crate::geom::*;

/// Copy the inheritable properties (layer, colour, linetype, lineweight...) of `from` onto `to`.
pub fn inherit_props(from: &Entity, mut to: Entity) -> Entity {
    if !from.layer.is_empty() {
        to.layer = from.layer.clone();
    }
    if from.color.is_some() {
        to.color = from.color;
    }
    if from.ltype.is_some() {
        to.ltype = from.ltype.clone();
    }
    if from.lw.is_some() {
        to.lw = from.lw;
    }
    if from.lts.is_some() {
        to.lts = from.lts;
    }
    if from.transp.is_some() {
        to.transp = from.transp;
    }
    if from.grp.is_some() {
        to.grp = from.grp.clone();
    }
    to
}

#[derive(Clone, Debug)]
pub struct Path {
    pub segs: Vec<Seg>,
    pub closed: bool,
}

/// Editable path of a line/arc/circle/pline, or `None` for entities that cannot be edited parametrically.
pub fn path_of(e: &Entity, doc: &Drawing) -> Option<Path> {
    match &e.kind {
        Kind::Line { .. } | Kind::Arc { .. } => Some(Path {
            segs: segs_of(e, doc).to_vec(),
            closed: false,
        }),
        Kind::Circle { .. } => Some(Path {
            segs: segs_of(e, doc).to_vec(),
            closed: true,
        }),
        Kind::Pline { closed, .. } => Some(Path {
            segs: segs_of(e, doc).to_vec(),
            closed: *closed,
        }),
        _ => None,
    }
}

/// Nearest point on a path.
#[derive(Clone, Copy, Debug)]
pub struct PathHit {
    pub u: f64,
    pub pt: Pt,
    pub d: f64,
}

pub fn path_nearest(path: &Path, p: Pt) -> PathHit {
    let mut best = PathHit {
        u: 0.0,
        pt: p,
        d: f64::INFINITY,
    };
    for (i, s) in path.segs.iter().enumerate() {
        let (q, t, d) = s.nearest(p);
        if d < best.d {
            best = PathHit {
                u: i as f64 + t,
                pt: q,
                d,
            };
        }
    }
    best
}

pub fn path_length(path: &Path) -> f64 {
    path.segs.iter().map(|s| s.length()).sum()
}

/// Segments from u0 to u1 on the same lap (0 <= u0 <= u1 <= n).
pub fn slice_linear(segs: &[Seg], u0: f64, u1: f64) -> Vec<Seg> {
    let eps = 1e-9;
    let mut out = vec![];
    let mut i = (u0 + eps).floor() as usize;
    while i < segs.len() && (i as f64) < u1 - eps {
        let ta = (u0 - i as f64).max(0.0);
        let tb = (u1 - i as f64).min(1.0);
        if tb - ta > eps {
            out.push(segs[i].slice(ta, tb));
        }
        i += 1;
    }
    out
}

/// Slice a path from u0 to u1; on closed paths u1 < u0 wraps through the start.
pub fn path_slice(path: &Path, u0: f64, u1: f64) -> Vec<Seg> {
    let n = path.segs.len() as f64;
    if u1 >= u0 {
        return slice_linear(&path.segs, u0, u1);
    }
    if !path.closed {
        return slice_linear(&path.segs, u1, u0)
            .iter()
            .rev()
            .map(|s| s.reverse())
            .collect();
    }
    let mut v = slice_linear(&path.segs, u0, n);
    v.extend(slice_linear(&path.segs, 0.0, u1));
    v
}

/// Merge consecutive arcs of the same circle into one (so a trimmed circle is a single arc).
pub fn merge_segs(segs: &[Seg]) -> Vec<Seg> {
    let mut out: Vec<Seg> = vec![];
    for s in segs {
        if let (
            Some(Seg::A {
                c: lc,
                r: lr,
                a0: la0,
                sw: lsw,
                full,
            }),
            Seg::A { c, r, sw, .. },
        ) = (out.last().copied(), s)
        {
            let last = out.last().copied().expect("last exists");
            if same_pt(lc, *c, 1e-9)
                && (lr - r).abs() < 1e-9
                && lsw.signum() == sw.signum()
                && same_pt(last.end(), s.start(), 1e-7)
                && lsw.abs() + sw.abs() <= TAU + 1e-9
            {
                *out.last_mut().expect("last exists") = Seg::A {
                    c: lc,
                    r: lr,
                    a0: la0,
                    sw: lsw + sw,
                    full,
                };
                continue;
            }
        }
        out.push(*s);
    }
    out
}

/// Build the simplest entity for a run of connected segments.
pub fn entity_from_segs(segs: &[Seg], template: &Entity, closed: bool) -> Option<Entity> {
    let segs = merge_segs(segs);
    if segs.is_empty() {
        return None;
    }
    let e = if segs.len() == 1 && !closed {
        match segs[0] {
            Seg::L { a, b, .. } => Entity::line(a, b),
            s => Entity::arc_seg(&s),
        }
    } else {
        Entity::pline(segs_to_verts(&segs, closed), closed)
    };
    Some(inherit_props(template, e))
}

/// Is an entity of a type that TRIM/EXTEND/BREAK/JOIN can edit as a path?
pub fn is_path_entity(e: &Entity) -> bool {
    matches!(
        e.kind,
        Kind::Line { .. } | Kind::Arc { .. } | Kind::Circle { .. } | Kind::Pline { .. }
    )
}

/// Cutting/boundary geometry of any entity.
pub fn edge_segs(e: &Entity, doc: &Drawing) -> Vec<Seg> {
    match e.kind {
        Kind::Line { .. }
        | Kind::Arc { .. }
        | Kind::Circle { .. }
        | Kind::Pline { .. }
        | Kind::Ellipse(_)
        | Kind::Spline { .. }
        | Kind::Xline { .. }
        | Kind::Ray { .. }
        | Kind::Leader { .. } => segs_of(e, doc).to_vec(),
        _ => vec![],
    }
}
