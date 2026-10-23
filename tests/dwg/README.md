# DWG test drawings

`samples/` holds one AutoCAD drawing per DWG version (R14 = AC1014, 2000 = AC1015, 2004 = AC1018, 2007 = AC1021,
2010 = AC1024, 2013 = AC1027, 2018 = AC1032) and AutoCAD's DXF export of the 2004 file. They come from the test suite
of [ACadSharp](https://github.com/DomCR/ACadSharp) (MIT License, see `samples/ACADSHARP-LICENSE`).

`expected.txt` is what CadForge reads from AutoCAD's DXF export of each file; `crates/cadforge-core/tests/dwg.rs`
checks that reading the DWG gives the same. The comment at the top of that test says how to rebuild it.

Developer tools: `cargo run -p cadforge-core --example dwginfo -- file.dwg [twin.dxf]` (what was read and skipped),
`--example dwgdiff -- file.dwg twin.dxf` (entity-by-entity comparison), `--example dwg2dxf -- file.dwg` (the DXF the
reader produces).
