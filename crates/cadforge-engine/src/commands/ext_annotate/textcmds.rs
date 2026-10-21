//! Text commands: DDEDIT, FIND, SPELL, FIELD, SCALETEXT and JUSTIFYTEXT.
use super::{ps, spell, strings_of, tables, visit_strings};
use crate::commands::{def, run};
use crate::ctx::*;
use crate::registry::Registry;
use crate::runtime::*;
use cadforge_core::entities::*;
use cadforge_core::geom::*;
use cadforge_core::text::{text_line_origins, HJust, TextData, VJust};
use indexmap::IndexMap;
use std::collections::HashSet;

fn is_text(e: &Entity) -> bool {
    matches!(e.kind, Kind::Text(_))
}

// ---------------------------------------------------------------- DDEDIT

/// Attribute definitions: edit tag, prompt and default value.
async fn edit_attdef(c: &Ctx, id: u64, t: &TextData) -> CmdResult<()> {
    let Some(att) = &t.att else { return Ok(()) };
    let tag = c.string(ps("Enter new tag").default_str(&att.tag)).await?.text;
    let prompt = c.string(ps("Enter new prompt").default_str(&att.prompt).raw()).await?.text;
    let value = c.string(ps("Enter new default value").default_str(&t.s).raw()).await?.text;
    c.doc_mut(|d| {
        d.modify(id, |e| {
            if let Kind::Text(TextData { att: Some(a), s, .. }) = &mut e.kind {
                (a.tag, a.prompt, *s) = (tag, prompt, value);
            }
        })
    });
    Ok(())
}

async fn ddedit(c: Ctx, args: Args) -> CmdResult<()> {
    let from_ui = args.id("entity");
    loop {
        let (id, at) = match from_ui {
            Some(id) => (id, None),
            None => match c.entity(ps("Select an annotation object").allow_enter().filter(|e| matches!(e.kind, Kind::Text(_) | Kind::Dim(_) | Kind::Leader { .. } | Kind::Table(_)))).await? {
                EntR::Entity { id, pt, .. } => (id, Some(pt)),
                _ => return Ok(()),
            },
        };
        match c.entity_by_id(id).map(|e| e.kind) {
            Some(Kind::Text(t)) if t.att.is_some() => edit_attdef(&c, id, &t).await?,
            Some(Kind::Table(_)) => match at {
                Some(q) => tables::edit_cell_at(&c, id, q).await?,
                None => c.log_error("Pick a table cell with DDEDIT or use TABLEEDIT."),
            },
            Some(_) => c.sub_forced("TEXTEDIT", Args::none().with("entity", ArgVal::Id(id))).await,
            None => {}
        }
        if from_ui.is_some() {
            return Ok(());
        }
    }
}

// ---------------------------------------------------------------- FIND

/// Replace every occurrence of `find` in `s`; returns the new text and the number of replacements.
fn replace_all(s: &str, find: &str, repl: &str, case: bool, whole: bool) -> (String, usize) {
    let (text, pat): (Vec<char>, Vec<char>) = (s.chars().collect(), find.chars().collect());
    if pat.is_empty() {
        return (s.to_string(), 0);
    }
    let same = |a: char, b: char| a == b || (!case && a.to_lowercase().eq(b.to_lowercase()));
    let word = |c: char| c.is_alphanumeric() || c == '_';
    let (mut out, mut n, mut i) = (String::new(), 0, 0);
    while i < text.len() {
        let end = i + pat.len();
        let hit = end <= text.len() && text[i..end].iter().zip(&pat).all(|(a, b)| same(*a, *b)) && (!whole || ((i == 0 || !word(text[i - 1])) && (end == text.len() || !word(text[end]))));
        if hit {
            out.push_str(repl);
            (i, n) = (end, n + 1);
        } else {
            out.push(text[i]);
            i += 1;
        }
    }
    (out, n)
}

