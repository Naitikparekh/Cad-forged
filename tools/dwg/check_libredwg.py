#!/usr/bin/env python3
"""Check a DWG written by CadForge against the DXF it was encoded from, as decoded by LibreDWG.

    DXF_OUT=src.dxf cargo run -p cadforge-core --example dwgwrite -- in.dwg out.dwg
    dwgread -O JSON -o out.json out.dwg
    python3 tools/dwg/check_libredwg.py src.dxf out.json

LibreDWG is an independent decoder (used here as an external tool only). For every entity in the DXF the script
looks up the object with the same handle and compares its type, layer, colour, lineweight, linetype and main
geometry. Exit status 1 if anything differs.
"""
import json
import math
import re
import sys

LW = [0, 5, 9, 13, 15, 18, 20, 25, 30, 35, 40, 50, 53, 60, 70, 80, 90, 100, 106, 120, 140, 158, 200, 211]
DWG_TYPE = {'TEXT': 'TEXT', 'ATTRIB': 'ATTRIB', 'ATTDEF': 'ATTDEF', 'INSERT': 'INSERT', 'LINE': 'LINE', 'CIRCLE': 'CIRCLE',
            'ARC': 'ARC', 'POINT': 'POINT', 'ELLIPSE': 'ELLIPSE', 'SPLINE': 'SPLINE', 'RAY': 'RAY', 'XLINE': 'XLINE',
            'MTEXT': 'MTEXT', 'LEADER': 'LEADER', 'LWPOLYLINE': 'LWPOLYLINE', 'HATCH': 'HATCH', 'SOLID': 'SOLID',
            'VIEWPORT': 'VIEWPORT', 'IMAGE': 'IMAGE', 'SEQEND': 'SEQEND'}
DIM = {0: 'DIMENSION_LINEAR', 1: 'DIMENSION_ALIGNED', 2: 'DIMENSION_ANG2LN', 3: 'DIMENSION_DIAMETER',
       4: 'DIMENSION_RADIUS', 5: 'DIMENSION_ANG3PT', 6: 'DIMENSION_ORDINATE'}


def unescape(s):
    # LibreDWG shows \U+XXXX escapes as the characters
    return re.sub(r'\\U\+([0-9A-Fa-f]{4})', lambda m: chr(int(m.group(1), 16)), s)


def records(path):
    lines = [l.rstrip('\r') for l in open(path, encoding='cp1252').read().split('\n')]
    out, cur, section = [], None, None
    for i in range(0, len(lines) - 1, 2):
        c, v = int(lines[i]), lines[i + 1]
        if c == 0:
            cur = {'type': v, 'g': [], 'section': section}
            out.append(cur)
        elif cur is not None:
            cur['g'].append((c, v))
            if cur['type'] == 'SECTION' and c == 2:
                section = v
                cur['section'] = v
    return out


def first(r, code, default=None):
    for c, v in r['g']:
        if c == code:
            return v
    return default


def handle(r):
    return int(first(r, 5) or first(r, 105) or '0', 16)


