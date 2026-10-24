//! A small sample drawing: a dimensioned L-bracket with bolt holes, centre lines and a hatched section
//! (port of `src/io/sample.js`).
use crate::dim::{DimData, DimType};
use crate::drawing::Drawing;
use crate::entities::Entity;
use crate::geom::*;
use crate::style::{Color, Layer};

pub fn sample_drawing() -> Drawing {
    let mut d = Drawing::new("L-Bracket");
    d.layers
        .push(Layer::new("Outline").with_color(Color::Aci(7)).with_lw(50));
    d.layers
        .push(Layer::new("Holes").with_color(Color::Aci(4)).with_lw(25));
    d.layers.push(
        Layer::new("Centre")
            .with_color(Color::Aci(1))
            .with_ltype("CENTER")
            .with_lw(13),
    );
    d.layers.push(
        Layer::new("Dimensions")
            .with_color(Color::Aci(3))
            .with_lw(13),
    );
    d.layers
        .push(Layer::new("Hatch").with_color(Color::Aci(8)).with_lw(9));
    d.layers
        .push(Layer::new("Notes").with_color(Color::Aci(2)).with_lw(13));
    let outline: Vec<Pt> = [
        (0.0, 0.0),
        (120.0, 0.0),
        (120.0, 30.0),
        (30.0, 30.0),
        (30.0, 90.0),
        (0.0, 90.0),
    ]
    .iter()
    .map(|&(x, y)| pt(x, y))
    .collect();
    d.add(Entity::pline_pts(&outline, true).on_layer("Outline"));
    for (x, y, r) in [(75.0, 15.0, 5.0), (15.0, 60.0, 5.0), (15.0, 75.0, 5.0)] {
        d.add(Entity::circle(pt(x, y), r).on_layer("Holes"));
        d.add(Entity::line(pt(x - r - 4.0, y), pt(x + r + 4.0, y)).on_layer("Centre"));
        d.add(Entity::line(pt(x, y - r - 4.0), pt(x, y + r + 4.0)).on_layer("Centre"));
    }
    let mut hatch = Entity::hatch(
        vec![vec![
            Vertex::new(0.0, 0.0, 0.0),
            Vertex::new(8.0, 0.0, 0.0),
            Vertex::new(8.0, 30.0, 0.0),
            Vertex::new(0.0, 30.0, 0.0),
        ]],
        "ANSI31",
    )
    .on_layer("Hatch");
    if let crate::entities::Kind::Hatch(h) = &mut hatch.kind {
        h.ps = 1.2;
    }
    d.add(hatch);
    let lin = |p1: Pt, p2: Pt, dp: Pt, ang: f64| {
        let mut dd = DimData::new(DimType::Lin);
        dd.p1 = Some(p1);
        dd.p2 = Some(p2);
        dd.dp = Some(dp);
        dd.ang = Some(ang);
        Entity::dim(dd).on_layer("Dimensions")
    };
    d.add(lin(pt(0.0, 0.0), pt(120.0, 0.0), pt(60.0, -16.0), 0.0));
    d.add(lin(pt(0.0, 0.0), pt(0.0, 90.0), pt(-16.0, 45.0), PI / 2.0));
    d.add(lin(
        pt(120.0, 0.0),
        pt(120.0, 30.0),
        pt(136.0, 15.0),
        PI / 2.0,
    ));
    let mut dia = DimData::new(DimType::Dia);
    dia.c = Some(pt(75.0, 15.0));
    dia.pt = Some(pt(
        75.0 + 5.0 * std::f64::consts::FRAC_1_SQRT_2,
        15.0 + 5.0 * std::f64::consts::FRAC_1_SQRT_2,
    ));
    dia.dp = Some(pt(94.0, 29.0));
    d.add(Entity::dim(dia).on_layer("Dimensions"));
    d.add(
        Entity::leader(
            vec![pt(15.0, 75.0), pt(52.0, 100.0)],
            "Ø10 THRU, 2 HOLES",
            3.5,
        )
        .on_layer("Notes"),
    );
    d.add(
        Entity::mtext(
            pt(0.0, -42.0),
            4.0,
            "L-BRACKET\nMaterial: 6061-T6 aluminium\nScale 1:1",
            100.0,
        )
        .on_layer("Notes"),
    );
    d.cur.layer = "Outline".into();
    d.dirty = false;
    d
}
