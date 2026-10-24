//! Annotation commands: TEXT, MTEXT, TEXTEDIT, dimensions, LEADER, hatch commands and BOUNDARY
//! (port of `src/commands/annotate.js`).
use super::helpers::*;
use super::{def, run};
use crate::app::{Event, HatchSession};
use crate::ctx::*;
use crate::memory::HatchMemory;
use crate::registry::{CommandDef, Registry};
use crate::runtime::*;
use cadforge_core::dim::{DimData, DimType};
use cadforge_core::drawing::Drawing;
use cadforge_core::entities::*;
use cadforge_core::geom::*;
use cadforge_core::ops::boundary::find_region;
use cadforge_core::text::{HJust, TextData, VJust};
use std::cell::{Cell, RefCell};
use std::rc::Rc;

fn ps(m: &str) -> PromptSpec {
    PromptSpec::new(m)
}

fn da(name: &str, f: crate::registry::RunFn) -> CommandDef {
    def("Annotation", name, f).no_pickfirst()
}

// ---------------------------------------------------------------- text input helper

/// In-place text editor when the UI provides one, otherwise a command-line prompt.
pub async fn text_input(c: &Ctx, spec: TextEditSpec) -> CmdResult<String> {
    match c.dialog(DialogReq::EditText(spec.clone())).await? {
        Some(DialogReply::Text(s)) => Ok(s),
        Some(_) => Err(CmdError::Cancel),
        None => Ok(c.string(ps(if spec.prompt.is_empty() { "Enter text" } else { &spec.prompt }).raw()).await?.text),
    }
}

fn edit_spec(prompt: &str, p: Pt, h: f64, rot: f64, jh: HJust, jv: VJust, multi: bool, w: Option<f64>) -> TextEditSpec {
    TextEditSpec { prompt: prompt.into(), p, h, rot, jh, jv, multi, w, initial: None, editing: None }
}

const JUST: [(&str, HJust, VJust); 13] = [
    ("Left", HJust::Left, VJust::Baseline),
    ("Center", HJust::Center, VJust::Baseline),
    ("Right", HJust::Right, VJust::Baseline),
    ("Middle", HJust::Center, VJust::Middle),
    ("TL", HJust::Left, VJust::Top),
    ("TC", HJust::Center, VJust::Top),
    ("TR", HJust::Right, VJust::Top),
    ("ML", HJust::Left, VJust::Middle),
    ("MC", HJust::Center, VJust::Middle),
    ("MR", HJust::Right, VJust::Middle),
    ("BL", HJust::Left, VJust::Bottom),
    ("BC", HJust::Center, VJust::Bottom),
    ("BR", HJust::Right, VJust::Bottom),
];

fn just_of(k: &str, fallback: &str) -> (HJust, VJust) {
    let f = |name: &str| JUST.iter().find(|j| j.0 == name).map(|j| (j.1, j.2));
    f(k).or_else(|| f(fallback)).unwrap_or((HJust::Left, VJust::Baseline))
}

fn text_entity(p: Pt, h: f64, s: &str, rot: f64, jh: HJust, jv: VJust) -> Entity {
    Entity::new(Kind::Text(TextData::new(p, h, s).with_rot(rot).with_just(jh, jv)))
}

// ---------------------------------------------------------------- TEXT / MTEXT

async fn text(c: Ctx, _a: Args) -> CmdResult<()> {
    let mem_h = c.app(|a| a.mem.text_height);
    let (mut jh, mut jv) = (HJust::Left, VJust::Baseline);
    let r = loop {
        let r = c.point(ps("Specify start point of text").options(&["Justify", "Style"])).await?;
        match r.kw() {
            Some("Justify") => {
                let names: Vec<String> = JUST.iter().map(|j| j.0.to_string()).collect();
                let j = c.keyword(ps("Enter an option").options_vec(names).default_str("Left")).await?;
                (jh, jv) = just_of(j.kw().unwrap_or("Left"), "Left");
                continue;
            }
            Some("Style") => {
                c.log("Text style: Standard");
                continue;
            }
            _ => break r,
        }
    };
    let Some(mut p) = r.pt() else { return Ok(()) };
    let hh = c.distance(ps("Specify height").base(p).default_num(mem_h).min(0.0).nonzero()).await?;
    let Some(h) = hh.value() else { return Ok(()) };
    c.app(|a| a.mem.text_height = h);
    c.doc_mut(|d| d.settings.text_height = h);
    let rr = c.angle(ps("Specify rotation angle of text").base(p).default_num(0.0)).await?;
    let Some(rot) = rr.value() else { return Ok(()) };
    loop {
        let s = text_input(&c, edit_spec("Enter text", p, h, rot, jh, jv, false, None)).await?;
        if s.is_empty() {
            break;
        }
        c.add(text_entity(p, h, &s, rot, jh, jv));
        p = p + polar(pt(0.0, 0.0), rot - PI / 2.0, h * 1.5);
        if c.app(|a| a.ui.dialogs) {
            break; // the in-place editor handles one line at a time
        } // command-line mode continues until an empty line
    }
    Ok(())
}

