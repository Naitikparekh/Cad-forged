//! Headless driver for the command runtime: simulates the user at the command line and canvas
//! (port of `tests/unit/sim.js`). Used by the tests; also handy for scripting.
use crate::app::{App, Handle};
use crate::runtime::{Args, Mods};
use cadforge_core::drawing::Drawing;
use cadforge_core::entities::Entity;
use cadforge_core::geom::{pt, Pt};
use std::rc::Rc;

pub struct Sim {
    pub h: Handle,
}

/// One scripted interaction step for [`Sim::play`].
#[derive(Clone, Copy, Debug)]
pub enum Step {
    Click(f64, f64),
    Type(&'static str),
    Enter,
    Move(f64, f64),
    Esc,
}

impl Default for Sim {
    fn default() -> Sim {
        Sim::new()
    }
}

impl Sim {
    pub fn new() -> Sim {
        let app = App::new(Rc::new(crate::commands::build_registry()));
        let h = Handle::new(app);
        {
            let mut a = h.borrow_mut();
            a.new_drawing(None, false);
            // small snap aperture in drawing units, like a zoomed-in session
            a.editor_mut().expect("editor").view.scale = 50.0;
        }
        Sim { h }
    }

    pub fn cmd(&self, name: &str) -> &Sim {
        self.h.run(name);
        self
    }

    pub fn cmd_args(&self, name: &str, args: Args) -> &Sim {
        self.h.start(name, args);
        self
    }

    pub fn click(&self, x: f64, y: f64) -> &Sim {
        self.click_mods(x, y, Mods::default())
    }

    pub fn click_mods(&self, x: f64, y: f64, mods: Mods) -> &Sim {
        self.h.mouse_move(pt(x, y));
        self.h.click(pt(x, y), mods);
        self
    }

    pub fn shift_click(&self, x: f64, y: f64) -> &Sim {
        self.click_mods(
            x,
            y,
            Mods {
                shift: true,
                ..Mods::default()
            },
        )
    }

    pub fn mv(&self, x: f64, y: f64) -> &Sim {
        self.h.mouse_move(pt(x, y));
        self
    }

    pub fn drag(&self, a: (f64, f64), b: (f64, f64)) -> &Sim {
        self.h.drag(pt(a.0, a.1), pt(b.0, b.1), Mods::default());
        self
    }

    /// Type text at the command line (as if followed by Enter).
    pub fn ty(&self, text: &str) -> &Sim {
        self.h.text(text, false);
        self
    }

    pub fn enter(&self) -> &Sim {
        self.h.enter();
        self
    }

    pub fn esc(&self) -> &Sim {
        self.h.escape();
        self
    }

    /// Run a full scripted interaction.
    pub fn play(&self, name: &str, steps: &[Step]) -> &Sim {
        self.cmd(name);
        for s in steps {
            match *s {
                Step::Click(x, y) => {
                    self.click(x, y);
                }
                Step::Type(t) => {
                    self.ty(t);
                }
                Step::Enter => {
                    self.enter();
                }
                Step::Move(x, y) => {
                    self.mv(x, y);
                }
                Step::Esc => {
                    self.esc();
                }
            }
        }
        self
    }

    pub fn doc<R>(&self, f: impl FnOnce(&Drawing) -> R) -> R {
        f(&self.h.borrow().editor().expect("editor").doc)
    }

    pub fn doc_mut<R>(&self, f: impl FnOnce(&mut Drawing) -> R) -> R {
        f(&mut self.h.borrow_mut().editor_mut().expect("editor").doc)
    }

    /// Entities of the active space.
    pub fn ents(&self) -> Vec<Entity> {
        self.doc(|d| d.entities().clone())
    }

    pub fn ent(&self, i: usize) -> Entity {
        self.ents()[i].clone()
    }

    pub fn n(&self) -> usize {
        self.doc(|d| d.entities().len())
    }

