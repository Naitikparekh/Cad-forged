//! Application shell: owns the engine [`Handle`], UI state and lays the window out.
use crate::data::RibbonData;
use crate::dialogs::Dialog;
use crate::icons::Icons;
use crate::menu::Popup;
use crate::palettes::PalState;
use crate::platform::Inbox;
use crate::theme::{self, Tokens};
use crate::viewport::ViewportState;
use cadforge_core::style::Color;
use cadforge_engine::app::{App, Event, Handle};
use cadforge_engine::commands::build_registry;
use eframe::egui;
use std::collections::HashSet;
use std::path::PathBuf;
use std::rc::Rc;

/// Current text typed at the command line (with a caret) and its history recall / suggestion state.
#[derive(Default)]
pub struct CmdLine {
    pub text: String,
    pub caret: usize,
    /// Typed through the dynamic-input route (shown at the cursor).
    pub dynamic: bool,
    pub recall: Option<usize>,
    pub past: Vec<String>,
    pub sugs: Vec<String>,
    pub sug_idx: Option<usize>,
    pub lines: usize,
    pub lines_f: f32,
}

pub struct UiState {
    pub ribbon_tab: String,
    pub ribbon_mode: u8,
    pub ribbon_hidden: HashSet<String>,
    pub flyout: Option<String>,
    pub dark: bool,
    pub workspace: String,
    pub cmd: CmdLine,
    pub show_props: bool,
    pub show_ribbon: bool,
    /// Palettes (properties, tool palettes, layers, command line) that HIDEPALETTES put away.
    pub hidden_pals: Option<[bool; 4]>,
    pub show_tools: bool,
    pub show_cmd: bool,
    pub show_menubar: bool,
    pub clean: bool,
    pub status_hidden: HashSet<String>,
    pub coord_relative: bool,
    pub search: String,
    pub showing_start: bool,
    pub toast: Option<(String, f64)>,
    pub prev_tab: Option<String>,
    pub text_window: bool,
    pub slideout: Option<String>,
    pub flyout_panel: Option<(String, egui::Rect)>,
    pub layer_dd: Option<egui::Rect>,
    pub layer_dd_frame: u64,
    pub search_focus: bool,
    pub app_menu: Option<(egui::Rect, Option<usize>)>,
}

pub struct CadForgeApp {
    pub h: Handle,
    pub t: Tokens,
    pub icons: Icons,
    pub rib: RibbonData,
    pub ui: UiState,
    pub vp: ViewportState,
    pub popup: Option<Popup>,
    pub time: f64,
    /// Dialog to open (moved onto the dialog stack on the next frame).
    pub dialog: Option<Dialog>,
    pub dialogs: Vec<Dialog>,
    pub dialog_counter: u64,
    pub token_counter: u32,
    pub ret_color: Option<(u32, Option<Color>)>,
    pub ret_text: Option<(String, String)>,
    pub ret_lw: Option<Option<i32>>,
    pub inbox: Inbox,
    pub save_dir: PathBuf,
    pub context_menu_pending: Option<(egui::Pos2, bool, bool)>,
    pub shift_prev: bool,
    /// The customization revision last stored (see `cadforge_engine::cui`).
    pub cui_saved: u64,
    pub last_cmd_row: Option<egui::Rect>,
    pub pal: PalState,
    pub pending_theme: Option<bool>,
    pub save_as_focused: bool,
    /// The linked data file DATALINKUPDATE asked for (a file chosen for it is read as this one).
    pub datalink_file: Option<String>,
    /// Set when input changed state after the panels were drawn: schedules one more frame.
    pub input_dirty: bool,
    /// Viewport rectangle shared with the web test hook.
    pub rect_hook: std::rc::Rc<std::cell::RefCell<Option<[f32; 4]>>>,
}