async fn mtext(c: Ctx, args: Args) -> CmdResult<()> {
    let mut h = c.app(|a| a.mem.text_height);
    let (mut rot, mut jh, mut jv) = (0.0, HJust::Left, VJust::Top);
    let Some(a) = c.point(ps("Specify first corner")).await?.pt() else { return Ok(()) };
    let b = loop {
        let b = c.corner(a, ps("Specify opposite corner").options(&["Height", "Justify", "Rotation", "Width"])).await?;
        match b.kw() {
            Some("Height") => {
                let v = c.distance(ps("Specify height").default_num(h).min(0.0).nonzero()).await?;
                if let Some(x) = v.value().filter(|x| *x != 0.0) {
                    h = x;
                    c.app(|app| app.mem.text_height = x);
                }
                continue;
            }
            Some("Justify") => {
                let j = c.keyword(ps("Enter justification").options(&["TL", "TC", "TR", "ML", "MC", "MR", "BL", "BC", "BR"]).default_str("TL")).await?;
                (jh, jv) = just_of(j.kw().unwrap_or("TL"), "TL");
                continue;
            }
            Some("Rotation") => {
                let v = c.angle(ps("Specify rotation angle").default_num(0.0)).await?;
                if let Some(x) = v.value() {
                    rot = x;
                }
                continue;
            }
            Some("Width") => {
                let v = c.distance(ps("Specify width").min(0.0).nonzero()).await?;
                if let Some(x) = v.value().filter(|x| *x != 0.0) {
                    break PointR::Pt(a + pt(x, -h * 3.0));
                }
                continue;
            }
            _ => break b,
        }
    };
    let Some(bp) = b.pt() else { return Ok(()) };
    let width = (bp.x - a.x).abs();
    let p = pt(a.x.min(bp.x), a.y.max(bp.y));
    let mut spec = edit_spec("MText", p, h, rot, jh, jv, true, Some(width));
    spec.initial = args.str("initial").map(str::to_string);
    let s = text_input(&c, spec).await?;
    if s.is_empty() {
        return Ok(());
    }
    let mut e = Entity::mtext(p, h, &s, width);
    if let Kind::Text(t) = &mut e.kind {
        t.rot = rot;
        t.jh = jh;
        t.jv = jv;
    }
    c.add(e);
    Ok(())
}

async fn textedit(c: Ctx, args: Args) -> CmdResult<()> {
    let mut id = args.id("entity");
    if id.is_none() {
        id = c.entity(ps("Select an annotation object or [Undo]").options(&["Undo"]).allow_enter().filter(|x| matches!(x.kind, Kind::Text(_) | Kind::Dim(_) | Kind::Leader { .. }))).await?.id();
    }
    let Some(id) = id else { return Ok(()) };
    let Some(e) = c.entity_by_id(id) else { return Ok(()) };
    c.ed(|ed| ed.temp_sel = [id].into_iter().collect());
    let (initial, p, h, rot, multi, w) = match &e.kind {
        Kind::Text(t) => (t.s.clone(), t.p, t.h, t.rot, t.multi, t.w),
        Kind::Dim(d) => (d.txt.clone().unwrap_or_else(|| "<>".into()), dim_text_pos(&e, &c), 2.5, 0.0, false, None),
        Kind::Leader { pts, s, h } => (s.clone(), pts.first().copied().unwrap_or(pt(0.0, 0.0)), *h, 0.0, false, None),
        _ => return Ok(()),
    };
    let mut spec = edit_spec("Enter new text", p, h, rot, HJust::Left, VJust::Baseline, multi, w);
    spec.initial = Some(initial);
    spec.editing = Some(id);
    let s = text_input(&c, spec).await;
    c.ed(|ed| ed.temp_sel.clear());
    let s = s?;
    c.doc_mut(|d| {
        d.change(|d| {
            if let Some(ent) = d.by_id_mut(id) {
                match &mut ent.kind {
                    Kind::Dim(dd) => dd.txt = if s == "<>" { Some(String::new()) } else { Some(s.clone()) },
                    Kind::Text(t) => t.s = s.clone(),
                    Kind::Leader { s: ls, .. } => *ls = s.clone(),
                    _ => {}
                }
            }
        })
    });
    Ok(())
}

