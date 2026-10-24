//! The spatial index must not change any answer: picking, selecting and snapping on a large drawing give exactly what the
//! plain linear scan gives. The reference is the same engine code on a drawing small enough to skip the index (it holds
//! only the entities that can matter for the probe, in the same order).
use cadforge_core::drawing::Drawing;
use cadforge_core::entities::*;
use cadforge_core::geom::*;
use cadforge_engine::parse::{OS_CEN, OS_END, OS_INT, OS_MID};
use cadforge_engine::select::*;
use cadforge_engine::snap::*;

struct Rng(u64);
impl Rng {
    fn next(&mut self) -> f64 {
        self.0 = self
            .0
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        ((self.0 >> 33) as f64) / (1u64 << 31) as f64
    }
}

/// ~3000 entities of every sort that matters: lines, circles, arcs, closed polylines, text, hatches, points, infinite lines,
/// objects on a locked, a frozen and a switched-off layer.
fn mixed(n: usize) -> Drawing {
    let mut d = Drawing::new("mixed");
    for l in ["Locked", "Frozen", "Off"] {
        d.add_layer(l).unwrap();
    }
    d.layer_mut("Locked").unwrap().locked = true;
    d.layer_mut("Frozen").unwrap().frozen = true;
    d.layer_mut("Off").unwrap().on = false;
    let mut r = Rng(777);
    for i in 0..n {
        let (x, y) = (r.next() * 2000.0, r.next() * 1200.0);
        let mut e = match i % 9 {
            0 => Entity::line(
                pt(x, y),
                pt(x + r.next() * 60.0 - 30.0, y + r.next() * 60.0 - 30.0),
            ),
            1 => Entity::circle(pt(x, y), 2.0 + r.next() * 25.0),
            2 => Entity::arc(
                pt(x, y),
                5.0 + r.next() * 30.0,
                r.next() * 6.0,
                r.next() * 6.0,
            ),
            3 => Entity::pline(
                vec![
                    Vertex::new(x, y, 0.3),
                    Vertex::new(x + 30.0, y, 0.0),
                    Vertex::new(x + 30.0, y + 20.0, -0.4),
                    Vertex::new(x, y + 20.0, 0.0),
                ],
                true,
            ),
            4 => Entity::text(pt(x, y), 3.0, "Label"),
            5 => Entity::hatch(
                vec![vec![
                    Vertex::new(x, y, 0.0),
                    Vertex::new(x + 20.0, y, 0.0),
                    Vertex::new(x + 20.0, y + 15.0, 0.0),
                    Vertex::new(x, y + 15.0, 0.0),
                ]],
                "SOLID",
            ),
            6 => Entity::point(pt(x, y)),
            7 => Entity::line(pt(x, y), pt(x + 400.0, y + 3.0)), // long and thin: crosses many grid cells
            _ => Entity::circle(pt(x, y), 1.0),
        };
        if i % 41 == 0 {
            e.layer = ["Locked", "Frozen", "Off"][i / 41 % 3].to_string();
        }
        d.add(e);
    }
    d.add(Entity::xline(pt(1000.0, 600.0), pt(1.0, 0.3)));
    d.add(Entity::ray(pt(300.0, 300.0), pt(0.2, 1.0)));
    d
}

/// A drawing small enough for the linear path holding the entities of `big` whose bounds meet `reach` (and the infinite ones), in order.
fn reduced(big: &Drawing, reach: &Bounds) -> Drawing {
    let mut small = Drawing::new("small");
    small.layers = big.layers.clone();
    for e in big.entities() {
        let keep = matches!(e.kind, Kind::Xline { .. } | Kind::Ray { .. })
            || bounds_of(e, big).map_or(true, |b| b.overlaps(reach));
        if keep {
            small.add(e.clone());
        }
    }
    assert!(
        small.entities().len() < cadforge_core::spatial::MIN_ENTITIES,
        "the reference must use the linear path ({} entities)",
        small.entities().len()
    );
    small
}

fn grow(b: &Bounds, by: f64) -> Bounds {
    Bounds::new(b.min_x - by, b.min_y - by, b.max_x + by, b.max_y + by)
}

#[test]
fn picking_gives_the_same_answers_with_and_without_the_index() {
    let big = mixed(3000);
    assert!(big.entities().len() >= cadforge_core::spatial::MIN_ENTITIES);
    let mut r = Rng(5);
    let mut hits = 0;
    for _ in 0..300 {
        let p = pt(r.next() * 2000.0, r.next() * 1200.0);
        let ap = 3.0 + r.next() * 12.0;
        let small = reduced(&big, &grow(&Bounds::new(p.x, p.y, p.x, p.y), ap));
        for locked_ok in [false, true] {
            let a = pick_at_locked(&big, p, ap, None, None, locked_ok);
            let b = pick_at_locked(&small, p, ap, None, None, locked_ok);
            assert_eq!(a, b, "pick at {p:?} ap {ap} locked_ok {locked_ok}");
            hits += a.is_some() as usize;
        }
        assert_eq!(
            pick_all(&big, p, ap, None),
            pick_all(&small, p, ap, None),
            "pick_all at {p:?}"
        );
    }
    assert!(
        hits > 100,
        "the probes hit things often enough to mean something ({hits})"
    );
}

