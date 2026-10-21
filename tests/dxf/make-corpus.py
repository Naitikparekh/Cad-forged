#!/usr/bin/env python3
"""Generates a corpus of DXF files with ezdxf for the interoperability tests (crates/cadforge-core/tests/corpus.rs).

    python3 -I tests/dxf/make-corpus.py

Every file holds the same drawing in a different DXF version, plus a few special-purpose files.
"""
import ezdxf, pathlib, math
from ezdxf.enums import TextEntityAlignment
from ezdxf import colors

out = pathlib.Path(__file__).resolve().parent / "corpus"
out.mkdir(exist_ok=True)

def build(version: str) -> "ezdxf.document.Drawing":
    doc = ezdxf.new(version, setup=True)
    doc.header["$INSUNITS"] = 4
    doc.layers.add("Outline", color=7, linetype="Continuous", lineweight=50)
    doc.layers.add("Centre", color=1, linetype="CENTER")
    doc.layers.add("Hidden", color=3, linetype="HIDDEN")
    doc.layers.add("Dims", color=2)
    doc.layers.add("Off", color=5).off()
    doc.layers.add("Frozen", color=6).freeze()
    doc.layers.add("Rgb").rgb = (255, 128, 0)
    msp = doc.modelspace()
    msp.add_line((0, 0), (100, 0), dxfattribs={"layer": "Outline"})
    msp.add_line((100, 0), (100, 60), dxfattribs={"layer": "Outline", "color": 1})
    msp.add_circle((50, 30), 15, dxfattribs={"layer": "Centre"})
    msp.add_arc((50, 30), 25, 30, 150, dxfattribs={"layer": "Hidden"})
    msp.add_lwpolyline([(0, 70, 0, 0, 0.5), (40, 70), (40, 90, 0, 0, -0.3), (0, 90)], format="xyseb", close=True, dxfattribs={"layer": "Outline"})
    pl = msp.add_polyline2d([(120, 0), (160, 0), (160, 30), (120, 30)], close=True)
    msp.add_ellipse((200, 30), major_axis=(30, 0), ratio=0.5, start_param=0, end_param=math.tau)
    msp.add_ellipse((200, 80), major_axis=(20, 10), ratio=0.4, start_param=0.3, end_param=2.5)
    msp.add_spline([(0, 100), (20, 120), (40, 105), (60, 125)])
    msp.add_open_spline([(80, 100), (100, 130), (120, 100), (140, 130), (160, 100)], degree=3)
    msp.add_point((5, 5))
    msp.add_text("Hello Ø DXF", height=5, rotation=20, dxfattribs={"layer": "Dims"}).set_placement((120, 100), align=TextEntityAlignment.MIDDLE_CENTER)
    msp.add_mtext("{\\fArial|b1;Bold}\\PLine two\\P%%udegree%%d", dxfattribs={"insert": (120, 120), "char_height": 4, "width": 60})
    msp.add_xline((0, 0), (1, 1))
    msp.add_ray((0, 0), (1, 0.2))
    msp.add_solid([(0, 0), (10, 0), (0, 10), (10, 10)])
    msp.add_trace([(20, 0), (30, 0), (20, 10), (30, 10)])
    # dimensions
    msp.add_linear_dim(base=(0, -15), p1=(0, 0), p2=(100, 0), dxfattribs={"layer": "Dims"}).render()
    msp.add_aligned_dim(p1=(0, 0), p2=(100, 60), distance=10).render()
    msp.add_radius_dim((50, 30), mpoint=(62, 40), radius=15).render()
    msp.add_diameter_dim((50, 30), angle=45, radius=15).render()
    msp.add_angular_dim_3p(base=(20, 20), center=(0, 0), p1=(30, 0), p2=(0, 30)).render()
    msp.add_ordinate_x_dim(feature_location=(30, 10), offset=(0, 10), origin=(0, 0)).render()
    # leader
    msp.add_leader([(10, 10), (30, 40), (45, 40)], dxfattribs={"layer": "Dims"})
    # hatches
    h1 = msp.add_hatch(color=2)
    h1.paths.add_polyline_path([(0, 140), (40, 140), (40, 170), (0, 170)], is_closed=True)
    h2 = msp.add_hatch(color=4)
    h2.set_pattern_fill("ANSI31", scale=2)
    h2.paths.add_polyline_path([(60, 140), (100, 140), (100, 170), (60, 170)], is_closed=True)
    h2.paths.add_polyline_path([(70, 150), (90, 150), (90, 160), (70, 160)], is_closed=True)
    if version != "R2000":
        h3 = msp.add_hatch()
        h3.set_gradient((255, 0, 0), (0, 0, 255), rotation=30)
        h3.paths.add_polyline_path([(120, 140), (160, 140), (160, 170), (120, 170)], is_closed=True)
    h4 = msp.add_hatch(color=5)
    e = h4.paths.add_edge_path()
    e.add_line((180, 140), (220, 140)); e.add_arc((220, 155), 15, 270, 90); e.add_line((220, 170), (180, 170)); e.add_line((180, 170), (180, 140))
    # blocks and inserts with attributes
    blk = doc.blocks.new("TAG")
    blk.add_circle((0, 0), 5)
    blk.add_attdef("NUMBER", (7, 0), dxfattribs={"height": 2.5, "prompt": "Number"})
    ref = msp.add_blockref("TAG", (30, 200), dxfattribs={"xscale": 1.5, "yscale": 1.5, "rotation": 30})
    ref.add_attrib("NUMBER", "17", (37, 200), dxfattribs={"height": 2.5})
    msp.add_blockref("TAG", (60, 200)).add_attrib("NUMBER", "18", (67, 200))
    nested = doc.blocks.new("NESTED")
    nested.add_blockref("TAG", (0, 0))
    nested.add_line((0, 0), (20, 20))
    msp.add_blockref("NESTED", (100, 200))
    msp.add_auto_blockref("NESTED", (150, 200), {"NUMBER": "99"})
    # hidden-layer objects
    msp.add_line((0, 0), (5, 5), dxfattribs={"layer": "Off"})
    msp.add_line((0, 0), (5, 6), dxfattribs={"layer": "Frozen"})
    msp.add_line((0, 0), (5, 7), dxfattribs={"layer": "Rgb", "true_color": colors.rgb2int((10, 200, 90))})
    return doc

