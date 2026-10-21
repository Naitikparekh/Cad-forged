//! Picking and rectangle/polygon/fence selection (port of `src/engine/select.js`).
use cadforge_core::drawing::Drawing;
use cadforge_core::entities::*;
use cadforge_core::geom::*;
use std::collections::HashSet;

/// May the entity be selected (visible, unlocked layer)?
pub fn selectable(doc: &Drawing, e: &Entity) -> bool {
    doc.is_visible(e) && !doc.is_locked(e)
}

pub type Filter<'a> = Option<&'a dyn Fn(&Entity) -> bool>;

fn passes(f: Filter, e: &Entity) -> bool {
    f.map_or(true, |f| f(e))
}

fn is_infinite(e: &Entity) -> bool {
    matches!(e.kind, Kind::Xline { .. } | Kind::Ray { .. })
}

/// Topmost entity within `ap` world units of `p`. Nearest wins; ties go to the later (upper) entity.
/// Filled hatches yield to line work.
pub fn pick_at(doc: &Drawing, p: Pt, ap: f64, filter: Filter, exclude: Option<&HashSet<u64>>) -> Option<u64> {
    let mut best: Option<u64> = None;
    let mut best_score = f64::INFINITY;
    let win = Bounds::new(p.x - ap, p.y - ap, p.x + ap, p.y + ap);
    for e in doc.entities() {
        if !selectable(doc, e) || !passes(filter, e) || exclude.is_some_and(|x| x.contains(&e.id)) {
            continue;
        }
        if !is_infinite(e) && !bounds_of(e, doc).is_some_and(|b| b.overlaps(&win)) {
            continue;
        }
        let mut d = pick_distance(e, p, doc);
        if d > ap {
            continue;
        }
        if matches!(e.kind, Kind::Hatch(_)) {
            d += ap * 0.9;
        }
        if d <= best_score + 1e-12 {
            best = Some(e.id);
            best_score = d;
        }
    }
    best
}

/// All entities under `p`, nearest first (for selection cycling).
pub fn pick_all(doc: &Drawing, p: Pt, ap: f64, filter: Filter) -> Vec<u64> {
    let mut list: Vec<(u64, f64)> = vec![];
    let win = Bounds::new(p.x - ap, p.y - ap, p.x + ap, p.y + ap);
    for e in doc.entities() {
        if !selectable(doc, e) || !passes(filter, e) {
            continue;
        }
        if !is_infinite(e) && !bounds_of(e, doc).is_some_and(|b| b.overlaps(&win)) {
            continue;
        }
        let d = pick_distance(e, p, doc);
        if d <= ap {
            list.push((e.id, d));
        }
    }
    list.sort_by(|a, b| a.1.total_cmp(&b.1));
    list.into_iter().map(|x| x.0).collect()
}

pub fn rect_of(a: Pt, b: Pt) -> Bounds {
    Bounds::new(a.x.min(b.x), a.y.min(b.y), a.x.max(b.x), a.y.max(b.y))
}

/// Window (fully inside) or crossing (touching) selection.
pub fn select_rect(doc: &Drawing, rect: &Bounds, crossing: bool, filter: Filter) -> Vec<u64> {
    doc.entities().iter().filter(|e| selectable(doc, e) && passes(filter, e) && if crossing { crosses_rect(e, rect, doc) } else { in_window(e, rect, doc) }).map(|e| e.id).collect()
}

fn sample_points(e: &Entity, doc: &Drawing) -> Vec<Pt> {
    let mut pts = vec![];
    for s in segs_of(e, doc).iter() {
        pts.extend(tess_seg(s, 0.05));
    }
    for poly in text_polygons(e, doc) {
        pts.extend(poly);
    }
    match &e.kind {
        Kind::Point { p } => pts.push(*p),
        Kind::Hatch(h) => {
            for l in &h.loops {
                pts.extend(tess_segs(&pline_segs(l, true), 0.05));
            }
        }
        _ => {}
    }
    pts
}

fn poly_edges(poly: &[Pt], closed: bool) -> Vec<Seg> {
    let n = poly.len();
    let take = if closed { n } else { n.saturating_sub(1) };
    (0..take).map(|i| Seg::line(poly[i], poly[(i + 1) % n])).collect()
}

fn crosses_path(e: &Entity, doc: &Drawing, edges: &[Seg]) -> bool {
    for s in segs_of(e, doc).iter() {
        for q in edges {
            if !seg_intersect(s, q, false, false).is_empty() {
                return true;
            }
        }
    }
    for tp in text_polygons(e, doc) {
        for i in 0..tp.len() {
            let s = Seg::line(tp[i], tp[(i + 1) % tp.len()]);
            for q in edges {
                if !seg_intersect(&s, q, false, false).is_empty() {
                    return true;
                }
            }
        }
    }
    false
}