def main(dxf_path, json_path):
    recs = records(dxf_path)
    doc = json.load(open(json_path))
    objs = {o['handle'][-1] if o['handle'][1] else 0: o for o in doc['OBJECTS']}
    layers = {handle(r): first(r, 2) for r in recs if r['type'] == 'LAYER'}
    layer_by_name = {v.lower(): k for k, v in layers.items()}
    ltypes = {first(r, 2).lower(): handle(r) for r in recs if r['type'] == 'LTYPE'}
    problems, checked = [], 0
    for r in recs:
        if r['section'] not in ('ENTITIES', 'BLOCKS') or r['type'] in ('SECTION', 'ENDSEC', 'BLOCK', 'ENDBLK'):
            continue
        h = handle(r)
        o = objs.get(h)
        name = f"{r['type']} {h:X}"
        if o is None:
            problems.append(f'{name}: missing')
            continue
        checked += 1
        want = DIM[int(first(r, 70, '0')) & 7] if r['type'] == 'DIMENSION' else DWG_TYPE[r['type']]
        got = o.get('entity') or o.get('object')
        if got != want:
            problems.append(f'{name}: type {got}')
        lay = o.get('layer', [0, 0, 0, 0])[-1]
        if lay != layer_by_name.get((first(r, 8) or '0').lower()):
            problems.append(f'{name}: layer {layers.get(lay)}')
        color = o.get('color')
        rgb = first(r, 420)
        if isinstance(color, dict):
            # 2004 files: LibreDWG gives the index (left out when 0) and the true colour
            if rgb is not None:
                if int(color.get('rgb', '0'), 16) & 0xFFFFFF != int(rgb):
                    problems.append(f'{name}: true colour {color} != {int(rgb):06x}')
                color = None
            else:
                color = color.get('index', 0)
        if color is not None and color != int(first(r, 62, '256')):
            problems.append(f'{name}: colour {color} != {first(r, 62, "256")}')
        lw = int(first(r, 370, '-1'))
        want_lw = {-1: 29, -2: 30, -3: 31}.get(lw, LW.index(lw) if lw in LW else None)
        if o.get('linewt') != want_lw:
            problems.append(f'{name}: lineweight {o.get("linewt")} != {want_lw}')
        lt = (first(r, 6) or 'ByLayer').lower()
        want_flags = {'bylayer': 0, 'byblock': 1, 'continuous': 2}.get(lt, 3)
        if o.get('ltype_flags') != want_flags:
            problems.append(f'{name}: linetype flags {o.get("ltype_flags")} != {want_flags}')
        elif want_flags == 3 and o.get('ltype', [0, 0, 0, 0])[-1] != ltypes.get(lt):
            problems.append(f'{name}: linetype')

        def near(a, b):
            return abs(a - b) <= 1e-9 * max(1.0, abs(a), abs(b))

        def pt(code):
            return [float(first(r, code, '0')), float(first(r, code + 10, '0')), float(first(r, code + 20, '0'))]

        def same(key, value):
            v = o.get(key)
            if isinstance(value, list):
                v = (v or [])[:len(value)] if len(value) == 2 else v
                ok = v is not None and len(v) == len(value) and all(near(a, b) for a, b in zip(v, value))
            elif isinstance(value, float):
                ok = v is not None and near(v, value)
            else:
                ok = v == value
            if not ok:
                problems.append(f'{name}: {key} {v} != {value}')

        t = r['type']
        if t == 'LINE':
            same('start', pt(10))
            same('end', pt(11))
        elif t in ('CIRCLE', 'ARC'):
            same('center', pt(10))
            same('radius', float(first(r, 40)))
            if t == 'ARC':
                same('start_angle', math.radians(float(first(r, 50))))
                same('end_angle', math.radians(float(first(r, 51))))
        elif t in ('TEXT', 'ATTRIB', 'ATTDEF'):
            same('ins_pt', pt(10)[:2])
            same('height', float(first(r, 40)))
            same('default_value' if t == 'ATTDEF' else 'text_value', unescape(first(r, 1, '')))
            if t != 'TEXT':
                same('tag', first(r, 2))
        elif t == 'MTEXT':
            same('ins_pt', pt(10))
            same('text_height', float(first(r, 40)))
            same('text', unescape(''.join(v for c, v in r['g'] if c == 3) + first(r, 1, '')))
        elif t == 'INSERT':
            same('ins_pt', pt(10))
            same('rotation', math.radians(float(first(r, 50, '0'))))
        elif t == 'LWPOLYLINE':
            xs = [float(v) for c, v in r['g'] if c == 10]
            ys = [float(v) for c, v in r['g'] if c == 20]
            got = o.get('points', [])
            if len(got) != len(xs) or not all(near(p[0], x) and near(p[1], y) for p, x, y in zip(got, xs, ys)):
                problems.append(f'{name}: points {got[:3]} != {list(zip(xs, ys))[:3]}')
    print(f'{checked} entities checked, {len(problems)} problems')
    for p in problems[:40]:
        print('  ' + p)
    return 1 if problems else 0


if __name__ == '__main__':
    sys.exit(main(sys.argv[1], sys.argv[2]))
