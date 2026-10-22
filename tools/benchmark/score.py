#!/usr/bin/env python3
"""AutoCAD parity benchmark: computes the composite score and (optionally) writes docs/BENCHMARK.md.

    python3 tools/benchmark/score.py            # print the scores
    python3 tools/benchmark/score.py --check    # also fail when evidence pointers in rubric.tsv no longer match
    python3 tools/benchmark/score.py --write    # regenerate docs/BENCHMARK.md

Measured axes come from the command registry (`cargo run -p cadforge-engine --example parity`) and the test sources;
checklist axes come from tools/benchmark/rubric.tsv, where every credit points at code or tests.
"""
import argparse, glob, json, os, re, subprocess, sys

ROOT = os.path.abspath(os.path.join(os.path.dirname(__file__), "..", ".."))
BENCH = os.path.join(ROOT, "tools", "benchmark")

# id, title, weight, how it is measured
AXES = [
    ("cmd_core", "Command breadth: core inventory", 10, "registry vs. acad-commands.tsv (usage-weighted)"),
    ("cmd_tail", "Command breadth: long tail", 10, "registry vs. acad-longtail.tsv (usage-weighted)"),
    ("aliases", "Command aliases (acad.pgp)", 2, "registry vs. acad-aliases.tsv"),
    ("tested", "Commands exercised by automated tests", 8, "registered commands named by a test"),
    ("objects", "Object types and drawing model", 8, "checklist"),
    ("io", "File formats and interchange", 10, "checklist"),
    ("drafting", "Drafting aids, selection, inquiry", 8, "checklist"),
    ("ui", "User interface and workflow", 12, "checklist"),
    ("annotation", "Annotation, styles and symbols", 7, "checklist"),
    ("output", "Layouts, plotting and output", 6, "checklist"),
    ("solids3d", "3D modelling", 5, "checklist"),
    ("reliability", "Reliability and performance", 6, "checklist"),
    ("extend", "Extensibility and automation", 5, "checklist"),
    ("platform", "Platform and distribution", 3, "checklist"),
]
assert sum(a[2] for a in AXES) == 100


def run(cmd):
    return subprocess.run(cmd, cwd=ROOT, check=True, capture_output=True, text=True).stdout


def parity():
    out = run(["cargo", "run", "-q", "-p", "cadforge-engine", "--example", "parity", "--", "--json"])
    return json.loads(out.strip().splitlines()[-1])


def read_rubric():
    items = []
    for line in open(os.path.join(BENCH, "rubric.tsv"), encoding="utf-8"):
        if line.startswith("#") or not line.strip():
            continue
        f = line.rstrip("\n").split("\t")
        f += [""] * (6 - len(f))
        items.append(dict(axis=f[0], id=f[1], credit=float(f[2]), text=f[3], evidence=[e.strip() for e in f[4].split(" ; ") if e.strip()], missing=f[5]))
    scope = read_scope()
    known = {(i["axis"], i["id"]) for i in items}
    for key in scope:
        if key not in known:
            sys.exit(f"scope.tsv names {key[0]}/{key[1]}, which is not in rubric.tsv")
    for i in items:
        i["out"] = scope.get((i["axis"], i["id"]), "")
    return items


def read_scope():
    """Rubric items the project owner has put out of scope: {(axis, id): reason}. They leave the axis average and the gap list."""
    path = os.path.join(BENCH, "scope.tsv")
    out = {}
    if os.path.exists(path):
        for line in open(path, encoding="utf-8"):
            if line.startswith("#") or not line.strip():
                continue
            f = line.rstrip("\n").split("\t")
            f += [""] * (3 - len(f))
            out[(f[0], f[1])] = f[2]
    return out


def check_evidence(items):
    """Return the evidence pointers that do not hold."""
    bad = []
    for it in items:
        if it["credit"] > 0 and not it["evidence"]:
            bad.append((it["id"], "(no evidence)", "credit above 0 needs a pointer"))
        for e in it["evidence"]:
            path, _, rx = e.partition("::")
            files = glob.glob(os.path.join(ROOT, path))
            if not files:
                bad.append((it["id"], e, "no such file"))
                continue
            hit = False
            for f in files:
                try:
                    if re.search(rx, open(f, encoding="utf-8", errors="replace").read(), re.M):
                        hit = True
                        break
                except re.error as ex:
                    bad.append((it["id"], e, f"bad regex: {ex}"))
                    hit = True
                    break
            if not hit:
                bad.append((it["id"], e, "pattern not found"))
    return bad


