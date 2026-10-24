//! Per-drawing editing state (port of `Editor` in `src/engine/app.js`).
use crate::view::View;
use cadforge_core::drawing::Drawing;
use cadforge_core::entities::{bounds_of, Entity};
use cadforge_core::geom::Bounds;
use cadforge_core::solid::Solid;
use indexmap::IndexSet;
use std::collections::HashSet;

pub struct Editor {
    /// Stable identity (tab key).
    pub id: u64,
    pub doc: Drawing,
    pub view: View,
    /// Selected entity / solid ids.
    pub sel: IndexSet<u64>,
    /// Objects highlighted while a "Select objects" prompt is active.
    pub temp_sel: IndexSet<u64>,
    pub hover_id: Option<u64>,
    pub zoomed: bool,
    pub visual_style: String,
    /// Last view of each space (`model` or a layout name, lower-cased), restored when switching tabs.
    pub views: std::collections::HashMap<String, cadforge_core::drawing::ViewState>,
    /// Set while a layout viewport is active (MSPACE): `doc.space` is model space and `view` shows the viewport's
    /// model region at the viewport's place on the sheet.
    pub mspace: Option<crate::spaces::MSpace>,
}

impl Editor {
    pub fn new(id: u64, doc: Drawing) -> Editor {
        let mut view = View::new();
        if let Some(s) = &doc.view {
            view.set_state(s);
        }
        Editor { id, doc, view, sel: IndexSet::new(), temp_sel: IndexSet::new(), hover_id: None, zoomed: false, visual_style: "2D Wireframe".into(), views: Default::default(), mspace: None }
    }

    pub fn entity_by_id(&self, id: u64) -> Option<&Entity> {
        self.doc.entities().iter().find(|e| e.id == id)
    }

    pub fn selected(&self) -> Vec<&Entity> {
        self.doc.entities().iter().filter(|e| self.sel.contains(&e.id)).collect()
    }

    pub fn selected_ids(&self) -> Vec<u64> {
        self.doc.entities().iter().filter(|e| self.sel.contains(&e.id)).map(|e| e.id).collect()
    }

    pub fn selected_solids(&self) -> Vec<&Solid> {
        self.doc.solids.iter().filter(|s| self.sel.contains(&s.id)).collect()
    }

    pub fn select(&mut self, ids: impl IntoIterator<Item = u64>) {
        self.sel.extend(ids);
    }

    pub fn deselect(&mut self, ids: impl IntoIterator<Item = u64>) {
        for id in ids {
            self.sel.shift_remove(&id);
        }
    }

    pub fn clear_sel(&mut self) {
        self.sel.clear();
        self.temp_sel.clear();
    }

    /// Drop ids that no longer exist (after undo, erase...).
    pub fn prune(&mut self) {
        let live: HashSet<u64> = self.doc.entities().iter().map(|e| e.id).chain(self.doc.solids.iter().map(|s| s.id)).collect();
        self.sel.retain(|id| live.contains(id));
        self.temp_sel.retain(|id| live.contains(id));
    }

    pub fn extents(&self) -> Option<Bounds> {
        let mut b: Option<Bounds> = None;
        for e in self.doc.entities() {
            if self.doc.is_visible(e) {
                b = Bounds::union_opt(b, bounds_of(e, &self.doc));
            }
        }
        b
    }

    pub fn zoom_extents(&mut self, margin: f64) {
        self.view.push();
        let pts: Vec<_> = self.doc.solids.iter().flat_map(|s| s.vertices.iter().copied()).collect();
        if !self.view.is_plan() && !pts.is_empty() {
            self.view.fit_points_3d(&pts, 0.08);
            return;
        }
        let b = self.extents();
        self.view.fit(b, margin);
        self.zoomed = true;
    }
}
