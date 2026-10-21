#!/usr/bin/env python3
"""Adds ribbon panels/tabs and icons for the commands added after the initial port.

    cargo run -q -p cadforge-engine --example parity -- --list > /tmp/cmds.txt
    python3 tools/extend-ui.py /tmp/cmds.txt

Idempotent: panels are replaced by title, icons by name. Buttons for commands that are not registered are skipped,
so the script can be re-run whenever commands are added.
"""
import json, sys, pathlib

root = pathlib.Path(__file__).resolve().parent.parent
assets = root / "crates/cadforge-app/assets"
have = set(l.strip().upper() for l in open(sys.argv[1])) if len(sys.argv) > 1 else None

ICONS = {
    "layout": '<rect class="l" x="5" y="4" width="22" height="24"/><rect class="a" x="9" y="8" width="14" height="11"/><path class="k" d="M9 23h14M9 26h8"/>',
    "layoutnew": '<rect class="l" x="4" y="4" width="20" height="22"/><rect class="a" x="8" y="8" width="12" height="9"/><path class="o" d="M22 20v9M17.5 24.5h9"/>',
    "layoutcopy": '<rect class="k" x="9" y="4" width="19" height="21"/><rect class="l" x="4" y="9" width="19" height="21"/><rect class="a" x="8" y="13" width="11" height="8"/>',
    "layoutdel": '<rect class="l" x="5" y="4" width="20" height="22"/><rect class="a" x="9" y="8" width="12" height="9"/><path class="o" d="M19 21l8 8M27 21l-8 8"/>',
    "layoutwizard": '<rect class="l" x="5" y="4" width="20" height="22"/><path class="a" d="M10 10h10M10 15h10M10 20h6"/><path class="o" d="M24 18l1.2 3 3 1.2-3 1.2L24 26.4l-1.2-3-3-1.2 3-1.2z"/>',
    "viewport": '<rect class="l" x="4" y="6" width="24" height="20"/><rect class="a" x="8" y="10" width="16" height="12"/><path class="k" d="M12 16h8M16 12v8"/>',
    "vpolygon": '<path class="l" d="M5 21 11 6l15 4 2 14-13 4z"/><path class="a" d="M12 14l9 2-4 7z"/>',
    "vpobject": '<rect class="l" x="4" y="5" width="24" height="22"/><circle class="a" cx="16" cy="16" r="7"/>',
    "vpclip": '<rect class="l" x="4" y="5" width="24" height="22"/><path class="a" d="M10 9l13 3-3 10-10-3z"/><path class="o" d="M5 26l7-7"/>',
    "vports": '<rect class="l" x="4" y="5" width="24" height="22"/><path class="a" d="M16 5v22M4 16h12"/>',
    "paper": '<path class="l" d="M7 4h13l5 5v19H7z"/><path class="k" d="M20 4v5h5M10 14h12M10 18h12M10 22h8"/>',
    "pagesetup": '<path class="l" d="M6 4h12l5 5v10"/><path class="k" d="M18 4v5h5"/><circle class="a" cx="22" cy="23" r="4"/><path class="o" d="M22 16v3M22 27v3M15 23h3M26 23h3"/>',
    "ucs": '<path class="o" d="M7 25h19M22.5 21.5 26 25l-3.5 3.5"/><path class="g" d="M7 25V7M3.5 10.5 7 7l3.5 3.5"/><rect class="a" x="4" y="22" width="6" height="6"/>',
    "ucsicon": '<path class="o" d="M10 22h14M21 19l3 3-3 3"/><path class="g" d="M10 22V8M7 11l3-3 3 3"/><circle class="k" cx="10" cy="22" r="1.6"/>',
    "plan": '<rect class="l" x="5" y="5" width="22" height="22"/><path class="a" d="M5 16h22M16 5v22"/><circle class="o" cx="16" cy="16" r="3"/>',
    "table": '<rect class="l" x="4" y="6" width="24" height="20"/><path class="a" d="M4 12h24M4 18h24M12 6v20M20 6v20"/>',
    "tableedit": '<rect class="l" x="4" y="6" width="20" height="18"/><path class="a" d="M4 12h20M4 18h20M11 6v18"/><path class="o" d="M20 26l8-8 2.5 2.5-8 8-3.5 1z"/>',
    "tablestyle": '<rect class="l" x="4" y="5" width="24" height="16"/><path class="a" d="M4 10h24M4 15h24M12 5v16"/><path class="k" d="M6 26h20M6 29h12"/>',
    "attdef": '<path class="l" d="M16 4 27 10v12L16 28 5 22V10z"/><path class="a" d="M11 20l5-10 5 10M13 16.5h6"/>',
    "attedit": '<path class="l" d="M14 5 24 10v10L14 25 4 20V10z"/><path class="a" d="M9 17l5-9 5 9"/><path class="o" d="M20 26l8-8 2.5 2.5-8 8-3.5 1z"/>',
    "attsync": '<path class="l" d="M16 4 27 10v12L16 28 5 22V10z"/><path class="a" d="M10 14a6 6 0 0 1 10-2M22 18a6 6 0 0 1-10 2"/><path class="o" d="M19 9l2 3-3 1M13 23l-2-3 3-1"/>',
    "bedit": '<path class="l" d="M14 5 24 10v10L14 25 4 20V10z"/><path class="k" d="M4 10l10 5 10-5M14 15v10"/><path class="o" d="M20 26l8-8 2.5 2.5-8 8-3.5 1z"/>',
    "xref": '<path class="l" d="M7 4h12l6 6v18H7z"/><path class="k" d="M19 4v6h6"/><path class="a" d="M11 19h10M16 14v10"/>',
    "xattach": '<path class="l" d="M7 4h12l6 6v18H7z"/><path class="k" d="M19 4v6h6"/><path class="o" d="M10 22h8M14 18v8"/><path class="a" d="M20 20h4M20 24h4"/>',
    "xbind": '<path class="l" d="M7 4h12l6 6v18H7z"/><path class="k" d="M19 4v6h6"/><path class="a" d="M11 18h10M11 23h10"/>',
    "revcloud": '<path class="l" d="M6 22a4 4 0 0 1 1.5-7.4A5 5 0 0 1 16 11a5 5 0 0 1 8.5 3.5A4 4 0 0 1 26 22z"/>',
    "wipeout": '<path class="l" d="M5 23 13 6l14 5-4 15z"/><path class="a" d="M10 21l5-10 8 3-3 8z"/>',
    "gradient": '<rect class="l" x="5" y="6" width="22" height="20"/><path class="a" d="M7 10h18M7 13h14M7 16h10M7 19h7M7 22h4"/>',
    "mline": '<path class="l" d="M4 11h16l8 6M4 17h14l7 6"/><path class="a" d="M4 14h15l7 5"/>',
    "sketch": '<path class="l" d="M5 22c3-9 5-12 7-8s3 9 6 4 5-8 9-9"/><path class="o" d="M22 21l6-6 2 2-6 6z"/>',
    "align": '<path class="l" d="M4 8h12v6H4zM16 20h12v6H16z"/><path class="a" d="M10 14 22 20"/><circle class="o" cx="10" cy="14" r="1.8"/><circle class="o" cx="22" cy="20" r="1.8"/>',
    "overkill": '<path class="l" d="M4 10h24"/><path class="k" d="M4 16h24M4 22h24"/><path class="o" d="M10 5l12 22"/>',
    "arraypath": '<path class="a" d="M4 24c8-12 16-12 24-2"/><rect class="l" x="5" y="20" width="4" height="4"/><rect class="l" x="12" y="14" width="4" height="4"/><rect class="l" x="20" y="14" width="4" height="4"/>',
    "blend": '<path class="l" d="M4 24h6M22 8h6"/><path class="a" d="M10 24c10 0 2-16 12-16"/>',
    "splinedit": '<path class="l" d="M4 22c6-14 10 4 16-4s6-8 8-10"/><circle class="o" cx="12" cy="17" r="2"/><circle class="o" cx="20" cy="14" r="2"/>',
    "chprop": '<rect class="l" x="5" y="6" width="22" height="20"/><path class="a" d="M9 12h14M9 16h14M9 20h8"/><circle class="o" cx="24" cy="23" r="3"/>',
    "select": '<path class="l" d="M6 6l6 20 4-8 8 4z"/><rect class="a" x="4" y="4" width="22" height="22" stroke-dasharray="3 3" opacity=".6"/>',
    "filter": '<path class="l" d="M4 6h24L18 17v9l-4 2V17z"/><path class="a" d="M9 9h14"/>',
    "qdim": '<path class="l" d="M6 8v8M16 8v8M26 8v8"/><path class="a" d="M6 12h20M6 24h20"/><path class="o" d="M6 22v4M26 22v4"/>',
    "dimarc": '<path class="l" d="M6 24a14 14 0 0 1 20 0"/><path class="a" d="M8 14a12 12 0 0 1 16 0"/><path class="k" d="M7 24l1-10M25 24l-1-10"/>',
    "dimjogged": '<circle class="l" cx="20" cy="20" r="7"/><path class="a" d="M6 6l6 6 3-3 3 6"/>',
    "dimordinate": '<path class="l" d="M6 26V10h8"/><path class="a" d="M14 10h12M14 6l4 4-4 4"/>',
    "dimbreak": '<path class="l" d="M4 16h8M20 16h8M16 8v16"/><path class="o" d="M12 12l4 8"/><path class="a" d="M4 24h24"/>',
    "dimcenter": '<circle class="l" cx="16" cy="16" r="10"/><path class="a" d="M16 8v16M8 16h16"/>',
    "dimspace": '<path class="l" d="M4 8h24M4 16h24M4 24h24"/><path class="a" d="M16 8v8M16 16v8"/>',
    "dimedit": '<path class="l" d="M4 20h24"/><path class="a" d="M4 14v12M28 14v12"/><path class="o" d="M14 6l8 8-3 1-1 3-8-8z"/>',
    "tolerance": '<rect class="l" x="3" y="10" width="26" height="12"/><path class="a" d="M11 10v12M19 10v12"/><path class="o" d="M5 16h4M13 16h4M21 16h6"/>',
    "field": '<rect class="l" x="4" y="9" width="24" height="14" rx="2"/><path class="a" d="M8 16h16"/><path class="k" d="M8 12.5h4"/>',
    "spell": '<path class="l" d="M5 22 11 6l6 16M7.5 17h7"/><path class="g" d="M18 20l4 4 8-9"/>',
    "find": '<circle class="l" cx="13" cy="13" r="8"/><path class="a" d="M19 19l8 8"/><path class="k" d="M9 13h8"/>',
    "scaletext": '<path class="l" d="M5 24 11 8l6 16M7.5 19h7"/><path class="a" d="M19 12v14h12M22 15l-3-3-3 3"/>',
    "justifytext": '<path class="l" d="M5 24 11 8l6 16M7.5 19h7"/><path class="a" d="M20 10h8M20 16h8M20 22h8"/>',
    "audit": '<path class="l" d="M7 4h14l5 5v19H7z"/><path class="a" d="M12 17l3 3 6-7"/>',
    "limits": '<rect class="l" x="4" y="6" width="24" height="20"/><path class="o" d="M4 6l4 0M4 6l0 4M28 26l-4 0M28 26l0-4"/>',
    "revolve": '<path class="l" d="M16 4v24"/><path class="a" d="M16 8c9 0 11 4 11 8s-2 8-11 8"/><path class="o" d="M23 24l-3 1 1-3"/>',
    "slice": '<path class="l" d="M16 4 27 10v12L16 28 5 22V10z"/><path class="o" d="M3 20 29 11"/><path class="a" d="M5 22l11-6 11 6"/>',
    "sweep": '<circle class="l" cx="7" cy="23" r="3.5"/><path class="a" d="M10 22c6-1 8-8 8-12h10"/><path class="k" d="M18 10l4-3v6z"/>',
    "loft": '<ellipse class="l" cx="16" cy="25" rx="9" ry="3"/><ellipse class="l" cx="16" cy="7" rx="5" ry="2"/><path class="a" d="M7 25 11 7M25 25 21 7"/>',
    "torus": '<ellipse class="l" cx="16" cy="16" rx="12" ry="9"/><ellipse class="a" cx="16" cy="16" rx="5" ry="3"/>',
    "pyramid": '<path class="l" d="M16 4 4 22l12 6 12-6z"/><path class="a" d="M16 4v24M4 22l24 0"/>',
    "polysolid": '<path class="l" d="M4 24h14V10h10"/><path class="a" d="M4 28h18V14h6"/><path class="k" d="M4 24v4M18 24v4M18 10l4 4M28 10v4"/>',
    "shell": '<path class="l" d="M16 4 27 10v12L16 28 5 22V10z"/><path class="a" d="M16 10l6 3v7l-6 3-6-3v-7z"/>',
    "thicken": '<path class="l" d="M4 20 16 12l12 8-12 8z"/><path class="a" d="M4 14 16 6l12 8"/><path class="k" d="M4 14v6M28 14v6"/>',
    "filletedge": '<path class="l" d="M5 26V12l8-8h14v22z"/><path class="a" d="M5 12c0-5 3-8 8-8"/>',
    "chamferedge": '<path class="l" d="M5 26V12l8-8h14v22z"/><path class="a" d="M5 12l8-8"/>',
    "rotate3d": '<ellipse class="l" cx="16" cy="16" rx="12" ry="5"/><path class="a" d="M16 4v24"/><path class="o" d="M27 14l1 3-3 1"/>',
    "interfere": '<rect class="l" x="4" y="8" width="14" height="14"/><rect class="l" x="12" y="12" width="14" height="14"/><rect class="o" x="12" y="12" width="6" height="10"/>',
    "solidedit": '<path class="l" d="M14 5 24 10v10L14 25 4 20V10z"/><path class="k" d="M4 10l10 5 10-5M14 15v10"/><path class="o" d="M22 26l7-7"/>',
    "imageattach": '<rect class="l" x="4" y="6" width="24" height="20"/><circle class="a" cx="11" cy="13" r="2.5"/><path class="o" d="M4 24l8-7 5 4 5-6 6 5"/>',
    "imageclip": '<rect class="l" x="4" y="6" width="24" height="20"/><path class="a" d="M9 11h14v10H9z"/><path class="o" d="M4 24l8-7"/>',
    "helix": '<path class="a" d="M10 6c10 0 10 5 0 5s-10 5 0 5 10 5 0 5 10 5 12 5"/>',
    "sectionplane": '<path class="l" d="M16 4 27 10v12L16 28 5 22V10z"/><path class="o" d="M3 17 29 17"/>',
    "hide": '<path class="l" d="M16 4 27 10v12L16 28 5 22V10z"/><path class="k" d="M5 10l11 6M16 16v12" stroke-dasharray="2 2"/>',
    "status": '<circle class="l" cx="16" cy="16" r="11"/><path class="a" d="M16 10v1M16 14v8"/>',
    "time": '<circle class="l" cx="16" cy="16" r="11"/><path class="a" d="M16 8v8l6 3"/>',
    "refedit": '<path class="l" d="M7 4h12l6 6v18H7z"/><path class="o" d="M13 24l8-8 2 2-8 8-3 1z"/>',
}