def test_sources():
    files = glob.glob(os.path.join(ROOT, "crates", "*", "tests", "*.rs")) + glob.glob(os.path.join(ROOT, "tests", "web", "*.mjs"))
    return "\n".join(open(f, encoding="utf-8", errors="replace").read() for f in files)


def tested_commands():
    names = [n for n in run(["cargo", "run", "-q", "-p", "cadforge-engine", "--example", "parity", "--", "--list"]).split() if not n.startswith("_") and not n.startswith("OSNAP_")]
    aliases = {}
    for line in run(["cargo", "run", "-q", "-p", "cadforge-engine", "--example", "parity", "--", "--aliases"]).splitlines():
        a, c = line.split("\t")
        aliases.setdefault(c, []).append(a)
    src = test_sources()
    covered, missing = [], []
    for n in names:
        forms = [n] + [a for a in aliases.get(n, []) if len(a) > 1]
        hit = False
        for form in forms:
            q = re.escape(form)
            rx = rf"""(?i)(\.(?:cmd|cmd_args|play|run|start|ty|script)\(\s*\\?"{q}\\?"|\bt\.cmd\(\s*['"]{q}['"]|\(command\s+\\?"_?\.?{q}\\?"|\bcmd\(\s*"{q}")"""
            if re.search(rx, src):
                hit = True
                break
        (covered if hit else missing).append(n)
    return covered, missing


def count_tests():
    rust = 0
    for f in glob.glob(os.path.join(ROOT, "crates", "**", "*.rs"), recursive=True):
        rust += len(re.findall(r"#\[test\]", open(f, encoding="utf-8", errors="replace").read()))
    web = len(re.findall(r"^await test\(", open(os.path.join(ROOT, "tests", "web", "run.mjs"), encoding="utf-8").read(), re.M))
    return rust, web


def compute(par, items, covered, missing):
    weight = {a[0]: a[2] for a in AXES}
    frac = {
        "cmd_core": par["core"] / 100.0,
        "cmd_tail": par["tail"] / 100.0,
        "aliases": par["aliases"] / 100.0,
        "tested": len(covered) / max(1, len(covered) + len(missing)),
    }
    for a in AXES[4:]:
        mine = [i for i in items if i["axis"] == a[0] and not i["out"]]
        frac[a[0]] = sum(i["credit"] for i in mine) / len(mine) if mine else 0.0
    points = {k: frac[k] * weight[k] for k in frac}
    return frac, points


def table(par, items, frac, points, covered, missing):
    rows = ["| Axis | What is measured | Weight | Score | Points |", "|---|---|---:|---:|---:|"]
    for k, title, w, how in AXES:
        rows.append(f"| {title} | {how} | {w} | {100 * frac[k]:.1f}% | {points[k]:.1f} |")
    total = sum(points.values())
    rows.append(f"| **Composite** | | **100** | | **{total:.1f}** |")
    return "\n".join(rows), total


