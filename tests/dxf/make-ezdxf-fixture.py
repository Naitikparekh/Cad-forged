"""Generate tests/dxf/fixtures/ezdxf-sample.dxf with ezdxf (an independent DXF producer) for the reader tests."""
import sys
import ezdxf
from ezdxf.enums import TextEntityAlignment

out = sys.argv[1] if len(sys.argv) > 1 else "tests/dxf/fixtures/ezdxf-sample.dxf"
doc = ezdxf.new("R2010", setup=True, units=4)
doc.layers.add("WALLS", color=5, linetype="DASHED")
doc.layers.add("DIMS", color=3)
msp = doc.modelspace()
msp.add_line((0, 0), (100, 0), dxfattribs={"layer": "WALLS"})
msp.add_circle((50, 50), 20, dxfattribs={"color": 1})
msp.add_arc((0, 0), 30, 10, 100)
msp.add_lwpolyline([(0, 0, 0, 0, 0.5), (40, 0, 0, 0, 0), (40, 30, 0, 0, 0)], format="xyseb", close=True)
msp.add_ellipse((60, 60), major_axis=(20, 0), ratio=0.5, start_param=0, end_param=3.14159)
msp.add_spline(fit_points=[(0, 0), (10, 15), (20, 5), (30, 20)])
spl = msp.add_spline(dxfattribs={"degree": 3})
spl.control_points = [(0, 40), (10, 60), (20, 30), (30, 55), (40, 40)]
spl.knots = [0, 0, 0, 0, 0.5, 1, 1, 1, 1]
msp.add_point((5, 5))
msp.add_text("Hello %%c25", height=3, dxfattribs={"rotation": 30}).set_placement((10, 80), align=TextEntityAlignment.MIDDLE_CENTER)
msp.add_mtext("{\\fArial|b1;Bold} line\\Psecond \\S1/2;", dxfattribs={"insert": (0, 100), "char_height": 2.5, "width": 60})
blk = doc.blocks.new("WIDGET")
blk.add_circle((0, 0), 2)
blk.add_line((-3, 0), (3, 0))
msp.add_blockref("WIDGET", (70, 10), dxfattribs={"xscale": 2, "yscale": 2, "rotation": 15})
hatch = msp.add_hatch(color=2)
hatch.paths.add_polyline_path([(100, 0), (140, 0), (140, 40), (100, 40)], is_closed=True)
edge = msp.add_hatch(color=4)
ep = edge.paths.add_edge_path()
ep.add_line((100, 60), (140, 60)); ep.add_arc((140, 80), 20, 270, 90, ccw=True); ep.add_line((140, 100), (100, 100)); ep.add_line((100, 100), (100, 60))
msp.add_linear_dim(base=(0, -15), p1=(0, 0), p2=(100, 0), dimstyle="EZDXF", dxfattribs={"layer": "DIMS"}).render()
msp.add_aligned_dim(p1=(0, 0), p2=(40, 30), distance=8, dxfattribs={"layer": "DIMS"}).render()
msp.add_radius_dim(center=(50, 50), mpoint=(70, 50), dimstyle="EZDXF", dxfattribs={"layer": "DIMS"}).render()
msp.add_solid([(0, 0), (4, 0), (4, 3), (0, 3)], dxfattribs={"color": 6})
msp.add_arc((200, 0), 10, 0, 90, dxfattribs={"extrusion": (0, 0, -1)})
msp.add_xline((0, 0), (1, 1))
doc.saveas(out)
print("wrote", out)