# ---------------------------------------------------------------- item helpers
def ok(cmd):
    return have is None or cmd.upper().lstrip("-") in have or cmd.upper() in have

def btn(cmd, label=None, size="large", icon=None, args=None, menu=None):
    if not ok(cmd):
        return None
    b = {"t": "btn", "cmd": cmd, "size": size}
    if args:
        b["args"] = args
    if label:
        b["label"] = label
    if icon:
        b["icon"] = icon
    if menu:
        items = [m for m in menu if m is None or "sep" in m or ok(m["cmd"])]
        b["menu"] = [m for m in items if m is not None]
    return b

def mi(cmd, label=None, icon=None, args=None):
    if not ok(cmd):
        return None
    d = {"cmd": cmd}
    if label:
        d["label"] = label
    if args:
        d["args"] = args
    if icon:
        d["icon"] = icon
    return d

def stack(*items):
    items = [i for i in items if i]
    return {"t": "stack", "items": items} if items else None

def panel(title, items, launcher=None, slideout=None, prio=None):
    items = [i for i in items if i]
    if not items:
        return None
    p = {"title": title, "items": items}
    if launcher and ok(launcher):
        p["launcher"] = launcher
    if slideout:
        so = [i for i in slideout if i]
        if so:
            p["slideout"] = so
    if prio:
        p["prio"] = prio
    return p

