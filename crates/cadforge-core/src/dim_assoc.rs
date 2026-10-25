//! Associative dimensions. A dimension's defining points can be tied to snap points of the objects they measure
//! (`DimData::assoc`): when a command moves, stretches or resizes those objects, the dimension follows at the end of
//! the command. With DIMASSOC 2 a dimension made in a command is tied to whatever object has a snap point (an end,
//! centre, node, insertion, midpoint or quadrant point) exactly at its defining points; radius and diameter
//! dimensions follow their circle or arc. Erasing an object drops the links to it and leaves the dimension as it is.
use crate::dim::{DimData, DimRef, DimType};
use crate::drawing::{Drawing, MODEL};
use crate::entities::{snap_points_of, Entity, Kind, SnapType};
use crate::geom::*;
use std::collections::HashMap;

/// The defining points that can be tied to geometry, by dimension type.
pub fn keys(kind: DimType) -> &'static [&'static str] {
    match kind {
        DimType::Lin | DimType::Ali => &["p1", "p2"],
        DimType::Ang => &["p1", "p2", "c"],
        DimType::Arc => &["c", "p1", "p2"],
        DimType::Ord => &["pt"],
        DimType::Rad | DimType::Dia | DimType::Jog => &["c", "pt"],
    }
}

fn point(d: &DimData, key: &str) -> Option<Pt> {
    match key {
        "p1" => d.p1,
        "p2" => d.p2,
        "c" => d.c,
        "pt" => d.pt,
        _ => None,
    }
}

fn near(a: Pt, b: Pt) -> bool {
    dist(a, b) <= 1e-6 * (1.0 + a.x.abs().max(a.y.abs()))
}

/// Objects a dimension can be tied to.
fn measurable(e: &Entity) -> bool {
    matches!(e.kind, Kind::Line { .. } | Kind::Circle { .. } | Kind::Arc { .. } | Kind::Pline { .. } | Kind::Ellipse(_) | Kind::Spline { .. } | Kind::Point { .. } | Kind::Insert(_))
}

/// Snap kinds in the order a defining point prefers them.
const PREFERRED: [SnapType; 6] = [SnapType::End, SnapType::Nod, SnapType::Cen, SnapType::Ins, SnapType::Mid, SnapType::Qua];

impl Drawing {
    /// The space (`model` or a layout name) that holds the entity.
    fn space_of(&self, id: u64) -> Option<String> {
        if self.model.entities.iter().any(|e| e.id == id) {
            return Some(MODEL.to_string());
        }
        self.layouts.iter().find(|l| l.entities.iter().any(|e| e.id == id)).map(|l| l.name.clone())
    }

    /// The links a dimension gets from the objects of its space that have a snap point at its defining points.
    pub fn find_dim_refs(&self, id: u64, d: &DimData) -> Vec<DimRef> {
        let space = self.space_of(id).unwrap_or_else(|| MODEL.to_string());
        let ents = self.space_entities(&space);
        let mut out = vec![];
        let radial = matches!(d.kind, DimType::Rad | DimType::Dia | DimType::Jog);
        for key in keys(d.kind) {
            let Some(q) = point(d, key) else { continue };
            let ap = 1e-6 * (1.0 + q.x.abs().max(q.y.abs()));
            let win = Bounds::new(q.x - ap, q.y - ap, q.x + ap, q.y + ap);
            let cands: Vec<&Entity> = self.candidates_in(&space, &win).into_iter().map(|i| &ents[i]).filter(|e| e.id != id && measurable(e)).collect();
            if radial && *key == "pt" {
                // the point on the circle: tied to the circle or arc whose centre the dimension measures from
                let Some(c) = d.c else { continue };
                let on = self.space_entities(&space).iter().find(|e| match e.kind {
                    Kind::Circle { c: cc, r } | Kind::Arc { c: cc, r, .. } => near(cc, c) && (dist(q, c) - r).abs() <= 1e-6 * (1.0 + r),
                    _ => false,
                });
                if let Some(e) = on {
                    out.push(DimRef { key: key.to_string(), ent: e.id, i: 0, kind: None, radial: true });
                }
                continue;
            }
            let mut best: Option<(usize, DimRef)> = None;
            for e in cands {
                for (i, s) in snap_points_of(e, self).iter().enumerate() {
                    if !near(s.pt, q) {
                        continue;
                    }
                    let rank = PREFERRED.iter().position(|k| *k == s.kind).unwrap_or(PREFERRED.len());
                    if best.as_ref().is_none_or(|(r, _)| rank < *r) {
                        best = Some((rank, DimRef { key: key.to_string(), ent: e.id, i, kind: Some(s.kind), radial: false }));
                    }
                }
            }
            if let Some((_, r)) = best {
                out.push(r);
            }
        }
        out
    }

