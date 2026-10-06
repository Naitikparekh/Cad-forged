"""Validate a DXF file written by CadForge with ezdxf: load, audit, count entities."""
import sys
from collections import Counter
import ezdxf
from ezdxf import recover

path = sys.argv[1]
doc = ezdxf.readfile(path)
auditor = doc.audit()
msp = doc.modelspace()
counts = Counter(e.dxftype() for e in msp)
print("version:", doc.acad_release, "dxfversion:", doc.dxfversion)
print("entities:", dict(sorted(counts.items())))
print("layers:", sorted(l.dxf.name for l in doc.layers))
print("blocks:", sorted(b.name for b in doc.blocks if not b.name.startswith('*Paper') and not b.name.startswith('*Model')))
errs = [str(e.message) for e in auditor.errors]
fixes = [str(e.message) for e in auditor.fixes]
print("audit errors:", errs)
print("audit fixes:", fixes)
# round-trip through ezdxf's own writer to prove the structure is sound
import io
buf = io.StringIO(); doc.write(buf)
print("rewrite ok:", len(buf.getvalue()) > 1000)
sys.exit(1 if errs else 0)