ribbon = json.load(open(assets / "ribbon.json"))
icons = json.load(open(assets / "icons.json"))
icons.update(ICONS)

def tab(tid):
    return next((t for t in ribbon["tabs"] if t["id"] == tid), None)

def put_panel(tid, p, after=None):
    if not p:
        return
    t = tab(tid)
    for i, q in enumerate(t["panels"]):
        if q["title"] == p["title"]:
            t["panels"][i] = p
            return
    if after:
        for i, q in enumerate(t["panels"]):
            if q["title"] == after:
                t["panels"].insert(i + 1, p)
                return
    t["panels"].append(p)

def extend_panel(tid, title, add_items=None, add_slideout=None):
    t = tab(tid)
    for q in t["panels"]:
        if q["title"] == title:
            for it in add_items or []:
                if it and it not in q["items"]:
                    q["items"].append(it)
            if add_slideout:
                so = q.setdefault("slideout", [])
                for it in add_slideout:
                    if it and it not in so:
                        so.append(it)
            return

# ---------------------------------------------------------------- Layout tab
layout_tab = {
    "id": "layout",
    "title": "Layout",
    "panels": [p for p in [
        panel("Layout", [
            btn("LAYOUT", "New\nLayout", icon="layoutnew", args={"option": "New"}),
            stack(btn("LAYOUT", "Copy", "small", "layoutcopy", {"option": "Copy"}), btn("LAYOUT", "Rename", "small", "rename", {"option": "Rename"}), btn("LAYOUT", "Delete", "small", "layoutdel", {"option": "Delete"})),
            btn("PAGESETUP", "Page\nSetup", icon="pagesetup"),
            btn("LAYOUTWIZARD", "Layout\nWizard", icon="layoutwizard"),
        ]),
        panel("Layout Viewports", [
            btn("MVIEW", "Rectangular", icon="viewport"),
            btn("MVIEW", "Polygonal", icon="vpolygon", args={"option": "Polygonal"}),
            btn("MVIEW", "Object", icon="vpobject", args={"option": "Object"}),
            btn("VPCLIP", "Clip", icon="vpclip"),
            btn("VPORTS", "Viewport\nArray", icon="vports"),
        ]),
        panel("Model Space", [
            btn("MSPACE", "Model\nSpace", icon="viewcube"),
            btn("PSPACE", "Paper\nSpace", icon="paper"),
        ]),
        panel("Plot", [
            btn("PLOT", "Plot", icon="plot"),
            stack(btn("PREVIEW", "Preview", "small", "plot"), btn("EXPORTPDF", "Export PDF", "small", "pdf")),
        ]),
    ] if p],
}
if layout_tab["panels"]:
    ribbon["tabs"] = [t for t in ribbon["tabs"] if t["id"] != "layout"]
    idx = next(i for i, t in enumerate(ribbon["tabs"]) if t["id"] == "annotate") + 1
    ribbon["tabs"].insert(idx, layout_tab)

