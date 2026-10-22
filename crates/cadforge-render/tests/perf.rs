//! Performance benchmark on a large drawing (part of the AutoCAD parity benchmark, see docs/BENCHMARK.md).
use cadforge_core::drawing::Drawing;
use cadforge_core::entities::*;
use cadforge_core::geom::*;
use cadforge_core::io::{read_dxf, write_dxf};
use cadforge_core::view::View;
use cadforge_render::*;
use std::time::Instant;

fn big_drawing(n: usize) -> Drawing {
    let mut d = Drawing::new("big");
    let mut seed = 12345u64;
    let mut rnd = || {
        seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        ((seed >> 33) as f64) / (1u64 << 31) as f64
    };
    for i in 0..n {
        let (x, y) = (rnd() * 20000.0, rnd() * 12000.0);
        let e = match i % 5 {
            0 => Entity::line(pt(x, y), pt(x + rnd() * 80.0 - 40.0, y + rnd() * 80.0 - 40.0)),
            1 => Entity::circle(pt(x, y), 2.0 + rnd() * 30.0),
            2 => Entity::arc(pt(x, y), 5.0 + rnd() * 40.0, rnd() * 6.0, rnd() * 6.0),
            3 => Entity::pline(vec![Vertex::new(x, y, 0.3), Vertex::new(x + 30.0, y, 0.0), Vertex::new(x + 30.0, y + 20.0, -0.4), Vertex::new(x, y + 20.0, 0.0)], true),
            _ => Entity::text(pt(x, y), 3.0, "Label 1234"),
        };
        d.add(e);
    }
    d
}

fn full_view(d: &Drawing) -> View {
    let mut v = View::new();
    v.w = 1600.0;
    v.h = 900.0;
    let b = d.model.entities.iter().fold(None, |a, e| Bounds::union_opt(a, cadforge_core::entities::bounds_of(e, d)));
    v.fit(b, 0.02);
    v
}

#[test]
fn twenty_thousand_entities_render_pick_and_round_trip_quickly() {
    let d = big_drawing(20_000);
    let v = full_view(&d);
    // first frame builds the segment/bounds caches, the second is the steady state
    let t0 = Instant::now();
    let ops = render_scene(&d, &v, v.w, v.h, Theme::DARK, Prefs::default(), &Highlight::default(), None);
    let first = t0.elapsed();
    let t1 = Instant::now();
    let ops2 = render_scene(&d, &v, v.w, v.h, Theme::DARK, Prefs::default(), &Highlight::default(), None);
    let steady = t1.elapsed();
    assert_eq!(ops.len(), ops2.len());
    assert!(ops.len() > 10_000);
    // zoomed in on a corner only a few entities are drawn, culling keeps the frame cheap
    let mut z = v.clone();
    z.scale *= 40.0;
    let t2 = Instant::now();
    let zoomed = render_scene(&d, &z, z.w, z.h, Theme::DARK, Prefs::default(), &Highlight::default(), None);
    let zoom_t = t2.elapsed();
    assert!(zoomed.len() < ops.len() / 5);
    // picking
    let t3 = Instant::now();
    let mut hits = 0;
    for i in 0..200 {
        let p = pt(100.0 + i as f64 * 90.0, 6000.0);
        if cadforge_engine_pick(&d, p).is_some() {
            hits += 1;
        }
    }
    let pick = t3.elapsed() / 200;
    // file round trip
    let t4 = Instant::now();
    let dxf = write_dxf(&d);
    let write_t = t4.elapsed();
    let t5 = Instant::now();
    let back = read_dxf(&dxf, "big").unwrap();
    let read_t = t5.elapsed();
    assert_eq!(back.model.entities.len(), d.model.entities.len());
    let json = d.to_json_string();
    let t6 = Instant::now();
    let jb = Drawing::from_json_str(&json, None).unwrap();
    let json_t = t6.elapsed();
    assert_eq!(jb.model.entities.len(), 20_000);
    println!("PERF 20k entities: first frame {first:?}, steady frame {steady:?}, zoomed frame {zoom_t:?}, pick {pick:?} ({hits} hits), DXF write {write_t:?}, DXF read {read_t:?}, JSON load {json_t:?}");
    // generous ceilings (CI machines vary); AutoCAD regenerates drawings of this size in well under a second
    assert!(steady.as_secs_f64() < 1.5, "steady frame {steady:?}");
    assert!(first.as_secs_f64() < 4.0, "first frame {first:?}");
    assert!(pick.as_secs_f64() < 0.05, "pick {pick:?}");
    assert!(write_t.as_secs_f64() < 5.0 && read_t.as_secs_f64() < 5.0, "dxf {write_t:?} {read_t:?}");
}

/// Nearest entity under a point, brute force like the selection code (linear scan with bounds rejection).
fn cadforge_engine_pick(d: &Drawing, p: Pt) -> Option<u64> {
    let ap = 20.0;
    let win = Bounds::new(p.x - ap, p.y - ap, p.x + ap, p.y + ap);
    let mut best: Option<(u64, f64)> = None;
    for e in &d.model.entities {
        if !cadforge_core::entities::bounds_of(e, d).is_some_and(|b| b.overlaps(&win)) {
            continue;
        }
        let dist = cadforge_core::entities::pick_distance(e, p, d);
        if dist <= ap && best.map_or(true, |(_, bd)| dist < bd) {
            best = Some((e.id, dist));
        }
    }
    best.map(|b| b.0)
}

#[test]
fn a_hundred_thousand_entities_still_redraw_interactively() {
    let d = big_drawing(100_000);
    let v = full_view(&d);
    let _ = render_scene(&d, &v, v.w, v.h, Theme::DARK, Prefs::default(), &Highlight::default(), None);
    let t = Instant::now();
    let ops = render_scene(&d, &v, v.w, v.h, Theme::DARK, Prefs::default(), &Highlight::default(), None);
    let steady = t.elapsed();
    println!("PERF 100k entities: steady frame {steady:?} ({} ops)", ops.len());
    assert!(steady.as_secs_f64() < 3.0, "steady frame {steady:?}");
}

#[test]
fn a_zoomed_in_frame_of_a_hundred_thousand_entities_only_touches_what_is_on_screen() {
    let d = big_drawing(100_000);
    let v = full_view(&d);
    let full = render_scene(&d, &v, v.w, v.h, Theme::DARK, Prefs::default(), &Highlight::default(), None);
    // zoom in on a small part: roughly 1/1600 of the drawing is visible
    let mut z = v.clone();
    z.scale *= 40.0;
    let _ = render_scene(&d, &z, z.w, z.h, Theme::DARK, Prefs::default(), &Highlight::default(), None);
    let t = Instant::now();
    let n = 20;
    let mut ops = vec![];
    for _ in 0..n {
        ops = render_scene(&d, &z, z.w, z.h, Theme::DARK, Prefs::default(), &Highlight::default(), None);
    }
    let per_frame = t.elapsed() / n;
    println!("PERF 100k zoomed frame: {per_frame:?} ({} ops, full view {} ops)", ops.len(), full.len());
    assert!(ops.len() < full.len() / 50, "only a small part is drawn");
    assert!(per_frame.as_secs_f64() < 0.060, "zoomed frame {per_frame:?}");
}