/// Window polygon (all inside) or crossing polygon (any touch).
pub fn select_polygon(doc: &Drawing, poly: &[Pt], crossing: bool, filter: Filter) -> Vec<u64> {
    let edges = poly_edges(poly, true);
    let mut out = vec![];
    for e in doc.entities() {
        if !selectable(doc, e) || !passes(filter, e) {
            continue;
        }
        let pts = sample_points(e, doc);
        if pts.is_empty() {
            continue;
        }
        let all_in = pts.iter().all(|p| point_in_polygon(*p, poly));
        let any_in = pts.iter().any(|p| point_in_polygon(*p, poly));
        let crosses = crosses_path(e, doc, &edges);
        if if crossing { any_in || crosses } else { all_in && !crosses } {
            out.push(e.id);
        }
    }
    out
}

/// Fence: entities crossed by an open polyline.
pub fn select_fence(doc: &Drawing, pts: &[Pt], filter: Filter) -> Vec<u64> {
    let edges = poly_edges(pts, false);
    doc.entities().iter().filter(|e| selectable(doc, e) && passes(filter, e) && crosses_path(e, doc, &edges)).map(|e| e.id).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn doc() -> Drawing {
        let mut d = Drawing::new("t");
        d.add(Entity::line(pt(0.0, 0.0), pt(10.0, 0.0))); // 1
        d.add(Entity::circle(pt(20.0, 0.0), 3.0)); // 2
        d.add(Entity::hatch(vec![vec![Vertex::new(0.0, 0.0, 0.0), Vertex::new(10.0, 0.0, 0.0), Vertex::new(10.0, 10.0, 0.0), Vertex::new(0.0, 10.0, 0.0)]], "SOLID")); // 3
        d.add(Entity::text(pt(0.0, 20.0), 2.0, "HELLO")); // 4
        d
    }

    #[test]
    fn picking_prefers_line_work_over_fills() {
        let d = doc();
        assert_eq!(pick_at(&d, pt(5.0, 0.1), 0.5, None, None), Some(1), "a line beats the hatch under it");
        assert_eq!(pick_at(&d, pt(5.0, 5.0), 0.5, None, None), Some(3), "inside the fill picks the hatch");
        assert_eq!(pick_at(&d, pt(50.0, 50.0), 0.5, None, None), None);
        let f = |e: &Entity| matches!(e.kind, Kind::Circle { .. });
        assert_eq!(pick_at(&d, pt(5.0, 0.1), 0.5, Some(&f), None), None);
        let ex: HashSet<u64> = [1].into();
        assert_eq!(pick_at(&d, pt(5.0, 0.1), 0.5, None, Some(&ex)), Some(3), "excluded ids are skipped");
        assert_eq!(pick_all(&d, pt(5.0, 0.1), 0.5, None), vec![3, 1], "cycling lists nearest first (the fill is at distance 0)");
    }

    #[test]
    fn window_crossing_polygon_and_fence() {
        let d = doc();
        assert_eq!(select_rect(&d, &rect_of(pt(-1.0, -1.0), pt(11.0, 11.0)), false, None), vec![1, 3]);
        assert_eq!(select_rect(&d, &rect_of(pt(-1.0, -1.0), pt(5.0, 5.0)), false, None), Vec::<u64>::new());
        assert_eq!(select_rect(&d, &rect_of(pt(-1.0, -1.0), pt(5.0, 5.0)), true, None), vec![1, 3]);
        let tri = [pt(-1.0, -5.0), pt(30.0, -5.0), pt(30.0, 12.0), pt(-1.0, 12.0)];
        assert_eq!(select_polygon(&d, &tri, false, None), vec![1, 2, 3]);
        assert_eq!(select_fence(&d, &[pt(5.0, -5.0), pt(5.0, 5.0)], None), vec![1]);
    }

    #[test]
    fn locked_and_hidden_layers_are_not_selectable() {
        let mut d = doc();
        d.add_layer("L").unwrap();
        d.cur.layer = "L".into();
        let id = d.add(Entity::circle(pt(40.0, 0.0), 1.0));
        assert_eq!(pick_at(&d, pt(41.0, 0.0), 0.5, None, None), Some(id));
        d.layer_mut("L").unwrap().locked = true;
        assert_eq!(pick_at(&d, pt(41.0, 0.0), 0.5, None, None), None);
        d.layer_mut("L").unwrap().locked = false;
        d.layer_mut("L").unwrap().on = false;
        assert_eq!(pick_at(&d, pt(41.0, 0.0), 0.5, None, None), None);
    }
}