# ---------------------------------------------------------------- View: coordinates
put_panel("view", panel("Coordinates", [
    btn("UCS", "World\nUCS", icon="ucs", args={"option": "World"}, menu=[
        mi("UCS", "World", "ucs", {"option": "World"}), mi("UCS", "Move Origin", "ucs", {"option": "Move"}), mi("UCS", "Z Rotate...", "ucs", {"option": "New"}),
        mi("UCS", "Previous", "ucs", {"option": "Prev"}), mi("UCS", "Save...", "ucs", {"option": "Save"}), mi("UCS", "Restore...", "ucs", {"option": "Restore"}),
    ]),
    stack(btn("UCSICON", "Icon On", "small", "ucsicon", {"option": "ON"}), btn("UCSICON", "Icon Off", "small", "ucsicon", {"option": "OFF"}), btn("UCSICON", "At Origin", "small", "ucsicon", {"option": "ORigin"})),
    btn("PLAN", "Plan\nView", icon="plan"),
]), after="Navigate")

# ---------------------------------------------------------------- Insert
put_panel("insert", panel("Block Definition", [
    btn("ATTDEF", "Define\nAttributes", icon="attdef"),
    stack(btn("ATTEDIT", "Edit Attribute", "small", "attedit"), btn("BATTMAN", "Manager", "small", "attsync"), btn("BEDIT", "Block Editor", "small", "bedit")),
]), after="Block")
put_panel("insert", panel("Reference", [
    btn("XATTACH", "Attach", icon="xattach"),
    stack(btn("XREF", "External Refs", "small", "xref"), btn("XBIND", "Bind", "small", "xbind"), btn("IMAGEATTACH", "Image", "small", "imageattach")),
]), after="Block Definition")

