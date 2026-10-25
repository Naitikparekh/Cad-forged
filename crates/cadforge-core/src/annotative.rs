//! Annotative objects. Text, leaders, dimensions, hatches and block references can be annotative: they keep their
//! size on paper (`Annotative::paper`, the size at 1:1) and are sized for the drawing's current annotation scale
//! (CANNOSCALE) in model space. A layout viewport draws them at its own scale, so a note is the same height on the
//! sheet in a 1:50 and a 1:100 viewport. Each object also lists the annotation scales it supports (OBJECTSCALE).
use crate::drawing::Drawing;
use crate::entities::{Entity, Kind};
use serde::{Deserialize, Serialize};

/// What makes an object annotative.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Annotative {
    /// The object's size at 1:1: text or leader height, hatch pattern scale, block or dimension scale.
    pub paper: f64,
    /// The annotation scales the object supports, by name.
    #[serde(default)]
    pub scales: Vec<String>,
}

/// One entry of the drawing's scale list: `paper` units on the sheet show `drawing` units of the model.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct AnnoScale {
    pub name: String,
    pub paper: f64,
    pub drawing: f64,
}

impl AnnoScale {
    pub fn new(name: &str, paper: f64, drawing: f64) -> AnnoScale {
        AnnoScale { name: name.into(), paper, drawing }
    }
    /// Model units per paper unit: an annotative object's model size is its paper size times this.
    pub fn factor(&self) -> f64 {
        if self.paper > 0.0 {
            self.drawing / self.paper
        } else {
            1.0
        }
    }
}

/// AutoCAD's default metric scale list.
pub fn default_scales() -> Vec<AnnoScale> {
    let mut v: Vec<AnnoScale> = [1.0, 2.0, 4.0, 5.0, 8.0, 10.0, 16.0, 20.0, 30.0, 40.0, 50.0, 100.0].iter().map(|d| AnnoScale::new(&format!("1:{d}"), 1.0, *d)).collect();
    v.extend([2.0, 4.0, 8.0, 10.0, 100.0].iter().map(|p| AnnoScale::new(&format!("{p}:1"), *p, 1.0)));
    v
}

/// "1:50", "2:1", "1/4\" = 1'-0\"" style names: a parsed `paper:drawing` (or `paper=drawing`) pair.
pub fn parse_scale(s: &str) -> Option<(f64, f64)> {
    let (a, b) = s.split_once([':', '='])?;
    let (p, d) = (a.trim().parse::<f64>().ok()?, b.trim().parse::<f64>().ok()?);
    (p > 0.0 && d > 0.0 && p.is_finite() && d.is_finite()).then_some((p, d))
}

/// The size an annotative object scales (see [`Annotative::paper`]); `None` for kinds that cannot be annotative.
pub fn size_of(e: &Entity) -> Option<f64> {
    match &e.kind {
        Kind::Text(t) => Some(t.h),
        Kind::Leader { h, .. } => Some(*h),
        Kind::Hatch(h) if h.gradient.is_none() => Some(h.ps),
        Kind::Insert(i) => Some(i.sx),
        Kind::Dim(d) => Some(d.sc.unwrap_or(1.0)),
        _ => None,
    }
}

/// Scale the object's annotation size by `k` about its own anchor (text and attributes keep their insertion points
/// relative to the block's).
pub fn scale_size(e: &mut Entity, k: f64) {
    if !(k.is_finite() && k > 0.0) || (k - 1.0).abs() < 1e-15 {
        return;
    }
    match &mut e.kind {
        Kind::Text(t) => {
            t.h *= k;
            t.w = t.w.map(|w| w * k);
        }
        Kind::Leader { h, .. } => *h *= k,
        Kind::Hatch(h) => h.ps *= k,
        Kind::Insert(i) => {
            i.sx *= k;
            i.sy *= k;
            for a in &mut i.attribs {
                a.t.p = i.p + (a.t.p - i.p) * k;
                a.t.h *= k;
                a.t.w = a.t.w.map(|w| w * k);
            }
        }
        Kind::Dim(d) => d.sc = Some(d.sc.unwrap_or(1.0) * k),
        _ => {}
    }
}

/// The object as drawn at annotation scale factor `factor` (when it is annotative), otherwise unchanged.
pub fn at_factor(e: &Entity, factor: f64) -> std::borrow::Cow<'_, Entity> {
    match (&e.anno, size_of(e)) {
        (Some(a), Some(cur)) if cur > 0.0 && (a.paper * factor - cur).abs() > 1e-12 * cur.max(1.0) => {
            let mut c = e.clone();
            scale_size(&mut c, a.paper * factor / cur);
            std::borrow::Cow::Owned(c)
        }
        _ => std::borrow::Cow::Borrowed(e),
    }
}