fn dim_text_pos(e: &Entity, c: &Ctx) -> Pt {
    c.doc(|d| texts_of(e, d).first().map(|t| t.p).unwrap_or(pt(0.0, 0.0)))
}

// ---------------------------------------------------------------- dimensions

fn dim_ent(kind: DimType, ds: &str, f: impl FnOnce(&mut DimData)) -> Entity {
    let mut d = DimData::new(kind);
    d.ds = Some(ds.to_string());
    f(&mut d);
    Entity::dim(d)
}

fn cur_dim_style(c: &Ctx) -> String {
    c.doc(|d| d.settings.cur_dim_style.clone())
}

fn h_or_v(p1: Pt, p2: Pt, dp: Pt) -> f64 {
    let m = mid(p1, p2);
    if (dp.y - m.y).abs() >= (dp.x - m.x).abs() {
        0.0
    } else {
        PI / 2.0
    }
}

fn nearest_seg(segs: &[Seg], p: Pt) -> Option<Seg> {
    let mut best: Option<(Seg, f64)> = None;
    for s in segs {
        let d = s.nearest(p).2;
        if best.map_or(true, |b| d < b.1) {
            best = Some((*s, d));
        }
    }
    best.map(|b| b.0)
}

async fn linear_like(c: Ctx, kind: DimType, rotated: bool) -> CmdResult<()> {
    let first = c.point(ps("Specify first extension line origin or <select object>").allow_enter()).await?;
    let (p1, p2);
    if matches!(first, PointR::Enter(None)) {
        let o = c.entity(ps("Select object to dimension").filter(|e| matches!(e.kind, Kind::Line { .. } | Kind::Arc { .. } | Kind::Circle { .. } | Kind::Pline { .. }))).await?;
        let EntR::Entity { id, pt: op, .. } = o else { return Ok(()) };
        let Some(e) = c.entity_by_id(id) else { return Ok(()) };
        let segs = c.doc(|d| segs_of(&e, d).to_vec());
        if let Kind::Circle { c: cc, r } = e.kind {
            p1 = cc + pt(-r, 0.0);
            p2 = cc + pt(r, 0.0);
        } else if let Some(best) = nearest_seg(&segs, op) {
            p1 = best.start();
            p2 = best.end();
        } else {
            return Ok(());
        }
    } else if let Some(f) = first.pt() {
        p1 = f;
        let Some(s) = c.point(ps("Specify second extension line origin").base(f)).await?.pt() else { return Ok(()) };
        p2 = s;
    } else {
        return Ok(());
    }
    let ds = cur_dim_style(&c);
    let mut fixed: Option<f64> = None;
    if rotated {
        let Some(a) = c.angle(ps("Specify angle of dimension line").default_num(0.0)).await?.value() else { return Ok(()) };
        fixed = Some(a);
    }
    let mut override_: Option<String> = None;
    loop {
        let (ds2, ov, fx) = (ds.clone(), override_.clone(), fixed);
        let make = move |dp: Pt| -> Entity {
            dim_ent(kind, &ds2, |d| {
                d.p1 = Some(p1);
                d.p2 = Some(p2);
                d.dp = Some(dp);
                if kind == DimType::Lin {
                    d.ang = Some(fx.unwrap_or_else(|| h_or_v(p1, p2, dp)));
                }
                if let Some(t) = &ov {
                    d.txt = Some(t.clone());
                }
            })
        };
        let opts: &[&str] = if kind == DimType::Lin { &["Mtext", "Text", "Angle", "Horizontal", "Vertical", "Rotated"] } else { &["Mtext", "Text", "Angle"] };
        let make2 = make.clone();
        let r = c.point(ps("Specify dimension line location").options(opts).base(mid(p1, p2)).no_ortho().preview(move |_, p| vec![pv(make2(p))])).await?;
        match r.kw() {
            Some("Mtext") | Some("Text") => {
                let t = c.string(ps("Enter dimension text <>").raw().default_str("<>")).await?;
                if t.text != "<>" {
                    override_ = Some(t.text);
                }
                continue;
            }
            Some("Horizontal") => {
                fixed = Some(0.0);
                continue;
            }
            Some("Vertical") => {
                fixed = Some(PI / 2.0);
                continue;
            }
            Some("Rotated") => {
                let a = c.angle(ps("Specify angle of dimension line").default_num(0.0)).await?;
                if let Some(v) = a.value() {
                    fixed = Some(v);
                }
                continue;
            }
            Some("Angle") => {
                c.log("Text angle is determined automatically.");
                continue;
            }
            _ => {}
        }
        let Some(p) = r.pt() else { return Ok(()) };
        let id = c.add(make(p));
        c.app(|a| a.mem.last_dim = Some(id));
        return Ok(());
    }
}