# ---------------------------------------------------------------- Annotate
put_panel("annotate", panel("Tables", [
    btn("TABLE", "Table", icon="table"),
    stack(btn("TABLEEDIT", "Edit Table", "small", "tableedit"), btn("TABLESTYLE", "Table Style", "small", "tablestyle")),
]), after="Leaders")
extend_panel("annotate", "Text", add_slideout=[btn("FIND", "Find Text", "small", "find"), btn("SPELL", "Spelling", "small", "spell"), btn("FIELD", "Field", "small", "field"), btn("SCALETEXT", "Scale Text", "small", "scaletext"), btn("JUSTIFYTEXT", "Justify Text", "small", "justifytext"), btn("DDEDIT", "Edit...", "small", "textedit")])
extend_panel("annotate", "Dimensions", add_slideout=[btn(c, l, "small", i) for c, l, i in [("QDIM", "Quick Dim", "qdim"), ("DIMARC", "Arc Length", "dimarc"), ("DIMJOGGED", "Jogged", "dimjogged"), ("DIMORDINATE", "Ordinate", "dimordinate"), ("DIMBREAK", "Break", "dimbreak"), ("DIMSPACE", "Adjust Space", "dimspace"), ("DIMCENTER", "Center Mark", "dimcenter"), ("DIMEDIT", "Edit", "dimedit"), ("DIMTEDIT", "Text Position", "textedit"), ("TOLERANCE", "Tolerance", "tolerance")]])
extend_panel("annotate", "Leaders", add_items=[stack(btn("MLEADERSTYLE", "Style", "small", "mleader"), btn("MLEADEREDIT", "Add/Remove", "small", "mleader"))])

