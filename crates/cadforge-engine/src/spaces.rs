//! Model space / layout switching and floating model space (MSPACE) inside a layout viewport.
use crate::editor::Editor;
use cadforge_core::drawing::{ViewState, MODEL};
use cadforge_core::entities::{Kind, ViewportData};
use cadforge_core::geom::*;
use cadforge_core::view::View;

/// State kept while a layout viewport is active.
#[derive(Clone, Debug)]
pub struct MSpace {
    pub layout: String,
    pub vp: u64,
    /// The sheet view to return to (and to draw the sheet with while the viewport is active).
    pub paper_view: ViewState,
}

fn key(space: &str) -> String {
    space.to_lowercase()
}

impl Editor {
    /// Name of the tab that is current: `model` or the layout name (also while a viewport is active).
    pub fn current_tab(&self) -> String {
        match &self.mspace {
            Some(m) => m.layout.clone(),
            None => self.doc.space.clone(),
        }
    }

    /// Activate model space (`model`) or a layout. Leaves any active viewport, remembers the view of the tab left
    /// and restores the one of the tab entered (a layout seen for the first time is fitted to its sheet).
    pub fn switch_space(&mut self, name: &str) -> Result<(), String> {
        let target_is_model = name.eq_ignore_ascii_case(MODEL) || name.eq_ignore_ascii_case("Model");
        if !target_is_model && self.doc.layout(name).is_none() {
            return Err(format!("There is no layout named \"{name}\"."));
        }
        if self.mspace.is_some() {
            self.leave_mspace();
        }
        self.views.insert(key(&self.doc.space), self.view.state());
        let incoming = if target_is_model { MODEL.to_string() } else { self.doc.layout(name).map(|l| l.name.clone()).unwrap_or_default() };
        self.doc.realize_layout(&incoming);
        self.doc.set_space(&incoming)?;
        match self.views.get(&key(&incoming)).cloned() {
            Some(vs) => self.view.set_state(&vs),
            None if !target_is_model => self.fit_sheet(),
            None => {
                if let Some(vs) = self.doc.view.clone() {
                    self.view.set_state(&vs);
                }
            }
        }
        self.clear_sel();
        Ok(())
    }

    /// Fit the current layout's sheet into the view.
    pub fn fit_sheet(&mut self) {
        let Some(p) = self.doc.paper_of(&self.doc.space) else { return };
        let b = Bounds::new(0.0, 0.0, p.w, p.h);
        self.view.yaw = 0.0;
        self.view.pitch = std::f64::consts::FRAC_PI_2;
        self.view.fit(Some(b), 0.06);
    }

    fn viewport(&self, layout: &str, id: u64) -> Option<ViewportData> {
        self.doc.layout(layout)?.entities.iter().find_map(|e| match &e.kind {
            Kind::Viewport(v) if e.id == id => Some(v.clone()),
            _ => None,
        })
    }

    /// Ids of the layout's viewports in draw order.
    pub fn viewport_ids(&self, layout: &str) -> Vec<u64> {
        self.doc.layout(layout).map(|l| l.entities.iter().filter(|e| matches!(e.kind, Kind::Viewport(_))).map(|e| e.id).collect()).unwrap_or_default()
    }

    /// Topmost visible viewport of the current layout containing the sheet point `p`.
    pub fn viewport_at(&self, p: Pt) -> Option<u64> {
        let l = self.doc.layout(&self.current_tab())?;
        l.entities.iter().rev().find_map(|e| match &e.kind {
            Kind::Viewport(v) if !v.off && point_in_polygon(p, &v.outline()) => Some(e.id),
            _ => None,
        })
    }

    /// Make a viewport of the current layout the active one (floating model space).
    pub fn enter_mspace(&mut self, vp: u64) -> Result<(), String> {
        if self.mspace.is_some() {
            self.leave_mspace();
        }
        if self.doc.is_model() {
            return Err("MSPACE needs a layout. Switch to a layout tab first.".into());
        }
        let layout = self.doc.space.clone();
        let v = self.viewport(&layout, vp).ok_or("That viewport does not exist.")?;
        if v.off {
            return Err("The viewport is turned off.".into());
        }
        let paper_view = self.view.state();
        let (vc, s) = v.model_view(&self.doc);
        let centre = self.view.to_screen(v.c);
        let mut mv = View::new();
        mv.w = self.view.w;
        mv.h = self.view.h;
        mv.scale = self.view.scale * s;
        mv.target = cadforge_core::solid::V3::new(vc.x, vc.y, 0.0);
        // put the model point `vc` at the centre of the viewport on screen
        let k = mv.scale;
        let cur = mv.to_screen(vc);
        mv.target.x -= (centre.x - cur.x) / k;
        mv.target.y += (centre.y - cur.y) / k;
        self.views.insert(key(&layout), paper_view);
        self.view = mv;
        self.doc.space = MODEL.into();
        self.mspace = Some(MSpace { layout, vp, paper_view });
        self.clear_sel();
        Ok(())
    }

    /// Leave the active viewport and return to the sheet.
    pub fn leave_mspace(&mut self) {
        let Some(m) = self.mspace.take() else { return };
        self.doc.space = m.layout;
        self.view.set_state(&m.paper_view);
        self.clear_sel();
    }

    /// Screen position of the active viewport's centre.
    pub fn mspace_centre(&self) -> Option<Pt> {
        let m = self.mspace.as_ref()?;
        let v = self.viewport(&m.layout, m.vp)?;
        let mut pv = View::new();
        pv.w = self.view.w;
        pv.h = self.view.h;
        pv.set_state(&m.paper_view);
        Some(pv.to_screen(v.c))
    }

    /// The view the sheet is drawn with: the stored sheet view while a viewport is active, else the live view.
    pub fn paper_view(&self) -> View {
        let mut v = View::new();
        v.w = self.view.w;
        v.h = self.view.h;
        match &self.mspace {
            Some(m) => v.set_state(&m.paper_view),
            None => v.set_state(&self.view.state()),
        }
        v
    }

    /// After zoom / pan inside an active viewport: store the resulting model region in the viewport (unless it is
    /// locked, in which case the view snaps back).
    pub fn sync_mspace_view(&mut self) {
        let Some(m) = self.mspace.clone() else { return };
        let Some(v) = self.viewport(&m.layout, m.vp) else { return };
        let mut pv = View::new();
        pv.w = self.view.w;
        pv.h = self.view.h;
        pv.set_state(&m.paper_view);
        if v.locked {
            // a locked viewport keeps its display: rebuild the model view from the stored viewport
            self.mspace = None;
            self.doc.space = m.layout.clone();
            self.view.set_state(&m.paper_view);
            let _ = self.enter_mspace(m.vp);
            return;
        }
        let centre = pv.to_screen(v.c);
        let vc = self.view.to_world(centre.x, centre.y);
        let s = self.view.scale / pv.scale;
        self.doc.set_viewport_view(&m.layout, m.vp, vc, s);
    }
}