async fn dim_text(c: &Ctx, e: &mut Entity) -> CmdResult<()> {
    let t = c.string(ps("Enter dimension text <>").raw().default_str("<>")).await?;
    if t.text != "<>" {
        if let Kind::Dim(d) = &mut e.kind {
            d.txt = Some(t.text);
        }
    }
    Ok(())
}

async fn radial(c: Ctx, kind: DimType) -> CmdResult<()> {
    let o = c.entity(ps("Select arc or circle").filter(|e| matches!(e.kind, Kind::Circle { .. } | Kind::Arc { .. }))).await?;
    let EntR::Entity { id, pt: op, .. } = o else { return Ok(()) };
    let Some(e0) = c.entity_by_id(id) else { return Ok(()) };
    let (ctr, rr) = match e0.kind {
        Kind::Circle { c, r } | Kind::Arc { c, r, .. } => (c, r),
        _ => return Ok(()),
    };
    let dir = unit(op - ctr);
    let on = ctr + dir * rr;
    let ds = cur_dim_style(&c);
    let ds2 = ds.clone();
    let make = move |dp: Pt| -> Entity {
        dim_ent(kind, &ds2, |d| {
            d.c = Some(ctr);
            d.pt = Some(on);
            d.dp = Some(dp);
        })
    };
    let m2 = make.clone();
    let r = c.point(ps("Specify dimension line location").options(&["Mtext", "Text", "Angle"]).base(ctr).no_ortho().preview(move |_, p| vec![pv(m2(p))])).await?;
    if matches!(r.kw(), Some("Mtext") | Some("Text")) {
        let mut e = make(polar(ctr, angle_of(ctr, op), rr * 1.5));
        dim_text(&c, &mut e).await?;
        c.add(e);
        return Ok(());
    }
    let Some(p) = r.pt() else { return Ok(()) };
    c.add(make(p));
    Ok(())
}

async fn dimangular(c: Ctx, _a: Args) -> CmdResult<()> {
    let o = c.entity(ps("Select arc, circle, line, or <specify vertex>").allow_enter().filter(|e| matches!(e.kind, Kind::Line { .. } | Kind::Arc { .. } | Kind::Circle { .. }))).await?;
    let (ctr, p1, p2);
    let picked = match &o {
        EntR::Entity { id, pt, .. } => c.entity_by_id(*id).map(|e| (e, *pt)),
        _ => None,
    };
    match picked {
        Some((e, _)) if matches!(e.kind, Kind::Line { .. }) => {
            let o2 = c.entity(ps("Select second line").filter(|e| matches!(e.kind, Kind::Line { .. }))).await?;
            let Some(e2) = o2.id().and_then(|i| c.entity_by_id(i)) else { return Ok(()) };
            let (a, b) = c.doc(|d| (segs_of(&e, d)[0], segs_of(&e2, d)[0]));
            let Some(hit) = seg_intersect(&a, &b, true, true).first().copied() else {
                c.log_error("The lines are parallel.");
                return Ok(());
            };
            ctr = hit.pt;
            let far = |s: &Seg| if dist(s.start(), ctr) > dist(s.end(), ctr) { s.start() } else { s.end() };
            p1 = far(&a);
            p2 = far(&b);
        }
        Some((Entity { kind: Kind::Arc { c: cc, r, a0, a1 }, .. }, _)) => {
            ctr = cc;
            p1 = polar(cc, a0, r);
            p2 = polar(cc, a1, r);
        }
        Some((Entity { kind: Kind::Circle { c: cc, .. }, .. }, op)) => {
            ctr = cc;
            p1 = op;
            let Some(s) = c.point(ps("Specify second angle endpoint").base(cc)).await?.pt() else { return Ok(()) };
            p2 = s;
        }
        _ => {
            let Some(v) = c.point(ps("Specify angle vertex")).await?.pt() else { return Ok(()) };
            ctr = v;
            let Some(a) = c.point(ps("Specify first angle endpoint").base(ctr)).await?.pt() else { return Ok(()) };
            p1 = a;
            let Some(b) = c.point(ps("Specify second angle endpoint").base(ctr)).await?.pt() else { return Ok(()) };
            p2 = b;
        }
    }
    let ds = cur_dim_style(&c);
    let make = move |dp: Pt| -> Entity {
        dim_ent(DimType::Ang, &ds, |d| {
            d.c = Some(ctr);
            d.p1 = Some(p1);
            d.p2 = Some(p2);
            d.dp = Some(dp);
        })
    };
    let m2 = make.clone();
    let r = c.point(ps("Specify dimension arc line location").options(&["Mtext", "Text"]).base(ctr).no_ortho().preview(move |_, p| vec![pv(m2(p))])).await?;
    let Some(p) = r.pt() else { return Ok(()) };
    c.add(make(p));
    Ok(())
}

