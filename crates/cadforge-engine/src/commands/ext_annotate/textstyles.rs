//! -STYLE: create or change a text style from the command line (the STYLE dialog does the same): its font, fixed
//! height, width factor and oblique angle. The style becomes current, and every text in it is drawn again.
use super::ps;
use crate::commands::{def, run};
use crate::ctx::*;
use crate::registry::Registry;
use crate::runtime::*;
use cadforge_core::style::TextStyle;

/// The current text style's name for new text (None for Standard) and its fixed height (0: none).
pub fn current_style(c: &Ctx) -> (Option<String>, f64) {
    c.doc(|d| {
        let st = d.settings.cur_text();
        ((!st.name.eq_ignore_ascii_case("Standard")).then(|| st.name.clone()), st.height)
    })
}

/// Define or replace a text style, make it current and redraw the text in it.
pub fn put_style(c: &Ctx, st: TextStyle) {
    c.doc_mut(|d| {
        d.change(|d| {
            d.settings.text_styles.retain(|k, _| !k.eq_ignore_ascii_case(&st.name));
            d.settings.text_style = st.name.clone();
            d.settings.text_styles.insert(st.name.clone(), st);
            d.sync_text_styles();
        })
    });
}

async fn style_cli(c: Ctx, a: Args) -> CmdResult<()> {
    let cur = c.doc(|d| d.settings.cur_text());
    let name = match a.str("name") {
        Some(n) => n.to_string(),
        None => loop {
            let n = c.string(ps("Enter name of text style or [?]").default_str(&cur.name)).await?.text;
            if n.trim() != "?" {
                break n.trim().to_string();
            }
            let all = c.doc(|d| d.settings.all_text_styles());
            let lines: Vec<String> = all.iter().map(|s| format!("  {}: {}{}{}{}", s.name, s.font, if s.height > 0.0 { format!(", height {}", s.height) } else { String::new() }, if (s.width - 1.0).abs() > 1e-9 { format!(", width {}", s.width) } else { String::new() }, if s.oblique != 0.0 { format!(", oblique {}", s.oblique) } else { String::new() })).collect();
            c.log(&format!("Text styles:\n{}", lines.join("\n")));
        },
    };
    if name.is_empty() {
        return Ok(());
    }
    let old = c.doc(|d| d.settings.text_style_named(&name));
    let font = match a.str("font") {
        Some(f) => f.to_string(),
        None => {
            let f = c.string(ps("Specify font name (a TrueType font such as Arial, or an SHX font such as romans.shx)").default_str(&old.font)).await?.text;
            if f.trim().is_empty() { old.font.clone() } else { f.trim().to_string() }
        }
    };
    let height = match a.num("height") {
        Some(h) => h,
        None => match c.distance(ps("Specify height of text (0 for a height chosen per text)").default_num(old.height).min(0.0)).await?.value() {
            Some(h) => h,
            None => return Ok(()),
        },
    };
    let width = match a.num("width") {
        Some(w) => w,
        None => match c.number(ps("Specify width factor").default_num(old.width).min(0.01)).await?.value() {
            Some(w) => w,
            None => return Ok(()),
        },
    };
    let oblique = match a.num("oblique") {
        Some(o) => o,
        None => match c.number(ps("Specify obliquing angle").default_num(old.oblique)).await?.value() {
            Some(o) => o,
            None => return Ok(()),
        },
    };
    if !(-85.0..=85.0).contains(&oblique) {
        c.log_error("The obliquing angle must be between -85 and 85 degrees.");
        return Ok(());
    }
    let display = name.clone();
    put_style(&c, TextStyle { name, font, height, width, oblique });
    c.log(&format!("Text style \"{display}\" is current."));
    Ok(())
}

pub fn register(r: &mut Registry) {
    r.define(def("Annotation", "-STYLE", run(style_cli)).aliases(&["-ST"]).title("Text Style (command line)").icon("textstyle").desc("Creates or changes a text style from the command line: font, height, width factor and obliquing angle."));
}
