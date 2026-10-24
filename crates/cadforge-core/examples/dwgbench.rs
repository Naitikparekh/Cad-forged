//! Time DWG writing and reading of a large drawing: cargo run --release -p cadforge-core --example dwgbench -- [n]
use cadforge_core::entities::Entity;
use cadforge_core::geom::pt;
fn main() {
    let n: usize = std::env::args()
        .nth(1)
        .and_then(|s| s.parse().ok())
        .unwrap_or(100_000);
    let mut d = cadforge_core::drawing::Drawing::new("bench");
    for i in 0..n {
        let x = (i % 1000) as f64;
        let y = (i / 1000) as f64;
        d.add(match i % 4 {
            0 => Entity::line(pt(x, y), pt(x + 0.8, y + 0.5)),
            1 => Entity::circle(pt(x, y), 0.3),
            2 => Entity::arc(pt(x, y), 0.4, 0.0, 1.5),
            _ => Entity::text(pt(x, y), 0.2, &format!("T{i}")),
        });
    }
    let t = std::time::Instant::now();
    let dxf = cadforge_core::io::write_dxf(&d);
    let t_dxf = t.elapsed();
    let t = std::time::Instant::now();
    let dwg = cadforge_core::io::write_dwg(&d).unwrap();
    let t_dwg = t.elapsed();
    let t = std::time::Instant::now();
    let back = cadforge_core::io::read_dwg(&dwg, "bench").unwrap();
    let t_read = t.elapsed();
    println!("{n} entities: DXF {} KB in {t_dxf:?}; DWG 2004 {} KB in {t_dwg:?}; read back {} entities in {t_read:?}", dxf.len() / 1024, dwg.len() / 1024, back.model.entities.len());
}