impl CadForgeApp {
    pub fn new(cc: &eframe::CreationContext<'_>) -> CadForgeApp {
        theme::install_fonts(&cc.egui_ctx);
        cadforge_core::text::set_measure(cadforge_render::helvetica_width);
        let t = theme::DARK;
        theme::apply(&cc.egui_ctx, &t);
        let registry = Rc::new(build_registry());
        let mut app = App::new(registry);
        app.ui.dialogs = true;
        app.autosave.salt = crate::platform::session_salt();
        let h = Handle::new(app);
        {
            let mut a = h.borrow_mut();
            a.new_drawing(None, false);
            a.log("CadForge 2.0 — type a command, or choose one from the ribbon. Press F1 for help.", "info");
        }
        h.pump();
        let rib = RibbonData::load();
        let mut app = CadForgeApp {
            h,
            t,
            icons: Icons::load(),
            rib,
            ui: UiState {
                ribbon_tab: "home".into(),
                ribbon_mode: 0,
                ribbon_hidden: HashSet::new(),
                flyout: None,
                dark: true,
                workspace: "Drafting & Annotation".into(),
                cmd: CmdLine { lines: 3, lines_f: 3.0, ..Default::default() },
                show_props: false,
                show_ribbon: true,
                hidden_pals: None,
                show_tools: false,
                show_cmd: true,
                show_menubar: false,
                clean: false,
                status_hidden: HashSet::new(),
                coord_relative: false,
                search: String::new(),
                showing_start: false,
                toast: None,
                prev_tab: None,
                text_window: false,
                slideout: None,
                flyout_panel: None,
                layer_dd: None,
                layer_dd_frame: 0,
                search_focus: false,
                app_menu: None,
            },
            vp: ViewportState::default(),
            popup: None,
            time: 0.0,
            dialog: None,
            dialogs: vec![],
            dialog_counter: 0,
            token_counter: 0,
            ret_color: None,
            ret_text: None,
            ret_lw: None,
            inbox: Default::default(),
            save_dir: std::env::current_dir().unwrap_or_default(),
            context_menu_pending: None,
            shift_prev: false,
            cui_saved: 0,
            last_cmd_row: None,
            pal: PalState::default(),
            pending_theme: None,
            save_as_focused: false,
            datalink_file: None,
            input_dirty: false,
            rect_hook: Default::default(),
        };
        #[cfg(target_arch = "wasm32")]
        crate::install_hook(app.h.clone(), app.rect_hook.clone());
        app.load_user_aliases();
        app.load_customization();
        app.recover_autosaved();
        app
    }

    /// Reopen the drawings a previous session left unsaved (its recovery files), as unsaved tabs.
    fn recover_autosaved(&mut self) {
        let mut names = vec![];
        for (key, json) in crate::platform::autosave_list() {
            match self.h.borrow_mut().recover_autosave(&key, &json) {
                Ok(name) => names.push(name),
                Err(_) => crate::platform::autosave_remove(&key),
            }
        }
        if !names.is_empty() {
            let msg = format!("Recovered {} unsaved drawing{} from the last session: {}.", names.len(), if names.len() == 1 { "" } else { "s" }, names.join(", "));
            self.h.borrow_mut().log(&msg, "info");
            self.toast(&msg);
        }
    }

    /// Run the autosave timer; called every frame, cheap until a pass is due.
    fn autosave(&mut self, ctx: &egui::Context) {
        let plan = self.h.borrow_mut().autosave_tick(false);
        if !plan.write.is_empty() || !plan.remove.is_empty() {
            let failed = crate::platform::autosave_apply(&plan);
            if failed > 0 {
                self.toast("Autosave could not store a recovery file (storage is full).");
            }
        }
        // the timer needs a frame now and then even when nothing is happening
        ctx.request_repaint_after(std::time::Duration::from_secs(20));
    }

    /// The customization stored by an earlier session.
    pub fn load_customization(&mut self) {
        if let Some(c) = crate::platform::store_get("cui").and_then(|s| serde_json::from_str::<cadforge_engine::cui::CuiState>(&s).ok()) {
            self.h.borrow_mut().cui = c;
        }
    }

    /// Store the customization when it has changed.
    fn store_customization(&mut self) {
        let rev = self.h.borrow().cui.rev;
        if rev != self.cui_saved {
            self.cui_saved = rev;
            let json = serde_json::to_string(&self.h.borrow().cui).unwrap_or_default();
            crate::platform::store_set("cui", &json);
        }
    }

    pub fn toast(&mut self, msg: &str) {
        self.ui.toast = Some((msg.to_string(), self.time + 3.2));
    }

    pub fn set_dark(&mut self, ctx: &egui::Context, dark: bool) {
        self.ui.dark = dark;
        self.t = if dark { theme::DARK } else { theme::LIGHT };
        theme::apply(ctx, &self.t);
        let _ = self.h.borrow_mut().setvar("THEME", if dark { "dark" } else { "light" });
    }