fn dim_of(e: &Entity) -> Option<&DimData> {
    match &e.kind {
        Kind::Dim(d) => Some(d),
        _ => None,
    }
}

async fn base_dim(c: &Ctx, prompt: &str) -> CmdResult<Option<Entity>> {
    let last = c.app(|a| a.mem.last_dim);
    if let Some(e) = last.and_then(|id| c.entity_by_id(id)) {
        return Ok(Some(e));
    }
    let o = c.entity(ps(prompt).filter(|e| matches!(&e.kind, Kind::Dim(d) if matches!(d.kind, DimType::Lin | DimType::Ali)))).await?;
    Ok(o.id().and_then(|i| c.entity_by_id(i)))
}

async fn dimcontinue(c: Ctx, _a: Args) -> CmdResult<()> {
    let Some(base) = base_dim(&c, "Select continued dimension").await? else { return Ok(()) };
    let Some(bd) = dim_of(&base).cloned() else { return Ok(()) };
    let Some(mut prev) = bd.p2 else { return Ok(()) };
    loop {
        let bd2 = bd.clone();
        let make = move |p_: Pt, prev: Pt| -> Entity {
            let mut d = DimData::new(bd2.kind);
            d.p1 = Some(prev);
            d.p2 = Some(p_);
            d.dp = bd2.dp;
            d.ang = bd2.ang;
            d.ds = bd2.ds.clone();
            Entity::dim(d)
        };
        let m2 = make.clone();
        let r = c.point(ps("Specify a second extension line origin or [Undo/Select]").options(&["Undo", "Select"]).allow_enter().base(prev).preview(move |_, p| vec![pv(m2(p, prev))])).await?;
        let Some(p) = r.pt() else { return Ok(()) };
        let id = c.add(make(p, prev));
        prev = p;
        c.app(|a| a.mem.last_dim = Some(id));
    }
}

async fn dimbaseline(c: Ctx, _a: Args) -> CmdResult<()> {
    let Some(base) = base_dim(&c, "Select base dimension").await? else { return Ok(()) };
    let Some(bd) = dim_of(&base).cloned() else { return Ok(()) };
    let (Some(bp1), Some(bdp)) = (bd.p1, bd.dp) else { return Ok(()) };
    let scale = c.doc(|d| {
        let name = bd.ds.clone().unwrap_or_default();
        d.settings.dim_styles.get(&name).or_else(|| d.settings.dim_styles.get("Standard")).map(|s| s.scale).unwrap_or(1.0)
    });
    let space = 3.75 * scale;
    let ang = if bd.kind == DimType::Ali { angle_of(bp1, bd.p2.unwrap_or(bp1)) } else { bd.ang.unwrap_or(0.0) };
    let n = pt(-ang.sin(), ang.cos());
    let side = if dot(bdp - bp1, n) >= 0.0 { 1.0 } else { -1.0 };
    let mut dp = bdp;
    loop {
        let (bd2, cur_dp) = (bd.clone(), dp);
        let make = move |p_: Pt| -> Entity {
            let mut d = DimData::new(bd2.kind);
            d.p1 = Some(bp1);
            d.p2 = Some(p_);
            d.dp = Some(cur_dp + n * (side * space));
            d.ang = bd2.ang;
            d.ds = bd2.ds.clone();
            Entity::dim(d)
        };
        let m2 = make.clone();
        let r = c.point(ps("Specify a second extension line origin or [Undo/Select]").options(&["Undo", "Select"]).allow_enter().base(bp1).preview(move |_, p| vec![pv(m2(p))])).await?;
        let Some(p) = r.pt() else { return Ok(()) };
        let id = c.add(make(p));
        dp = c.doc(|d| d.by_id(id).and_then(|e| dim_of(e).and_then(|x| x.dp))).unwrap_or(dp);
    }
}