# ---------------------------------------------------------------- Home
extend_panel("home", "Modify", add_slideout=[btn(c, l, "small", i) for c, l, i in [("ALIGN", "Align", "align"), ("OVERKILL", "Overkill", "overkill"), ("ARRAYPATH", "Path Array", "arraypath"), ("BLEND", "Blend", "blend"), ("SPLINEDIT", "Edit Spline", "splinedit"), ("CHPROP", "Change Properties", "chprop"), ("SELECT", "Select", "select"), ("FILTER", "Filter", "filter"), ("DRAWORDER", "Draw Order", "copy")]])
extend_panel("home", "Draw", add_slideout=[btn(c, l, "small", i) for c, l, i in [("REVCLOUD", "Revision Cloud", "revcloud"), ("WIPEOUT", "Wipeout", "wipeout"), ("REGION", "Region", "region"), ("GRADIENT", "Gradient", "gradient"), ("MLINE", "Multiline", "mline"), ("SKETCH", "Sketch", "sketch"), ("TABLE", "Table", "table")]])
extend_panel("home", "Block", add_items=[stack(btn("ATTDEF", "Define Attributes", "small", "attdef"), btn("BEDIT", "Block Editor", "small", "bedit"), btn("XATTACH", "Attach Xref", "small", "xattach"))])

# ---------------------------------------------------------------- Manage / Output
extend_panel("manage", "Styles", add_items=[stack(btn("MLEADERSTYLE", "Multileader\nStyle", "small", "mleader"), btn("TABLESTYLE", "Table\nStyle", "small", "tablestyle"))])
extend_panel("manage", "Drawing Utilities", add_slideout=[btn(c, l, "small", i) for c, l, i in [("AUDIT", "Audit", "audit"), ("LIMITS", "Drawing Limits", "limits"), ("STATUS", "Status", "status"), ("TIME", "Time", "time"), ("PAGESETUP", "Page Setup", "pagesetup")]])
extend_panel("output", "Plot", add_items=[stack(btn("PAGESETUP", "Page Setup", "small", "pagesetup"), btn("PREVIEW", "Preview", "small", "plot"))])

# ---------------------------------------------------------------- 3D Tools
extend_panel("solid", "Primitives", add_items=[stack(btn("TORUS", "Torus", "small", "torus"), btn("PYRAMID", "Pyramid", "small", "pyramid"), btn("POLYSOLID", "Polysolid", "small", "polysolid"))])
put_panel("solid", panel("Create", [
    btn("REVOLVE", "Revolve", icon="revolve"),
    btn("SWEEP", "Sweep", icon="sweep"),
    btn("LOFT", "Loft", icon="loft"),
]), after="Solid")
extend_panel("solid", "Modify", add_items=[stack(btn("SLICE", "Slice", "small", "slice"), btn("FILLETEDGE", "Fillet Edge", "small", "filletedge"), btn("CHAMFEREDGE", "Chamfer Edge", "small", "chamferedge"))])
extend_panel("solid", "Modify", add_items=[stack(btn("SHELL", "Shell", "small", "shell"), btn("THICKEN", "Thicken", "small", "thicken"), btn("SOLIDEDIT", "Solid Edit", "small", "solidedit"))])
extend_panel("solid", "Modify", add_items=[stack(btn("3DROTATE", "3D Rotate", "small", "rotate3d"), btn("INTERFERE", "Interfere", "small", "interfere"), btn("3DSCALE", "3D Scale", "small", "scale"))])

json.dump(ribbon, open(assets / "ribbon.json", "w"), separators=(",", ":"))
json.dump(icons, open(assets / "icons.json", "w"), separators=(",", ":"))
print("ribbon tabs:", [t["id"] for t in ribbon["tabs"]], "icons:", len(icons))