def add_paper(doc):
    """Two layouts with viewports looking at model space."""
    lay = doc.layouts.get("Layout1")
    lay.page_setup(size=(420, 297), margins=(10, 10, 10, 10), units="mm")
    vp = lay.add_viewport(center=(210, 148), size=(300, 200), view_center_point=(100, 60), view_height=120)
    lay.add_line((10, 10), (410, 10))
    lay.add_text("TITLE BLOCK", height=8, dxfattribs={"insert": (20, 20)})
    lay2 = doc.layouts.new("Detail")
    lay2.page_setup(size=(297, 210), margins=(5, 5, 5, 5), units="mm")
    lay2.add_viewport(center=(148, 105), size=(200, 120), view_center_point=(50, 30), view_height=60)
    lay2.add_viewport(center=(60, 40), size=(80, 40), view_center_point=(200, 30), view_height=80)

for v in ["R2000", "R2004", "R2007", "R2010", "R2013", "R2018"]:
    d = build(v)
    if v in ("R2010", "R2018"):
        add_paper(d)
    d.saveas(out / f"kitchen-{v}.dxf")

# modern annotation objects
d = ezdxf.new("R2018", setup=True)
msp = d.modelspace()
mleader = msp.add_multileader_mtext("Standard")
mleader.set_content("Note: see detail", char_height=3)
mleader.add_leader_line(ezdxf.render.mleader.ConnectionSide.left, [(10, 10), (30, 30)])
from ezdxf.math import Vec2
mleader.build(insert=Vec2(40, 30))
msp.add_mline([(0, 0), (50, 0), (50, 30)], dxfattribs={"scale_factor": 2})
msp.add_3dface([(0, 0, 0), (10, 0, 0), (10, 10, 5), (0, 10, 5)])
msp.add_polyline3d([(0, 0, 0), (10, 10, 5), (20, 0, 10)])
msp.add_wipeout([(60, 0), (80, 0), (80, 20), (60, 20)])
msp.add_helix(radius=5, pitch=2, turns=3, ccw=True)
d.saveas(out / "annotation-R2018.dxf")

# units + header variables
d = ezdxf.new("R2010")
d.header["$INSUNITS"] = 1
d.header["$LTSCALE"] = 2.5
d.header["$PDMODE"] = 35
d.header["$PDSIZE"] = 3
d.header["$LIMMIN"] = (0, 0)
d.header["$LIMMAX"] = (420, 297)
d.header["$INSBASE"] = (10, 20, 0)
d.modelspace().add_point((1, 1))
d.saveas(out / "header-R2010.dxf")
print("wrote", len(list(out.glob('*.dxf'))), "files")