async fn dim(c: Ctx, _a: Args) -> CmdResult<()> {
    let o = c
        .entity(
            ps("Select object or specify first extension line origin or")
                .options(&["Angular", "Baseline", "Continue", "aLign"])
                .allow_enter()
                .filter(|e| matches!(e.kind, Kind::Line { .. } | Kind::Arc { .. } | Kind::Circle { .. } | Kind::Pline { .. })),
        )
        .await?;
    match o.kw() {
        Some("Angular") => return Ok(c.sub_forced("DIMANGULAR", Args::none()).await),
        Some("Baseline") => return Ok(c.sub_forced("DIMBASELINE", Args::none()).await),
        Some("Continue") => return Ok(c.sub_forced("DIMCONTINUE", Args::none()).await),
        Some("aLign") => return linear_like(c, DimType::Ali, false).await,
        _ => {}
    }
    let EntR::Entity { id, pt: op, .. } = o else { return Ok(()) };
    let Some(e0) = c.entity_by_id(id) else { return Ok(()) };
    let ds = cur_dim_style(&c);
    if let Kind::Circle { c: ctr, r } | Kind::Arc { c: ctr, r, .. } = e0.kind {
        let on = ctr + unit(op - ctr) * r;
        let ds2 = ds.clone();
        let make = move |dp: Pt| dim_ent(DimType::Rad, &ds2, |d| {
            d.c = Some(ctr);
            d.pt = Some(on);
            d.dp = Some(dp);
        });
        let m2 = make.clone();
        let r_ = c
            .point(ps("Specify dimension line location or [Mtext/Text/Angle/Diameter/Radius]").options(&["Diameter", "Radius"]).base(ctr).no_ortho().preview(move |_, p| vec![pv(m2(p))]))
            .await?;
        if r_.kw() == Some("Diameter") {
            if let Some(p) = c.point(ps("Specify dimension line location").base(ctr).no_ortho()).await?.pt() {
                c.add(dim_ent(DimType::Dia, &ds, |d| {
                    d.c = Some(ctr);
                    d.pt = Some(on);
                    d.dp = Some(p);
                }));
            }
            return Ok(());
        }
        if let Some(p) = r_.pt() {
            c.add(make(p));
        }
        return Ok(());
    }
    let segs = c.doc(|d| segs_of(&e0, d).to_vec());
    let Some(s) = nearest_seg(&segs, op) else { return Ok(()) };
    let (p1, p2) = (s.start(), s.end());
    let kind = if (p1.x - p2.x).abs() < 1e-9 || (p1.y - p2.y).abs() < 1e-9 { DimType::Lin } else { DimType::Ali };
    let ds2 = ds.clone();
    let make = move |dp: Pt| {
        dim_ent(kind, &ds2, |d| {
            d.p1 = Some(p1);
            d.p2 = Some(p2);
            d.dp = Some(dp);
            if kind == DimType::Lin {
                d.ang = Some(h_or_v(p1, p2, dp));
            }
        })
    };
    let m2 = make.clone();
    let r_ = c.point(ps("Specify dimension line location").base(mid(p1, p2)).no_ortho().preview(move |_, p| vec![pv(m2(p))])).await?;
    if let Some(p) = r_.pt() {
        let id = c.add(make(p));
        c.app(|a| a.mem.last_dim = Some(id));
    }
    Ok(())
}

// ---------------------------------------------------------------- LEADER

async fn mleader(c: Ctx, _a: Args) -> CmdResult<()> {
    // a style defined with MLEADERSTYLE sets the text height; otherwise the last text height is used
    let styled = c.doc(|d| d.settings.mleader_styles.get(&d.settings.cur_mleader_style).map(|st| st.text_height));
    let h = styled.unwrap_or_else(|| c.app(|a| a.mem.text_height));
    let Some(a) = c.point(ps("Specify leader arrowhead location")).await?.pt() else { return Ok(()) };
    let Some(r) = c.point(ps("Specify leader landing location").base(a).preview(move |_, p| vec![pv(Entity::leader(vec![a, p], "", h))])).await?.pt() else { return Ok(()) };
    let jh = if r.x >= a.x { HJust::Left } else { HJust::Right };
    let s = text_input(&c, edit_spec("Enter text", r, h, 0.0, jh, VJust::Middle, false, None)).await?;
    c.add(Entity::leader(vec![a, r], &s, h));
    Ok(())
}