    /// Start a command from the UI (ribbon, menu, QAT): cancels whatever is running.
    pub fn run_command(&mut self, name: &str, args: cadforge_engine::runtime::Args) {
        if self.h.borrow().active.is_none() {
            return;
        }
        self.popup = None;
        self.h.start(name, args);
    }

    pub fn escape(&mut self) {
        self.ui.cmd = CmdLine { past: std::mem::take(&mut self.ui.cmd.past), lines: self.ui.cmd.lines, ..Default::default() };
        if self.vp.realtime.is_some() {
            self.end_realtime();
            return;
        }
        self.popup = None;
        self.h.escape();
    }

    fn process_events(&mut self, ctx: &egui::Context) {
        let events = self.h.borrow_mut().take_events();
        for ev in events {
            match ev {
                Event::Palette { name, show } => self.toggle_palette(&name, show),
                Event::UiAction { name, args } => self.ui_action(ctx, &name, args),
                Event::HatchSession => self.hatch_session_changed(),
                Event::Export(ents) => self.export_entities(ents),
                Event::ExportMesh(fmt) => self.export_mesh(&fmt),
                Event::SaveFile { name, data, mime } => self.deliver_file(&name, data, &mime),
                _ => {}
            }
        }
    }

    pub fn toggle_palette(&mut self, name: &str, show: Option<bool>) {
        match name {
            // HIDEPALETTES / SHOWPALETTES: remember what was open and bring it back
            "hide-all" => {
                if self.ui.hidden_pals.is_none() {
                    self.ui.hidden_pals = Some([self.ui.show_props, self.ui.show_tools, self.pal.lm_open, self.ui.show_cmd]);
                    (self.ui.show_props, self.ui.show_tools, self.pal.lm_open, self.ui.show_cmd) = (false, false, false, false);
                }
                return;
            }
            "blocks" => {
                self.ui.show_tools = true;
                self.pal.tool_tab = usize::MAX;
                return;
            }
            "show-all" => {
                if let Some([p, t, l, c]) = self.ui.hidden_pals.take() {
                    (self.ui.show_props, self.ui.show_tools, self.pal.lm_open, self.ui.show_cmd) = (p, t, l, c);
                }
                return;
            }
            _ => {}
        }
        let slot = match name {
            "ribbon" => &mut self.ui.show_ribbon,
            "properties" => &mut self.ui.show_props,
            "layers" => &mut self.pal.lm_open,
            "tools" => &mut self.ui.show_tools,
            "command" => &mut self.ui.show_cmd,
            _ => return,
        };
        *slot = show.unwrap_or(!*slot);
    }

    fn hatch_session_changed(&mut self) {
        let session = self.h.borrow().hatch_session.clone();
        match session {
            Some(s) => {
                self.ui.prev_tab = Some(self.ui.ribbon_tab.clone());
                self.ui.ribbon_tab = if s.editing.is_some() { "hatchedit".into() } else { "hatchcreate".into() };
            }
            None => {
                if self.ui.ribbon_tab == "hatchedit" || self.ui.ribbon_tab == "hatchcreate" {
                    self.ui.ribbon_tab = self.ui.prev_tab.take().unwrap_or_else(|| "home".into());
                }
            }
        }
    }
}

impl eframe::App for CadForgeApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        self.time = ctx.input(|i| i.time);
        self.h.borrow_mut().now = self.time * 1000.0;
        self.autosave(&ctx);
        if let Some(dark) = self.pending_theme.take() {
            self.set_dark(&ctx, dark);
        }
        self.store_customization();
        let locale = self.h.borrow().sysvars.string("LOCALE");
        crate::i18n::set_lang(crate::i18n::from_code(&locale).unwrap_or(crate::i18n::Lang::En));
        self.handle_keys(&ctx);
        self.process_events(&ctx);
        self.layout(ui, &ctx);
        if let Some(r) = self.vp.rect {
            *self.rect_hook.borrow_mut() = Some([r.left(), r.top(), r.width(), r.height()]);
        }
        self.show_popup(&ctx);
        self.show_toast(&ctx);
        if self.h.borrow().busy() || self.input_dirty {
            ctx.request_repaint();
        }
        self.input_dirty = false;
    }
}
