//! Design tokens (AutoCAD-style dark theme and a light theme) and egui visuals / fonts.
use eframe::egui::{self, Color32, FontData, FontDefinitions, FontFamily};
use std::sync::Arc;

const fn c(hex: u32) -> Color32 {
    Color32::from_rgb((hex >> 16) as u8, (hex >> 8) as u8, hex as u8)
}

#[derive(Clone, Copy, Debug)]
pub struct Tokens {
    pub dark: bool,
    pub canvas: Color32,
    pub chrome: Color32,
    pub chrome2: Color32,
    pub ribbon: Color32,
    pub ribbon_title: Color32,
    pub raised: Color32,
    pub sep: Color32,
    pub border: Color32,
    pub text: Color32,
    pub text2: Color32,
    pub text3: Color32,
    pub accent: Color32,
    pub accent2: Color32,
    pub accent_soft: Color32,
    pub hover: Color32,
    pub press: Color32,
    pub input: Color32,
    pub warn: Color32,
    pub err: Color32,
    pub ok: Color32,
    pub ic_l: Color32,
    pub ic_k: Color32,
    pub ic_a: Color32,
    pub ic_o: Color32,
    pub ic_g: Color32,
}

pub const DARK: Tokens = Tokens {
    dark: true,
    canvas: c(0x212830),
    chrome: c(0x1b1f24),
    chrome2: c(0x23282e),
    ribbon: c(0x353b43),
    ribbon_title: c(0x2c3137),
    raised: c(0x3d444d),
    sep: c(0x48505a),
    border: c(0x3c434b),
    text: c(0xe9edf1),
    text2: c(0xadb6c0),
    text3: c(0x7f8a95),
    accent: c(0x2b8fe0),
    accent2: c(0x5bb8ff),
    accent_soft: c(0x24496b),
    hover: c(0x464f5a),
    press: c(0x2d6396),
    input: c(0x14181c),
    warn: c(0xffb454),
    err: c(0xff7f74),
    ok: c(0x7ddc92),
    ic_l: c(0xd9dfe6),
    ic_k: c(0x7f8b98),
    ic_a: c(0x55b5ff),
    ic_o: c(0xffae42),
    ic_g: c(0x7bd88f),
};

pub const LIGHT: Tokens = Tokens {
    dark: false,
    canvas: c(0xffffff),
    chrome: c(0xdfe3e8),
    chrome2: c(0xeceff2),
    ribbon: c(0xf6f7f9),
    ribbon_title: c(0xe9ecf0),
    raised: c(0xffffff),
    sep: c(0xc7cdd4),
    border: c(0xc3c9d0),
    text: c(0x1c2229),
    text2: c(0x4c5864),
    text3: c(0x76828e),
    accent: c(0x0b73c7),
    accent2: c(0x0b73c7),
    accent_soft: c(0xcfe4f7),
    hover: c(0xdbe6f1),
    press: c(0xbcd6ee),
    input: c(0xffffff),
    warn: c(0xb8660a),
    err: c(0xc43b2f),
    ok: c(0x1d8a3b),
    ic_l: c(0x3a4651),
    ic_k: c(0x8b97a3),
    ic_a: c(0x0b73c7),
    ic_o: c(0xd9741a),
    ic_g: c(0x1f9a4a),
};

pub fn install_fonts(ctx: &egui::Context) {
    let mut fonts = FontDefinitions::default();
    let add = |fonts: &mut FontDefinitions, name: &str, bytes: &'static [u8]| {
        fonts
            .font_data
            .insert(name.to_owned(), Arc::new(FontData::from_static(bytes)));
    };
    add(
        &mut fonts,
        "inter",
        include_bytes!("../assets/fonts/Inter-Regular.otf"),
    );
    add(
        &mut fonts,
        "inter-semibold",
        include_bytes!("../assets/fonts/Inter-SemiBold.otf"),
    );
    add(
        &mut fonts,
        "liberation",
        include_bytes!("../assets/fonts/LiberationSans-Regular.ttf"),
    );
    add(
        &mut fonts,
        "liberation-bold",
        include_bytes!("../assets/fonts/LiberationSans-Bold.ttf"),
    );
    add(
        &mut fonts,
        "dejavu",
        include_bytes!("../assets/fonts/DejaVuSans.ttf"),
    );
    let prop = fonts.families.entry(FontFamily::Proportional).or_default();
    prop.insert(0, "inter".to_owned());
    prop.push("dejavu".to_owned());
    fonts.families.insert(
        FontFamily::Name("bold".into()),
        vec!["inter-semibold".into(), "dejavu".into()],
    );
    fonts.families.insert(
        FontFamily::Name("cad".into()),
        vec!["liberation".into(), "dejavu".into()],
    );
    fonts.families.insert(
        FontFamily::Name("cadbold".into()),
        vec!["liberation-bold".into(), "dejavu".into()],
    );
    fonts
        .families
        .entry(FontFamily::Monospace)
        .or_default()
        .push("dejavu".to_owned());
    ctx.set_fonts(fonts);
}