// ---------------------------------------------------------------- HATCH

fn hatch_entity(loops: &[Vec<Vertex>], h: &HatchMemory) -> Entity {
    let mut e = Entity::hatch(loops.to_vec(), &h.pattern);
    if let Kind::Hatch(hd) = &mut e.kind {
        hd.ps = h.scale;
        hd.pa = rad(h.angle);
    }
    if let Some(col) = h.color {
        e.color = Some(col);
    }
    e
}

fn loop_from_entity(e: &Entity) -> Option<Vec<Vertex>> {
    match &e.kind {
        Kind::Circle { c, r } => Some(vec![Vertex::new(c.x + r, c.y, 1.0), Vertex::new(c.x - r, c.y, 1.0)]),
        Kind::Pline { v, closed: true } => Some(v.clone()),
        _ => None,
    }
}

async fn hatch(c: Ctx, args: Args) -> CmdResult<()> {
    let settings = c.app(|a| a.mem.hatch.clone());
    let loops: Rc<RefCell<Vec<Vec<Vertex>>>> = Rc::new(RefCell::new(vec![]));
    let count = Rc::new(Cell::new(0usize));
    c.app(|a| {
        a.hatch_session = Some(HatchSession { settings: settings.clone(), editing: None, count: count.clone() });
        a.emit(Event::HatchSession);
    });
    let (l2, s2) = (loops.clone(), settings.clone());
    let preview = move |_: &Drawing, _: Pt| -> Vec<PreviewItem> {
        let l = l2.borrow();
        if l.is_empty() {
            vec![]
        } else {
            vec![pv(hatch_entity(&l, &s2.borrow()))]
        }
    };
    c.set_preview(preview.clone());
    let result: CmdResult<()> = async {
        if let Some(p) = args.str("pattern") {
            settings.borrow_mut().pattern = p.to_string();
        }
        loop {
            let r = c.point(ps("Pick internal point or").options(&["Select objects", "seTtings", "Undo"]).allow_enter().no_ortho().preview(preview.clone())).await?;
            match r.kw() {
                Some("seTtings") => {
                    c.dialog(DialogReq::Hatch).await?;
                    continue;
                }
                Some("Undo") => {
                    loops.borrow_mut().clear();
                    count.set(0);
                    continue;
                }
                Some("Select objects") => {
                    let sel = c.select(ps("Select objects").filter(|e| matches!(e.kind, Kind::Circle { .. } | Kind::Pline { .. } | Kind::Ellipse(_) | Kind::Line { .. }))).await?;
                    for e in c.entities(&sel.ents) {
                        if let Some(l) = loop_from_entity(&e) {
                            loops.borrow_mut().push(l);
                        }
                    }
                    count.set(loops.borrow().len());
                    continue;
                }
                _ => {}
            }
            let Some(p) = r.pt() else { break };
            match c.doc(|d| find_region(d, p, None)) {
                Err(m) => c.log_error(&m),
                Ok(reg) => {
                    let n = reg.loops.len();
                    loops.borrow_mut().extend(reg.loops);
                    count.set(loops.borrow().len());
                    c.log(&format!("{n} loop{} extracted", if n == 1 { "" } else { "s" }));
                    c.set_preview(preview.clone());
                }
            }
        }
        let l = loops.borrow().clone();
        if !l.is_empty() {
            let e = hatch_entity(&l, &settings.borrow());
            c.add(e);
            c.log("Hatch created.");
        }
        Ok(())
    }
    .await;
    c.app(|a| {
        a.hatch_session = None;
        a.emit(Event::HatchSession);
    });
    result
}

async fn hatchedit(c: Ctx, args: Args) -> CmdResult<()> {
    let id = match args.id("entity") {
        Some(i) => Some(i),
        None => c.entity(ps("Select hatch object").filter(|x| matches!(x.kind, Kind::Hatch(_)))).await?.id(),
    };
    let Some(id) = id else { return Ok(()) };
    let Some(e) = c.entity_by_id(id) else { return Ok(()) };
    let Kind::Hatch(h) = &e.kind else { return Ok(()) };
    let live = Rc::new(RefCell::new(HatchMemory { pattern: h.pattern.clone(), scale: h.ps, angle: deg(h.pa), color: e.color, island: "normal".into() }));
    c.app(|a| {
        a.hatch_session = Some(HatchSession { settings: live.clone(), editing: Some(id), count: Rc::new(Cell::new(0)) });
        a.emit(Event::HatchSession);
    });
    let r = c.dialog(DialogReq::HatchEdit).await;
    let l = live.borrow().clone();
    c.doc_mut(|d| {
        d.change(|d| {
            if let Some(Entity { kind: Kind::Hatch(h), .. }) = d.by_id_mut(id) {
                h.pattern = l.pattern.clone();
                h.ps = l.scale;
                h.pa = rad(l.angle);
            }
        })
    });
    c.app(|a| {
        a.hatch_session = None;
        a.emit(Event::HatchSession);
    });
    r.map(|_| ())
}

