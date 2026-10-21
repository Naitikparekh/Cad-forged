//! Additional AutoCAD draw/modify/selection commands (parity batch: ext_modify).
//!
//! SELECT, FILTER, MULTIPLE, GROUPEDIT, GROUPS (`select`), CHPROP, CHANGE, ALIGN, OVERKILL, FLATTEN (`props`),
//! REGION, GRADIENT, WIPEOUT and region-aware UNION / SUBTRACT / INTERSECT (`region`), REVCLOUD (`cloud`),
//! MLINE, SKETCH (`lines`), BLEND, SPLINEDIT (`curves`) and ARRAYPATH (`array`).
use super::def;
use crate::ctx::Ctx;
use crate::registry::{CommandDef, Registry, RunFn};
use crate::runtime::PromptSpec;
use crate::sysvars::Value;
use cadforge_core::geom::{Pt, Seg};

mod array;
mod cloud;
mod curves;
mod lines;
mod props;
mod region;
mod select;

fn ps(m: &str) -> PromptSpec {
    PromptSpec::new(m)
}

fn dm(name: &str, f: RunFn) -> CommandDef {
    def("Modify", name, f)
}

fn dd(name: &str, f: RunFn) -> CommandDef {
    def("Draw", name, f)
}

// Session settings (arc lengths, gradient colours...) live with the application preferences, so they follow the
// application rather than the drawing and are visible to SETVAR once stored.

fn pref_num(c: &Ctx, name: &str, default: f64) -> f64 {
    c.app(|a| match a.sysvars.prefs.get(name) {
        Some(Value::Num(v)) => *v,
        _ => default,
    })
}

fn pref_text(c: &Ctx, name: &str, default: &str) -> String {
    c.app(|a| match a.sysvars.prefs.get(name) {
        Some(v) => v.text(),
        None => default.to_string(),
    })
}

fn set_pref(c: &Ctx, name: &str, v: Value) {
    c.app(|a| {
        a.sysvars.prefs.insert(name.to_string(), v);
    });
}

/// Point and unit tangent at distance `d` along `segs` (clamped to the ends).
fn point_at(segs: &[Seg], d: f64) -> (Pt, Pt) {
    let mut acc = 0.0;
    for s in segs {
        let len = s.length();
        if d <= acc + len + 1e-9 {
            let t = if len > 0.0 { ((d - acc) / len).clamp(0.0, 1.0) } else { 0.0 };
            return (s.at(t), s.tangent(t));
        }
        acc += len;
    }
    let last = segs[segs.len() - 1];
    (last.end(), last.tangent(1.0))
}

pub fn register(r: &mut Registry) {
    select::register(r);
    props::register(r);
    region::register(r);
    cloud::register(r);
    lines::register(r);
    curves::register(r);
    array::register(r);
}
