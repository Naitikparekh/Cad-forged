#!/usr/bin/env python3
"""Adds tests/dxf/corpus/pline-widths-R2018.dxf: polylines with start / end widths (per vertex, constant, and the old POLYLINE form).

    python3 -I tests/dxf/make-corpus-widths.py

Separate from make-corpus.py so that regenerating this file leaves the others untouched.
"""
import ezdxf, pathlib

out = pathlib.Path(__file__).resolve().parent / "corpus"
d = ezdxf.new("R2018")
msp = d.modelspace()
# x, y, start width, end width, bulge: the edge from the first vertex tapers 2 -> 6, the next one is uniform 6, the last has none
msp.add_lwpolyline([(0, 0, 2, 6, 0), (20, 0, 6, 6, 0), (20, 10, 0, 0, 0)], format="xyseb")
# one constant width for every edge (group 43)
msp.add_lwpolyline([(0, 30), (20, 30), (20, 40)], dxfattribs={"const_width": 4})
# the old heavy POLYLINE: header defaults 1 -> 3 for vertices without widths of their own
msp.add_polyline2d([(0, 60), (20, 60), (20, 70)], dxfattribs={"default_start_width": 1, "default_end_width": 3})
# a closed one with an arc edge and widths
msp.add_lwpolyline([(50, 0, 1, 1, 0), (60, 0, 1, 3, 0.5), (60, 10, 3, 1, 0)], format="xyseb", close=True)
d.saveas(out / "pline-widths-R2018.dxf")
print("wrote", out / "pline-widths-R2018.dxf")
