#!/usr/bin/env python3
"""Adds tests/dxf/corpus/linetypes-R2018.dxf: custom linetypes defined in the file's LTYPE table, used by a layer and by objects.

    python3 -I tests/dxf/make-corpus-defs.py

Separate from make-corpus.py so that regenerating this file leaves the others untouched.
"""
import ezdxf, pathlib

out = pathlib.Path(__file__).resolve().parent / "corpus"
d = ezdxf.new("R2018")
# pattern = [total length, dash, gap (negative), dash, gap, ...]; a 0 element is a dot
d.linetypes.add("FENCE_LINE", pattern=[18.0, 12.0, -6.0], description="Fence ---- ---- ----")
d.linetypes.add("UTILITY_DOT", pattern=[9.0, 6.0, -3.0, 0.0, -3.0], description="Utility ---- . ---- .")
d.layers.add("fences", linetype="FENCE_LINE", color=3)
msp = d.modelspace()
msp.add_line((0, 0), (100, 0), dxfattribs={"layer": "fences"})
msp.add_line((0, 10), (100, 10), dxfattribs={"linetype": "UTILITY_DOT"})
d.saveas(out / "linetypes-R2018.dxf")
print("wrote", out / "linetypes-R2018.dxf")