pub fn apply(ctx: &egui::Context, t: &Tokens) {
    let mut v = if t.dark {
        egui::Visuals::dark()
    } else {
        egui::Visuals::light()
    };
    v.panel_fill = t.chrome;
    v.window_fill = t.ribbon;
    v.extreme_bg_color = t.input;
    v.faint_bg_color = t.chrome2;
    v.window_stroke = egui::Stroke::new(1.0, t.border);
    v.override_text_color = Some(t.text);
    v.selection.bg_fill = t.accent_soft;
    v.selection.stroke = egui::Stroke::new(1.0, t.accent2);
    v.hyperlink_color = t.accent2;
    let r = egui::CornerRadius::same(2);
    for w in [
        &mut v.widgets.noninteractive,
        &mut v.widgets.inactive,
        &mut v.widgets.hovered,
        &mut v.widgets.active,
        &mut v.widgets.open,
    ] {
        w.corner_radius = r;
    }
    v.widgets.noninteractive.bg_fill = t.chrome2;
    v.widgets.noninteractive.bg_stroke = egui::Stroke::new(1.0, t.border);
    v.widgets.noninteractive.fg_stroke = egui::Stroke::new(1.0, t.text2);
    v.widgets.inactive.bg_fill = t.raised;
    v.widgets.inactive.weak_bg_fill = t.raised;
    v.widgets.inactive.bg_stroke = egui::Stroke::new(1.0, t.sep);
    v.widgets.inactive.fg_stroke = egui::Stroke::new(1.0, t.text);
    v.widgets.hovered.bg_fill = t.hover;
    v.widgets.hovered.weak_bg_fill = t.hover;
    v.widgets.hovered.bg_stroke = egui::Stroke::new(1.0, t.accent);
    v.widgets.hovered.fg_stroke = egui::Stroke::new(1.0, t.text);
    v.widgets.active.bg_fill = t.press;
    v.widgets.active.weak_bg_fill = t.press;
    v.widgets.active.bg_stroke = egui::Stroke::new(1.0, t.accent2);
    v.widgets.active.fg_stroke = egui::Stroke::new(1.0, t.text);
    v.widgets.open.bg_fill = t.accent_soft;
    v.widgets.open.weak_bg_fill = t.accent_soft;
    ctx.set_visuals_of(
        if t.dark {
            egui::Theme::Dark
        } else {
            egui::Theme::Light
        },
        v,
    );
    ctx.set_theme(if t.dark {
        egui::Theme::Dark
    } else {
        egui::Theme::Light
    });
    ctx.all_styles_mut(|s| {
        s.spacing.item_spacing = egui::vec2(6.0, 4.0);
        s.spacing.button_padding = egui::vec2(8.0, 3.0);
        s.spacing.interact_size.y = 20.0;
        s.text_styles
            .insert(egui::TextStyle::Body, egui::FontId::proportional(12.5));
        s.text_styles
            .insert(egui::TextStyle::Button, egui::FontId::proportional(12.5));
        s.text_styles
            .insert(egui::TextStyle::Small, egui::FontId::proportional(11.0));
        s.text_styles
            .insert(egui::TextStyle::Monospace, egui::FontId::monospace(12.5));
        s.text_styles.insert(
            egui::TextStyle::Heading,
            egui::FontId::new(15.0, FontFamily::Name("bold".into())),
        );
    });
}
