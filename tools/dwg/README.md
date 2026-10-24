# Checking DWG files with independent readers

CadForge's DWG writer is tested against its own reader in `crates/cadforge-core/tests/dwg_write.rs`. Two other
decoders check the same files when they are installed; the tests skip them otherwise.

## ACadSharp (MIT, .NET)

`acadsharp-check/` is a small console program on [ACadSharp](https://github.com/DomCR/ACadSharp): `read` loads a
DWG with CRC checking on and prints what it found (and every warning), `convert` turns DWG into DXF and back, `new`
writes ACadSharp's own empty drawing, `classes` lists a file's class records.

    git clone --recurse-submodules https://github.com/DomCR/ACadSharp ~/acadsharp
    dotnet build tools/dwg/acadsharp-check -c Release -p:TargetFrameworks=net10.0 -p:ACadSharpDir=$HOME/acadsharp
    DWG_ACADSHARP=tools/dwg/acadsharp-check/bin/Release/net10.0/acadsharp-check cargo test -p cadforge-core --test dwg_write

(ACadSharp needs the .NET 10 SDK: `apt install dotnet-sdk-10.0` on Ubuntu 24.04.)

## LibreDWG (GPL, C)

Used as an external program only. Build a release (`./configure --disable-bindings && make && make install`), then:

    LIBREDWG_BIN=/prefix/bin LD_LIBRARY_PATH=/prefix/lib cargo test -p cadforge-core --test dwg_write

`check_libredwg.py` compares LibreDWG's JSON dump of a written file with the DXF it was encoded from, entity by
entity (type, layer, colour, lineweight, linetype, geometry):

    DXF_OUT=src.dxf cargo run -p cadforge-core --example dwgwrite -- drawing.dxf out.dwg
    dwgread -O JSON -o out.json out.dwg
    python3 tools/dwg/check_libredwg.py src.dxf out.json
