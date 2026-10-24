//! Selection and snapping on very large drawings (part of the AutoCAD parity benchmark, see docs/BENCHMARK.md).
use cadforge_core::drawing::Drawing;
use cadforge_core::entities::*;
use cadforge_core::geom::*;
use cadforge_engine::parse::{OS_CEN, OS_END, OS_INT, OS_MID};
use cadforge_engine::select::*;
use cadforge_engine::snap::*;
use std::time::{Duration, Instant};

fn big_drawing(n: usize) -> Drawing {
    let mut d = Drawing::new("big");
    let mut seed = 12345u64;
    let mut rnd = || {
        seed = seed
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        ((seed >> 33) as f64) / (1u64 << 31) as f64
    };
    for i in 0..n {
        let (x, y) = (rnd() * 20000.0, rnd() * 12000.0);
        let e = match i % 5 {
            0 => Entity::line(
                pt(x, y),
                pt(x + rnd() * 80.0 - 40.0, y + rnd() * 80.0 - 40.0),
            ),
            1 => Entity::circle(pt(x, y), 2.0 + rnd() * 30.0),
            2 => Entity::arc(pt(x, y), 5.0 + rnd() * 40.0, rnd() * 6.0, rnd() * 6.0),
            3 => Entity::pline(
                vec![
                    Vertex::new(x, y, 0.3),
                    Vertex::new(x + 30.0, y, 0.0),
                    Vertex::new(x + 30.0, y + 20.0, -0.4),
                    Vertex::new(x, y + 20.0, 0.0),
                ],
                true,
            ),
            _ => Entity::text(pt(x, y), 3.0, "Label 1234"),
        };
        d.add(e);
    }
    d
}

fn avg(f: impl Fn(usize), n: usize) -> Duration {
    let t = Instant::now();
    for i in 0..n {
        f(i);
    }
    t.elapsed() / n as u32
}

#[test]
fn picking_windowing_and_snapping_stay_fast_on_a_hundred_thousand_entities() {
    let d = big_drawing(100_000);
    // the first operation after a change builds the bounds (and the index); measure it separately
    let t0 = Instant::now();
    let _ = pick_at(&d, pt(10_000.0, 6_000.0), 5.0, None, None);
    let first = t0.elapsed();

    let hits = std::cell::Cell::new(0);
    let pick = avg(
        |i| {
            if pick_at(
                &d,
                pt(100.0 + i as f64 * 97.0, 500.0 + (i % 7) as f64 * 1500.0),
                8.0,
                None,
                None,
            )
            .is_some()
            {
                hits.set(hits.get() + 1)
            }
        },
        200,
    );
    let all = avg(
        |i| {
            let _ = pick_all(&d, pt(100.0 + i as f64 * 97.0, 3000.0), 8.0, None);
        },
        100,
    );
    let win = avg(
        |i| {
            let x = 500.0 + i as f64 * 700.0;
            let _ = select_rect(
                &d,
                &rect_of(pt(x, 2000.0), pt(x + 300.0, 2200.0)),
                false,
                None,
            );
        },
        20,
    );
    let cross = avg(
        |i| {
            let x = 500.0 + i as f64 * 700.0;
            let _ = select_rect(
                &d,
                &rect_of(pt(x, 2000.0), pt(x + 300.0, 2200.0)),
                true,
                None,
            );
        },
        20,
    );
    let snapper = Snapper::new();
    let mut opts = SnapOptions::new(1.0);
    opts.osmask = OS_END | OS_MID | OS_CEN | OS_INT;
    let snap = avg(
        |i| {
            let _ = snapper.object_snap(&d, pt(100.0 + i as f64 * 97.0, 4000.0), 8.0, &opts);
        },
        100,
    );
    println!("PERF 100k selection: first {first:?}, pick {pick:?} ({} hits of 200), pick_all {all:?}, window {win:?}, crossing {cross:?}, snap {snap:?}", hits.get());
    assert!(hits.get() > 0, "the probe points hit something");
    // ceilings: with a spatial index these take well under a millisecond; a linear scan of 100 000 entities is far above
    assert!(pick.as_secs_f64() < 0.010, "pick {pick:?}");
    assert!(all.as_secs_f64() < 0.010, "pick_all {all:?}");
    assert!(win.as_secs_f64() < 0.020, "window {win:?}");
    assert!(cross.as_secs_f64() < 0.020, "crossing {cross:?}");
    assert!(snap.as_secs_f64() < 0.020, "snap {snap:?}");
}