#[test]
fn window_crossing_polygon_and_fence_selection_are_unchanged() {
    let big = mixed(3000);
    let mut r = Rng(11);
    let mut selected = 0;
    for _ in 0..120 {
        let (x, y) = (r.next() * 1900.0, r.next() * 1100.0);
        let rect = rect_of(
            pt(x, y),
            pt(x + 20.0 + r.next() * 250.0, y + 20.0 + r.next() * 180.0),
        );
        let small = reduced(&big, &rect);
        for crossing in [false, true] {
            let a = select_rect(&big, &rect, crossing, None);
            let b = select_rect(&small, &rect, crossing, None);
            assert_eq!(a, b, "rect {rect:?} crossing {crossing}");
            selected += a.len();
        }
        let poly = vec![
            pt(rect.min_x, rect.min_y),
            pt(rect.max_x, rect.min_y + 10.0),
            pt(rect.max_x - 15.0, rect.max_y),
            pt(rect.min_x + 5.0, rect.max_y - 8.0),
        ];
        let reach = Bounds::of_points(&poly);
        let small = reduced(&big, &reach);
        for crossing in [false, true] {
            assert_eq!(
                select_polygon(&big, &poly, crossing, None),
                select_polygon(&small, &poly, crossing, None),
                "polygon {poly:?} crossing {crossing}"
            );
        }
        let fence = vec![
            pt(rect.min_x, rect.min_y),
            pt(rect.max_x, rect.max_y),
            pt(rect.max_x, rect.min_y),
        ];
        let small = reduced(&big, &Bounds::of_points(&fence));
        assert_eq!(
            select_fence(&big, &fence, None),
            select_fence(&small, &fence, None),
            "fence {fence:?}"
        );
    }
    assert!(selected > 200, "the windows selected things ({selected})");
}

#[test]
fn object_snap_is_unchanged() {
    let big = mixed(3000);
    let snapper = Snapper::new();
    let mut o = SnapOptions::new(1.0);
    o.osmask = OS_END | OS_MID | OS_CEN | OS_INT;
    let mut r = Rng(23);
    let mut found = 0;
    for _ in 0..200 {
        let p = pt(r.next() * 2000.0, r.next() * 1200.0);
        let ap = 6.0 + r.next() * 10.0;
        let small = reduced(&big, &grow(&Bounds::new(p.x, p.y, p.x, p.y), ap));
        let a = snapper
            .object_snap(&big, p, ap, &o)
            .map(|h| (h.pt.x, h.pt.y, h.kind.code()));
        let b = snapper
            .object_snap(&small, p, ap, &o)
            .map(|h| (h.pt.x, h.pt.y, h.kind.code()));
        assert_eq!(a, b, "snap at {p:?} ap {ap}");
        found += a.is_some() as usize;
    }
    assert!(found > 50, "snaps were found ({found})");
}

#[test]
fn the_index_follows_every_edit() {
    let mut big = mixed(3000);
    let probe = pt(1500.0, 5000.0); // empty space above the scatter
    assert_eq!(pick_at(&big, probe, 5.0, None, None), None);
    let id = big.add(Entity::circle(probe, 2.0));
    assert_eq!(
        pick_at(&big, probe, 5.0, None, None),
        Some(id),
        "a new entity is found at once"
    );
    big.modify(id, |e| {
        e.kind = Kind::Circle {
            c: pt(100.0, 5000.0),
            r: 2.0,
        };
    });
    assert_eq!(
        pick_at(&big, probe, 5.0, None, None),
        None,
        "a moved entity is gone from its old place"
    );
    assert_eq!(
        pick_at(&big, pt(100.0, 5000.0), 5.0, None, None),
        Some(id),
        "and found at the new one"
    );
    big.remove(id);
    assert_eq!(
        pick_at(&big, pt(100.0, 5000.0), 5.0, None, None),
        None,
        "an erased entity is not found"
    );
    // layer state is checked on the candidates, not baked into the index
    let on_layer = big.add(Entity::circle(probe, 2.0).on_layer("Frozen"));
    assert_eq!(
        pick_at(&big, probe, 5.0, None, None),
        None,
        "a frozen layer's object cannot be picked"
    );
    big.layer_mut("Frozen").unwrap().frozen = false;
    big.touch();
    assert_eq!(
        pick_at(&big, probe, 5.0, None, None),
        Some(on_layer),
        "thawing makes it pickable again"
    );
}

#[test]
fn layouts_have_their_own_index() {
    let mut d = Drawing::new("two spaces");
    for i in 0..1500 {
        d.add(Entity::circle(
            pt((i % 50) as f64 * 10.0, (i / 50) as f64 * 10.0),
            2.0,
        ));
    }
    d.add_layout("Sheet").unwrap();
    d.set_space("Sheet").unwrap();
    d.layouts
        .iter_mut()
        .find(|l| l.name == "Sheet")
        .unwrap()
        .entities
        .clear();
    for i in 0..1000 {
        d.add(Entity::line(
            pt((i % 40) as f64 * 5.0, (i / 40) as f64 * 5.0),
            pt((i % 40) as f64 * 5.0 + 2.0, (i / 40) as f64 * 5.0),
        ));
    }
    let on_sheet = pick_at(&d, pt(1.0, 0.0), 1.0, None, None).expect("a line on the sheet");
    assert!(matches!(d.by_id(on_sheet).unwrap().kind, Kind::Line { .. }));
    d.set_space("model").unwrap();
    let in_model = pick_at(&d, pt(2.0, 0.0), 1.0, None, None).expect("a circle in model space");
    assert!(matches!(
        d.by_id(in_model).unwrap().kind,
        Kind::Circle { .. }
    ));
}