    /// Run at the end of every command that changed the drawing: tie dimensions made in it (ids from `new_from`,
    /// when DIMASSOC is 2) and move associative dimensions after their geometry. Returns how many dimensions changed.
    pub fn update_dim_associations(&mut self, new_from: u64) -> usize {
        let plans = self.dim_association_plans(new_from);
        if plans.is_empty() {
            return 0;
        }
        self.will_change();
        let n = plans.len();
        for (id, nd) in plans {
            if let Some(e) = self.by_id_mut(id) {
                e.kind = Kind::Dim(nd);
            }
        }
        self.touch();
        n
    }

    /// The dimensions to change and their new data (see [`Drawing::update_dim_associations`]).
    fn dim_association_plans(&self, new_from: u64) -> Vec<(u64, DimData)> {
        let spaces: Vec<&Vec<Entity>> = std::iter::once(&self.model.entities).chain(self.layouts.iter().map(|l| &l.entities)).collect();
        let dims: Vec<(u64, DimData)> = spaces.iter().flat_map(|v| v.iter()).filter_map(|e| if let Kind::Dim(d) = &e.kind { Some((e.id, d.clone())) } else { None }).collect();
        let mut plans: Vec<(u64, DimData)> = vec![];
        if dims.is_empty() {
            return plans;
        }
        let tie_new = self.settings.dimassoc == 2;
        let by_id: HashMap<u64, &Entity> = spaces.iter().flat_map(|v| v.iter()).map(|e| (e.id, e)).collect();
        for (id, d) in dims {
            if d.assoc.is_empty() {
                if tie_new && id >= new_from {
                    let refs = self.find_dim_refs(id, &d);
                    if !refs.is_empty() {
                        plans.push((id, DimData { assoc: refs, ..d }));
                    }
                }
                continue;
            }
            let mut nd = d.clone();
            let mut moves: Vec<Pt> = vec![];
            nd.assoc.clear();
            for r in &d.assoc {
                let Some(g) = by_id.get(&r.ent) else { continue }; // erased: the link goes
                let to = if r.radial {
                    match g.kind {
                        Kind::Circle { c, r: rad } | Kind::Arc { c, r: rad, .. } => {
                            let (oc, op) = (d.c.unwrap_or(c), d.pt.unwrap_or(c));
                            let u = if dist(op, oc) > 1e-12 { unit(op - oc) } else { pt(1.0, 0.0) };
                            Some(c + u * rad)
                        }
                        _ => None,
                    }
                } else {
                    snap_points_of(g, self).get(r.i).filter(|s| Some(s.kind) == r.kind).map(|s| s.pt)
                };
                let Some(to) = to else { continue };
                if let (Some(from), Some(slot)) = (point(&d, &r.key), nd.point_mut(&r.key)) {
                    if !r.radial {
                        moves.push(to - from);
                    }
                    *slot = Some(to);
                }
                nd.assoc.push(r.clone());
            }
            // the measured points all moved the same way: the dimension line and text go along
            if let Some(v) = moves.first().copied() {
                let all = moves.len() == keys(d.kind).iter().filter(|k| point(&d, k).is_some() && !(**k == "pt" && matches!(d.kind, DimType::Rad | DimType::Dia | DimType::Jog))).count();
                if all && v.x.hypot(v.y) > 1e-12 && moves.iter().all(|m| near(*m, v)) {
                    nd.dp = nd.dp.map(|p| p + v);
                    nd.tp = nd.tp.map(|p| p + v);
                    if d.kind == DimType::Jog {
                        nd.p1 = nd.p1.map(|p| p + v);
                        nd.p2 = nd.p2.map(|p| p + v);
                    }
                }
            }
            if nd != d {
                plans.push((id, nd));
            }
        }
        plans
    }
}