async fn find(c: Ctx, _a: Args) -> CmdResult<()> {
    let needle = c.string(ps("Enter text to find").raw()).await?.text;
    if needle.is_empty() {
        return Ok(());
    }
    let repl = c.string(ps("Enter replacement text (Enter to only find)").raw()).await?.text;
    let (mut case, mut whole, mut scope): (bool, bool, Option<Vec<u64>>) = (false, false, None);
    let replace = loop {
        let default = if repl.is_empty() { "Find" } else { "Replace all" };
        match c.keyword(ps("Enter an option").options(&["Case", "Whole word", "Select objects", "Find", "Replace all"]).default_str(default)).await?.kw() {
            Some("Case") => {
                case = !case;
                c.log(&format!("Match case: {}", if case { "on" } else { "off" }));
            }
            Some("Whole word") => {
                whole = !whole;
                c.log(&format!("Find whole words only: {}", if whole { "on" } else { "off" }));
            }
            Some("Select objects") => scope = Some(c.select(ps("Select objects to search")).await?.ents).filter(|ids| !ids.is_empty()),
            Some("Replace all") => break true,
            _ => break false,
        }
    };
    let count = |e: &Entity| strings_of(e).iter().map(|s| replace_all(s, &needle, "", case, whole).1).sum::<usize>();
    let in_scope = |e: &Entity| !scope.as_ref().is_some_and(|ids| !ids.contains(&e.id));
    if !replace {
        let hits: Vec<(&'static str, usize)> = c.doc(|d| d.model.entities.iter().chain(d.layouts.iter().flat_map(|l| l.entities.iter())).filter(|e| in_scope(e)).map(|e| (e.label(), count(e))).filter(|h| h.1 > 0).collect());
        let n: usize = hits.iter().map(|h| h.1).sum();
        c.log(&format!("{n} occurrence{} of \"{needle}\" found in {} object{}.", if n == 1 { "" } else { "s" }, hits.len(), if hits.len() == 1 { "" } else { "s" }));
        return Ok(());
    }
    let locked: HashSet<String> = c.doc(|d| d.layers.iter().filter(|l| l.locked).map(|l| l.name.clone()).collect());
    let (n, objects, skipped) = c.doc_mut(|d| {
        d.change(|d| {
            let (mut n, mut objects, mut skipped) = (0, 0, 0);
            for e in d.model.entities.iter_mut().chain(d.layouts.iter_mut().flat_map(|l| l.entities.iter_mut())) {
                if !in_scope(e) || count(e) == 0 {
                    continue;
                }
                if locked.contains(if e.layer.is_empty() { "0" } else { &e.layer }) {
                    skipped += 1;
                    continue;
                }
                objects += 1;
                visit_strings(e, &mut |s| {
                    let (text, k) = replace_all(s, &needle, &repl, case, whole);
                    *s = text;
                    n += k;
                });
            }
            (n, objects, skipped)
        })
    });
    c.log(&format!("{n} occurrence{} replaced in {objects} object{}.", if n == 1 { "" } else { "s" }, if objects == 1 { "" } else { "s" }));
    if skipped > 0 {
        c.log_error(&format!("{skipped} object{} on locked layers skipped.", if skipped == 1 { "" } else { "s" }));
    }
    Ok(())
}

// ---------------------------------------------------------------- SPELL

async fn spell_cmd(c: Ctx, _a: Args) -> CmdResult<()> {
    let sel = c.select(ps("Select objects to check (Enter for all text in the current space)")).await?;
    let ents = if sel.ents.is_empty() { c.doc(|d| d.entities().clone()) } else { c.entities(&sel.ents) };
    let mut unknown: IndexMap<String, (usize, &'static str)> = IndexMap::new();
    for e in &ents {
        for w in strings_of(e).iter().flat_map(|s| spell::words(s)).filter(|w| !spell::known(w)) {
            unknown.entry(w).or_insert((0, e.label())).0 += 1;
        }
    }
    for (w, (n, label)) in &unknown {
        c.log(&format!("Unknown word: \"{w}\" ({n} time{}, first in {label})", if *n == 1 { "" } else { "s" }));
    }
    c.log(&format!("Spelling check complete: {} unknown word{}.", unknown.len(), if unknown.len() == 1 { "" } else { "s" }));
    Ok(())
}

// ---------------------------------------------------------------- FIELD

/// Format a UTC time with the tokens yyyy yy MMMM MMM MM M dddd ddd dd d HH H mm ss.
fn format_utc(secs: i64, fmt: &str) -> String {
    const MONTHS: [&str; 12] = ["January", "February", "March", "April", "May", "June", "July", "August", "September", "October", "November", "December"];
    const DAYS: [&str; 7] = ["Sunday", "Monday", "Tuesday", "Wednesday", "Thursday", "Friday", "Saturday"];
    let (days, rem) = (secs.div_euclid(86400), secs.rem_euclid(86400));
    // civil date from days since 1970-01-01
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + (month <= 2) as i64;
    let (hour, minute, second) = (rem / 3600, rem % 3600 / 60, rem % 60);
    let weekday = DAYS[(days + 4).rem_euclid(7) as usize];
    let chars: Vec<char> = fmt.chars().collect();
    let mut out = String::new();
    let mut i = 0;
    while i < chars.len() {
        let ch = chars[i];
        let run = chars[i..].iter().take_while(|c| **c == ch).count();
        let text = match (ch, run) {
            ('y', 4..) => format!("{year:04}"),
            ('y', _) => format!("{:02}", year % 100),
            ('M', 4..) => MONTHS[month as usize - 1].to_string(),
            ('M', 3) => MONTHS[month as usize - 1][..3].to_string(),
            ('M', 2) => format!("{month:02}"),
            ('M', _) => month.to_string(),
            ('d', 4..) => weekday.to_string(),
            ('d', 3) => weekday[..3].to_string(),
            ('d', 2) => format!("{day:02}"),
            ('d', _) => day.to_string(),
            ('H', 2..) => format!("{hour:02}"),
            ('H', _) => hour.to_string(),
            ('m', 2..) => format!("{minute:02}"),
            ('m', _) => minute.to_string(),
            ('s', 2..) => format!("{second:02}"),
            ('s', _) => second.to_string(),
            _ => ch.to_string().repeat(run),
        };
        out.push_str(&text);
        i += run;
    }
    out
}

/// FIELD's Object type: a property of a picked object as text.
async fn object_field(c: &Ctx) -> CmdResult<Option<String>> {
    let Some(e) = c.entity(ps("Select an object")).await?.id().and_then(|id| c.entity_by_id(id)) else { return Ok(None) };
    let Some(prop) = c.keyword(ps("Enter object property").options(&["Area", "Length", "Radius", "Diameter", "Layer", "Contents"]).default_str("Area")).await?.kw().map(str::to_string) else { return Ok(None) };
    let prec = c.doc(|d| d.settings.units.precision.max(0) as usize);
    let num = |v: f64| cadforge_core::style::fmt_num(v, prec, false);
    let sweep = |a0: f64, a1: f64| Some(sweep_ccw(a0, a1)).filter(|s| *s > 1e-12).unwrap_or(TAU);
    let value = match (prop.as_str(), &e.kind) {
        ("Layer", _) => Some(if e.layer.is_empty() { "0".into() } else { e.layer.clone() }),
        ("Contents", _) => Some(strings_of(&e).join(" ")).filter(|s| !s.is_empty()),
        ("Radius", Kind::Circle { r, .. } | Kind::Arc { r, .. }) => Some(num(*r)),
        ("Diameter", Kind::Circle { r, .. } | Kind::Arc { r, .. }) => Some(num(2.0 * r)),
        ("Area", Kind::Circle { r, .. }) => Some(num(PI * r * r)),
        ("Area", Kind::Pline { v, closed: true }) => Some(num(pline_area(v).abs())),
        ("Length", Kind::Line { a, b }) => Some(num(dist(*a, *b))),
        ("Length", Kind::Circle { r, .. }) => Some(num(TAU * r)),
        ("Length", Kind::Arc { r, a0, a1, .. }) => Some(num(r * sweep(*a0, *a1))),
        ("Length", Kind::Pline { v, closed }) => Some(num(pline_length(v, *closed))),
        _ => None,
    };
    if value.is_none() {
        c.log_error(&format!("A {} has no {prop} value.", e.label().to_lowercase()));
    }
    Ok(value)
}

async fn field(c: Ctx, _a: Args) -> CmdResult<()> {
    let value = match c.keyword(ps("Enter field type").options(&["Date", "Filename", "Author", "Object"]).default_str("Date")).await?.kw() {
        Some("Date") => {
            let fmt = c.string(ps("Enter date format").default_str("yyyy-MM-dd")).await?.text;
            let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_secs() as i64);
            Some(format_utc(now, &fmt))
        }
        Some("Filename") => Some(c.doc(|d| d.file_name.clone().unwrap_or_else(|| d.name.clone())).rsplit(['/', '\\']).next().unwrap_or_default().to_string()),
        Some("Author") => std::env::var("USER").or_else(|_| std::env::var("USERNAME")).ok().filter(|u| !u.is_empty()).or_else(|| {
            c.log_error("The author could not be determined.");
            None
        }),
        Some("Object") => object_field(&c).await?,
        _ => None,
    };
    let Some(value) = value else { return Ok(()) };
    let Some(at) = c.point(ps("Specify start point of field text")).await?.pt() else { return Ok(()) };
    let h = c.app(|a| a.mem.text_height);
    c.add(Entity::text(at, h, &value));
    c.log("The field was inserted as evaluated text; it is not updated automatically.");
    Ok(())
}

// ---------------------------------------------------------------- SCALETEXT / JUSTIFYTEXT

const JUSTIFY: [(&str, HJust, VJust); 13] = [
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

fn justification(name: &str) -> Option<(HJust, VJust)> {
    JUSTIFY.iter().find(|j| j.0 == name).map(|j| (j.1, j.2))
}

/// The insertion point that gives the text the justification `jh`/`jv` while it stays where it is.
fn justified_origin(t: &TextData, jh: HJust, jv: VJust) -> Pt {
    let mut moved = t.clone();
    (moved.jh, moved.jv) = (jh, jv);
    t.p + (text_line_origins(t)[0].1 - text_line_origins(&moved)[0].1)
}

async fn justifytext(c: Ctx, _a: Args) -> CmdResult<()> {
    let ids = c.select(ps("Select objects").filter(is_text)).await?.ents;
    if ids.is_empty() {
        return Ok(());
    }
    let opts = ["Left", "Align", "Fit", "Center", "Middle", "Right", "TL", "TC", "TR", "ML", "MC", "MR", "BL", "BC", "BR"];
    let Some(name) = c.keyword(ps("Enter a justification option").options(&opts).default_str("Left")).await?.kw().map(str::to_string) else { return Ok(()) };
    let Some((jh, jv)) = justification(&name) else {
        c.log_error("Align and Fit justification stretch the text between two points and are not supported.");
        return Ok(());
    };
    c.doc_mut(|d| {
        d.change(|d| {
            for id in &ids {
                if let Some(Entity { kind: Kind::Text(t), .. }) = d.by_id_mut(*id) {
                    t.p = justified_origin(t, jh, jv);
                    (t.jh, t.jv) = (jh, jv);
                }
            }
        })
    });
    Ok(())
}

async fn scaletext(c: Ctx, _a: Args) -> CmdResult<()> {
    let ids = c.select(ps("Select objects").filter(is_text)).await?.ents;
    let Some(Kind::Text(first)) = ids.first().and_then(|i| c.entity_by_id(*i)).map(|e| e.kind) else { return Ok(()) };
    let opts = ["Existing", "Left", "Center", "Middle", "Right", "TL", "TC", "TR", "ML", "MC", "MR", "BL", "BC", "BR"];
    let Some(base) = c.keyword(ps("Enter a base point option for scaling").options(&opts).default_str("Existing")).await?.kw().map(str::to_string) else { return Ok(()) };
    let reply = c.distance(ps("Specify new height or").options(&["Match object", "Scale factor"]).default_num(first.h).min(0.0).nonzero()).await?;
    // each text gets either the new height or the scale factor
    #[derive(Clone, Copy)]
    enum Size {
        Height(f64),
        Factor(f64),
    }
    let size = match reply.kw() {
        Some("Match object") => {
            let Some(Kind::Text(t)) = c.entity(ps("Select a text object").filter(is_text)).await?.id().and_then(|i| c.entity_by_id(i)).map(|e| e.kind) else { return Ok(()) };
            Size::Height(t.h)
        }
        Some(_) => {
            let Some(f) = c.number(ps("Specify scale factor").default_num(1.0).min(0.0).nonzero()).await?.value() else { return Ok(()) };
            Size::Factor(f)
        }
        None => match reply.value() {
            Some(h) => Size::Height(h),
            None => return Ok(()),
        },
    };
    c.doc_mut(|d| {
        d.change(|d| {
            for id in &ids {
                if let Some(Entity { kind: Kind::Text(t), .. }) = d.by_id_mut(*id) {
                    let f = match size {
                        Size::Height(h) => h / t.h,
                        Size::Factor(f) => f,
                    };
                    let anchor = justification(&base).map_or(t.p, |(jh, jv)| justified_origin(t, jh, jv));
                    t.p = anchor + (t.p - anchor) * f;
                    t.h *= f;
                    t.w = t.w.map(|w| w * f);
                }
            }
        })
    });
    Ok(())
}

pub fn register(r: &mut Registry) {
    r.define(def("Annotation", "DDEDIT", run(ddedit)).no_pickfirst().title("Edit Text").desc("Edits single-line text, multiline text, dimension text, table cells and attribute definitions."));
    r.define(def("Annotation", "FIND", run(find)).no_pickfirst().title("Find and Replace").desc("Finds and replaces text in text, dimensions, leaders, tables and attributes."));
    r.define(def("Annotation", "SPELL", run(spell_cmd)).title("Check Spelling").desc("Checks spelling in a drawing against a built-in word list."));
    r.define(def("Annotation", "FIELD", run(field)).no_pickfirst().title("Field").desc("Creates text with an evaluated field value."));
    r.define(def("Modify", "SCALETEXT", run(scaletext)).title("Scale Text").desc("Enlarges or reduces selected text objects without changing their locations."));
    r.define(def("Modify", "JUSTIFYTEXT", run(justifytext)).title("Justify Text").desc("Changes the justification point of selected text objects without changing their locations."));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn replacing_respects_case_and_whole_words() {
        assert_eq!(replace_all("Door door DOOR doors", "door", "gate", false, false), ("gate gate gate gates".into(), 4));
        assert_eq!(replace_all("Door door DOOR doors", "door", "gate", true, false), ("Door gate DOOR gates".into(), 2));
        assert_eq!(replace_all("Door door DOOR doors", "door", "gate", false, true), ("gate gate gate doors".into(), 3));
        assert_eq!(replace_all("a.b a_b", "a", "x", false, true), ("x.b a_b".into(), 1), "underscores belong to words");
        assert_eq!(replace_all("aaa", "aa", "b", true, false), ("ba".into(), 1), "matches do not overlap");
        assert_eq!(replace_all("abc", "", "x", true, false), ("abc".into(), 0));
        assert_eq!(replace_all("Straße", "STRASSE", "x", false, false).1, 0);
        assert_eq!(replace_all("x", "x", "", true, false), (String::new(), 1), "replacing with nothing deletes");
    }

    #[test]
    fn dates_are_formatted_in_utc() {
        // 2024-03-05 07:08:09 UTC was a Tuesday
        let t = 1_709_622_489;
        assert_eq!(format_utc(t, "yyyy-MM-dd"), "2024-03-05");
        assert_eq!(format_utc(t, "d/M/yy HH:mm:ss"), "5/3/24 07:08:09");
        assert_eq!(format_utc(t, "dddd, MMMM d, yyyy"), "Tuesday, March 5, 2024");
        assert_eq!(format_utc(t, "ddd MMM dd"), "Tue Mar 05");
        assert_eq!(format_utc(0, "yyyy-MM-dd"), "1970-01-01");
        assert_eq!(format_utc(951_782_400, "yyyy-MM-dd"), "2000-02-29", "leap day");
        assert_eq!(format_utc(-86_400, "yyyy-MM-dd"), "1969-12-31");
    }

    #[test]
    fn justification_keeps_the_text_where_it_is() {
        let mut t = TextData::new(pt(10.0, 20.0), 2.5, "Hello");
        let w = cadforge_core::text::measure_text("Hello", 2.5);
        let p = justified_origin(&t, HJust::Center, VJust::Baseline);
        assert!((p.x - (10.0 + w / 2.0)).abs() < 1e-9 && (p.y - 20.0).abs() < 1e-9);
        t.rot = PI / 2.0;
        let p = justified_origin(&t, HJust::Right, VJust::Baseline);
        assert!((p.x - 10.0).abs() < 1e-9 && (p.y - (20.0 + w)).abs() < 1e-9, "{p:?}");
        let q = justified_origin(&TextData::new(pt(0.0, 0.0), 2.0, "x"), HJust::Left, VJust::Middle);
        assert!((q.y - 1.0).abs() < 1e-9, "middle is half a height above the baseline: {q:?}");
    }
}