def write_doc(par, items, frac, points, covered, missing, total):
    tbl, _ = table(par, items, frac, points, covered, missing)
    rust, web = count_tests()
    lines = []
    w = lines.append
    w("# CadForge vs. AutoCAD: parity benchmark")
    w("")
    w("*Generated by `python3 tools/benchmark/score.py --write`; do not edit by hand. Rubric: `tools/benchmark/rubric.tsv`.*")
    w("")
    w(f"**Composite score: {total:.1f} / 100.**")
    w("")
    w("This number is a self-assessment against a written rubric, not a measurement against AutoCAD itself (AutoCAD cannot be run or downloaded in the build environment). "
      "It is meant to be reproducible and hard to inflate: the command axes are computed from the program's own command registry, the test axis from the test sources, "
      "and every credit in the checklists points at code or a test that `score.py --check` verifies still exists.")
    w("")
    w("## Scores")
    w("")
    w(tbl)
    w("")
    w(f"Command inventory: {par['core_n'][1]} core commands (weighted by how often drawings use them) and {par['tail_n'][1]} long-tail commands. "
      f"{par['core_n'][0]} / {par['core_n'][1]} core and {par['tail_n'][0]} / {par['tail_n'][1]} long-tail commands exist in the registry; commands that only report \"not supported\" earn part of their weight (`tools/benchmark/stubs.tsv`). "
      f"All {par['aliases']:.0f}% of the standard `acad.pgp` aliases resolve to the right command.")
    w("")
    w(f"Test inventory: {rust} Rust tests and {web} browser tests (headless Chromium driving the WebAssembly build).")
    w("")
    w("## How to read it")
    w("")
    w("- **Breadth** axes count commands that exist. They say nothing about how deep a command goes; the *tested* axis only credits a command when a test types it.")
    w("- **Checklists** list AutoCAD capabilities. Full credit means the capability is present and tested; partial credit names what is missing in the last column.")
    w("- The long-tail inventory is deliberately uncomfortable: it contains specialist features (meshes, surfaces, sheet sets, rendering, VBA...) that a 2D-first drafting program does not offer. They weigh 10 of the 100 points.")
    w("- The score counts only what is built. It does not reward things AutoCAD lacks, and it cannot measure feel, polish or the thousands of small behaviours of a 40-year-old product.")
    w("")
    w("## Checklists")
    for k, title, wt, how in AXES[4:]:
        mine = [i for i in items if i["axis"] == k]
        w("")
        w(f"### {title} ({wt} points, {100 * frac[k]:.1f}%)")
        w("")
        w("| Capability | Credit | Evidence / what is missing |")
        w("|---|---:|---|")
        for i in mine:
            ev = "; ".join(f"`{e}`" for e in i["evidence"])
            note = i["missing"]
            cell = ev if not note else (f"{ev}<br>*{note}*" if ev else f"*{note}*")
            if i["out"]:
                w(f"| ~~{i['text']}~~ | out of scope | *{i['out']}* |")
            else:
                w(f"| {i['text']} | {i['credit']:g} | {cell} |")
    w("")
    w("## What stands between this and 95")
    w("")
    live = [i for i in items if not i["out"]]
    per_item = lambda i: dict((a[0], a[2]) for a in AXES)[i["axis"]] / max(1, len([x for x in live if x["axis"] == i["axis"]]))
    gaps = sorted((i for i in live if i["credit"] < 1), key=lambda i: -per_item(i) * (1 - i["credit"]))
    w("Points still on the table, largest first (checklist items below full credit):")
    w("")
    for i in gaps[:25]:
        per = per_item(i)
        w(f"- {per * (1 - i['credit']):.2f} pt: {i['text']} ({i['axis']}) {('- ' + i['missing']) if i['missing'] else ''}")
    w("")
    w("Long-tail commands that do not exist yet are listed by `cargo run -p cadforge-engine --example parity -- --missing`. Many are tied to features this program does not have at all "
      "(meshes and surfaces, sheet sets, annotative scaling, dynamic blocks, PDF / DGN underlays, DWG, VBA / .NET, rendering). Closing the gap to 95 means building those, "
      "not polishing what is here; some of them (DWG, the plug-in ecosystem, 40 years of drawings and habits) cannot be reproduced at all.")
    w("")
    out = [i for i in items if i["out"]]
    if out:
        w("## Out of scope (by decision)")
        w("")
        w("These rubric items were taken out of the score by the project owner. They no longer count in their axis; the other items in that axis now carry the weight.")
        w("")
        for i in out:
            w(f"- {i['text']} ({i['axis']}): {i['out']}")
        w("")
    w("## Untested commands")
    w("")
    w(f"{len(missing)} registered commands are not named by any test: " + ", ".join(f"`{m}`" for m in missing))
    w("")
    open(os.path.join(ROOT, "docs", "BENCHMARK.md"), "w", encoding="utf-8").write("\n".join(lines))


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--check", action="store_true")
    ap.add_argument("--write", action="store_true")
    a = ap.parse_args()
    items = read_rubric()
    bad = check_evidence(items)
    for b in bad:
        print("EVIDENCE:", *b)
    par = parity()
    covered, missing = tested_commands()
    frac, points = compute(par, items, covered, missing)
    tbl, total = table(par, items, frac, points, covered, missing)
    print(tbl)
    print(f"\ncomposite {total:.1f} / 100   (commands tested: {len(covered)}/{len(covered) + len(missing)})")
    if a.write:
        write_doc(par, items, frac, points, covered, missing, total)
        print("wrote docs/BENCHMARK.md")
    if a.check and bad:
        sys.exit(1)


if __name__ == "__main__":
    main()