impl Drawing {
    /// The scale-list entry named `name` (case and spaces ignored).
    pub fn anno_scale(&self, name: &str) -> Option<&AnnoScale> {
        let key: String = name.chars().filter(|c| !c.is_whitespace()).collect::<String>().to_lowercase();
        self.settings.scales.iter().find(|s| s.name.chars().filter(|c| !c.is_whitespace()).collect::<String>().to_lowercase() == key)
    }

    /// Model units per paper unit at the current annotation scale (CANNOSCALE).
    pub fn anno_factor(&self) -> f64 {
        self.anno_scale(&self.settings.cannoscale).map_or(1.0, AnnoScale::factor)
    }

    /// Make an object annotative (keeping its current size at the current annotation scale) or not. Returns false
    /// for objects that cannot be annotative.
    pub fn set_annotative(&mut self, id: u64, on: bool) -> bool {
        let (factor, cur) = (self.anno_factor(), self.settings.cannoscale.clone());
        let mut ok = false;
        self.modify(id, |e| {
            let Some(size) = size_of(e) else { return };
            ok = true;
            e.anno = on.then(|| e.anno.clone().unwrap_or(Annotative { paper: size / factor, scales: vec![cur.clone()] }));
        });
        ok
    }

    /// Change the current annotation scale: every annotative object is sized for it. An object that does not support
    /// the scale yet gets it added when `add_to_objects` is set (ANNOAUTOSCALE). Returns how many objects changed.
    pub fn set_annotation_scale(&mut self, name: &str, add_to_objects: bool) -> Result<usize, String> {
        let s = self.anno_scale(name).cloned().ok_or_else(|| format!("\"{name}\" is not in the scale list."))?;
        self.will_change();
        self.settings.cannoscale = s.name.clone();
        let n = self.anno_resize(s.factor(), add_to_objects.then_some(&s.name));
        self.touch();
        Ok(n)
    }

    /// Size every annotative object for the current annotation scale (ANNOUPDATE / ANNORESET). Returns how many
    /// changed.
    pub fn anno_update(&mut self) -> usize {
        let f = self.anno_factor();
        self.will_change();
        let n = self.anno_resize(f, None);
        self.touch();
        n
    }

    fn anno_resize(&mut self, factor: f64, add: Option<&String>) -> usize {
        let mut n = 0;
        for e in self.all_entities_mut() {
            let (Some(a), Some(cur)) = (e.anno.clone(), size_of(e)) else { continue };
            if cur > 0.0 && (a.paper * factor - cur).abs() > 1e-12 * cur.max(1.0) {
                scale_size(e, a.paper * factor / cur);
                n += 1;
            }
            if let (Some(name), Some(an)) = (add, e.anno.as_mut()) {
                if !an.scales.iter().any(|s| s.eq_ignore_ascii_case(name)) {
                    an.scales.push(name.clone());
                }
            }
        }
        n
    }
}

/// The factor a layout viewport of scale `vp_scale` (paper per model unit) draws annotative objects at.
pub fn viewport_factor(vp_scale: f64) -> f64 {
    if vp_scale > 0.0 && vp_scale.is_finite() {
        1.0 / vp_scale
    } else {
        1.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::entities::Entity;
    use crate::geom::pt;

    #[test]
    fn scales_parse_and_factor() {
        assert_eq!(parse_scale("1:50"), Some((1.0, 50.0)));
        assert_eq!(parse_scale(" 2 = 1 "), Some((2.0, 1.0)));
        assert_eq!(parse_scale("1:0"), None);
        assert_eq!(AnnoScale::new("1:50", 1.0, 50.0).factor(), 50.0);
        assert!(default_scales().iter().any(|s| s.name == "1:100"));
    }

    #[test]
    fn annotative_text_follows_the_annotation_scale() {
        let mut d = Drawing::new("t");
        let id = d.add(Entity::text(pt(0.0, 0.0), 2.5, "note"));
        assert!(d.set_annotative(id, true));
        assert_eq!(d.by_id(id).unwrap().anno.as_ref().map(|a| a.paper), Some(2.5));
        assert_eq!(d.set_annotation_scale("1:50", false), Ok(1));
        let Kind::Text(t) = &d.by_id(id).unwrap().kind else { panic!() };
        assert_eq!(t.h, 125.0);
        let shown = at_factor(d.by_id(id).unwrap(), 20.0);
        let Kind::Text(t) = &shown.kind else { panic!() };
        assert_eq!(t.h, 50.0, "drawn at 1:20 in a viewport");
        assert!(d.set_annotation_scale("3:7", false).is_err());
    }
}