    pub fn busy(&self) -> bool {
        self.h.borrow().busy()
    }

    /// The command-line prompt (`Specify next point or [Close/Undo]:`).
    pub fn prompt(&self) -> String {
        self.h.borrow().prompt_line()
    }

    /// The prompt message without options/default.
    pub fn message(&self) -> String {
        self.h
            .borrow()
            .pending()
            .map(|p| p.spec.message.clone())
            .unwrap_or_default()
    }

    pub fn select_ids(&self, ids: &[u64]) -> &Sim {
        self.h
            .borrow_mut()
            .editor_mut()
            .expect("editor")
            .select(ids.iter().copied());
        self
    }

    pub fn selected(&self) -> Vec<u64> {
        self.h
            .borrow()
            .editor()
            .expect("editor")
            .sel
            .iter()
            .copied()
            .collect()
    }

    pub fn log_text(&self) -> String {
        self.h
            .borrow()
            .history
            .iter()
            .map(|l| l.text.clone())
            .collect::<Vec<_>>()
            .join("\n")
    }

    pub fn last_log(&self) -> String {
        self.h
            .borrow()
            .history
            .last()
            .map(|l| l.text.clone())
            .unwrap_or_default()
    }

    pub fn undo(&self) {
        self.h.borrow_mut().editor_mut().expect("editor").doc.undo();
    }

    pub fn redo(&self) {
        self.h.borrow_mut().editor_mut().expect("editor").doc.redo();
    }

    pub fn sel_count(&self) -> usize {
        self.h.borrow().editor().expect("editor").sel.len()
    }

    pub fn preview(&self) -> Vec<crate::runtime::PreviewItem> {
        self.h.borrow().rt.preview.clone()
    }

    /// Name of the innermost running command.
    pub fn running(&self) -> Option<String> {
        let a = self.h.borrow();
        a.top_task()
            .and_then(|t| a.rt.tasks.get(&t))
            .map(|t| t.def.name.clone())
    }

    pub fn with_view<R>(&self, f: impl FnOnce(&mut crate::view::View) -> R) -> R {
        f(&mut self.h.borrow_mut().editor_mut().expect("editor").view)
    }

    pub fn registry(&self) -> std::rc::Rc<crate::registry::Registry> {
        self.h.borrow().registry.clone()
    }

    pub fn pref(&self, name: &str) -> f64 {
        self.h.borrow().sysvars.real(name)
    }

    pub fn set_visual_style(&self, name: &str) {
        self.h
            .borrow_mut()
            .editor_mut()
            .expect("editor")
            .visual_style = name.into();
    }

    pub fn solids(&self) -> Vec<cadforge_core::solid::Solid> {
        self.doc(|d| d.solids.clone())
    }

    pub fn history_has(&self, needle: &str) -> bool {
        self.h
            .borrow()
            .history
            .iter()
            .any(|l| l.text.contains(needle))
    }

    pub fn solid_at(&self, x: f64, y: f64) -> Option<u64> {
        self.h
            .borrow()
            .solid_at(pt(x, y), &crate::runtime::Mods::default())
    }

    pub fn set_sel(&self, ids: &[u64]) {
        self.h.borrow_mut().editor_mut().expect("editor").sel = ids.iter().copied().collect();
    }

    pub fn selected_solid_count(&self) -> usize {
        self.h
            .borrow()
            .editor()
            .expect("editor")
            .selected_solids()
            .len()
    }

    pub fn view_scale(&self) -> f64 {
        self.h.borrow().editor().expect("editor").view.scale
    }

    pub fn add(&self, e: Entity) -> u64 {
        self.doc_mut(|d| d.add(e))
    }

    pub fn setvar(&self, name: &str, v: f64) {
        let _ = self.h.borrow_mut().setvar(name, v);
    }
}

pub fn p(x: f64, y: f64) -> Pt {
    pt(x, y)
}