async fn boundary(c: Ctx, _a: Args) -> CmdResult<()> {
    loop {
        let r = c.point(ps("Pick internal point").options(&["Advanced options"]).allow_enter()).await?;
        let Some(p) = r.pt() else { return Ok(()) };
        match c.doc(|d| find_region(d, p, None)) {
            Err(m) => {
                c.log_error(&m);
                continue;
            }
            Ok(reg) => {
                let n = reg.loops.len();
                for l in reg.loops {
                    c.add(Entity::pline(l, true));
                }
                c.log(&format!("BOUNDARY created {n} polyline{}", if n == 1 { "" } else { "s" }));
            }
        }
    }
}

pub fn register(r: &mut Registry) {
    r.define(da("TEXT", run(text)).aliases(&["DT", "DTEXT"]).title("Single Line Text").icon("text").desc("Creates a single-line text object."));
    r.define(da("MTEXT", run(mtext)).aliases(&["T", "MT"]).title("Multiline Text").icon("mtext").desc("Creates a multiline text object."));
    r.define(da("TEXTEDIT", run(textedit)).aliases(&["ED"]).title("Edit Text").icon("textedit").desc("Edits a dimensional constraint, dimension text, or text object."));
    r.define(da("DIMLINEAR", run(|c, _a| linear_like(c, DimType::Lin, false))).aliases(&["DLI"]).title("Linear").icon("dimlinear").desc("Creates a linear dimension."));
    r.define(da("DIMROTATED", run(|c, _a| linear_like(c, DimType::Lin, true))).title("Rotated").icon("dimlinear").desc("Creates a linear dimension whose dimension line is at a given angle."));
    r.define(da("DIMALIGNED", run(|c, _a| linear_like(c, DimType::Ali, false))).aliases(&["DAL"]).title("Aligned").icon("dimaligned").desc("Creates an aligned linear dimension."));
    r.define(da("DIMRADIUS", run(|c, _a| radial(c, DimType::Rad))).aliases(&["DRA"]).title("Radius").icon("dimradius").desc("Creates a radius dimension for a circle or an arc."));
    r.define(da("DIMDIAMETER", run(|c, _a| radial(c, DimType::Dia))).aliases(&["DDI"]).title("Diameter").icon("dimdiameter").desc("Creates a diameter dimension for a circle or an arc."));
    r.define(da("DIMANGULAR", run(dimangular)).aliases(&["DAN"]).title("Angular").icon("dimangular").desc("Creates an angular dimension."));
    r.define(da("DIMCONTINUE", run(dimcontinue)).aliases(&["DCO"]).title("Continue").icon("dimcontinue").desc("Creates a dimension that starts from an extension line of a previously created dimension."));
    r.define(da("DIMBASELINE", run(dimbaseline)).aliases(&["DBA"]).title("Baseline").icon("dimbaseline").desc("Creates a linear, angular, or ordinate dimension from the baseline of the previous or selected dimension."));
    r.define(da("DIM", run(dim)).title("Dimension").icon("dim").desc("Creates multiple dimension types with a single command."));
    r.define(da("MLEADER", run(mleader)).aliases(&["MLD", "LE", "LEADER", "QLEADER"]).title("Multileader").icon("mleader").desc("Creates a leader line with annotation text."));
    r.define(da("HATCH", run(hatch)).aliases(&["H", "BHATCH"]).title("Hatch").icon("hatch").desc("Fills an enclosed area or selected objects with a hatch pattern."));
    r.define(da("HATCHEDIT", run(hatchedit)).aliases(&["HE"]).title("Edit Hatch").icon("hatchedit").desc("Modifies an existing hatch or fill."));
    r.define(da("BOUNDARY", run(boundary)).aliases(&["BO"]).title("Boundary").icon("boundary").desc("Creates a region or a polyline from an area enclosed by objects."));
}
