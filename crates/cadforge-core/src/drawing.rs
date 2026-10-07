//! The drawing database (port of `src/core/drawing.js`): layers, blocks, model/paper-space entities, solids and
//! settings, plus snapshot-based undo. Serialises to the same JSON as the web application (format version 2).
use crate::entities::{Entity, Kind};
use crate::geom::{Pt, Seg};
use crate::solid::Solid;
use crate::style::{Color, Layer, Settings};
use indexmap::IndexMap;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

pub const FORMAT_VERSION: u32 = 2;
const MAX_UNDO: usize = 100;
const MAX_UNDO_BYTES: usize = 96_000_000;

pub const MODEL: &str = "model";

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Block {
    #[serde(default)]
    pub name: String,
    pub base: Pt,
    #[serde(default)]
    pub entities: Vec<Entity>,
}

#[derive(Clone, Debug, PartialEq, Default, Serialize, Deserialize)]
pub struct Space {
    #[serde(default)]
    pub entities: Vec<Entity>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Paper {
    pub w: f64,
    pub h: f64,
    #[serde(default)]
    pub name: String,
    #[serde(default = "yes")]
    pub landscape: bool,
}

fn yes() -> bool {
    true
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Layout {
    pub name: String,
    pub paper: Paper,
    #[serde(default)]
    pub entities: Vec<Entity>,
}

/// A named view: centre and scale (VIEW command), plus the 3D orientation when saved from a 3D view.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct NamedView {
    pub cx: f64,
    pub cy: f64,
    pub scale: f64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cz: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub yaw: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pitch: Option<f64>,
}

impl NamedView {
    pub fn from_state(s: &ViewState) -> NamedView {
        NamedView { cx: s.cx, cy: s.cy, scale: s.scale, cz: Some(s.cz), yaw: Some(s.yaw), pitch: Some(s.pitch) }
    }
    pub fn to_state(&self) -> ViewState {
        ViewState { cx: self.cx, cy: self.cy, cz: self.cz.unwrap_or(0.0), scale: self.scale, yaw: self.yaw.unwrap_or(0.0), pitch: self.pitch.unwrap_or(std::f64::consts::FRAC_PI_2) }
    }
}

/// Persisted camera state.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct ViewState {
    pub cx: f64,
    pub cy: f64,
    #[serde(default)]
    pub cz: f64,
    pub scale: f64,
    #[serde(default)]
    pub yaw: f64,
    #[serde(default = "half_pi")]
    pub pitch: f64,
}

fn half_pi() -> f64 {
    std::f64::consts::FRAC_PI_2
}

/// Current properties applied to new objects.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Current {
    pub layer: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub color: Option<Color>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ltype: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lw: Option<i32>,
}

impl Default for Current {
    fn default() -> Current {
        Current { layer: "0".into(), color: None, ltype: None, lw: None }
    }
}

/// On-disk representation (also used for undo snapshots, which omit the `format`/`name`/`view` fields).
#[derive(Serialize, Deserialize)]
struct File {
    format: String,
    version: u32,
    name: String,
    settings: Settings,
    layers: Vec<Layer>,
    blocks: IndexMap<String, Block>,
    model: Space,
    layouts: Vec<Layout>,
    solids: Vec<Solid>,
    views: IndexMap<String, NamedView>,
    cur: Current,
    #[serde(rename = "nextId")]
    next_id: u64,
    view: Option<ViewState>,
}

#[derive(Serialize, Deserialize)]
struct Snap {
    layers: Vec<Layer>,
    blocks: IndexMap<String, Block>,
    settings: Settings,
    model: Space,
    layouts: Vec<Layout>,
    solids: Vec<Solid>,
    views: IndexMap<String, NamedView>,
    cur: Current,
    #[serde(rename = "nextId")]
    next_id: u64,
}

struct UndoStep {
    snap: String,
    label: String,
}

/// Per-revision geometry caches (see `entities.rs`); cleared whenever the drawing changes.
#[derive(Default)]
pub struct Caches {
    pub segs: HashMap<u64, Rc<Vec<Seg>>>,
    pub bounds: HashMap<u64, Option<crate::geom::Bounds>>,
    pub hatch: HashMap<u64, Option<Rc<Vec<[f64; 4]>>>>,
    pub snaps: HashMap<u64, Rc<Vec<crate::entities::SnapPt>>>,
}

pub struct Drawing {
    pub name: String,
    /// Bumped on every change; caches key off it.
    pub rev: u64,
    pub next_id: u64,
    pub layers: Vec<Layer>,
    pub blocks: IndexMap<String, Block>,
    pub settings: Settings,
    pub model: Space,
    pub layouts: Vec<Layout>,
    pub solids: Vec<Solid>,
    pub views: IndexMap<String, NamedView>,
    pub cur: Current,
    /// Active space: `"model"` or a layout name.
    pub space: String,
    /// Last camera state (persisted).
    pub view: Option<ViewState>,
    pub dirty: bool,
    pub file_name: Option<String>,
    /// Entities skipped / paper-space objects ignored by the last DXF import.
    pub skipped: Vec<String>,
    pub paper_skipped: usize,
    /// Layer on/lock states saved by LAYISO (not persisted).
    pub layer_iso: Option<Vec<(String, bool, bool)>>,
    undo_stack: Vec<UndoStep>,
    redo_stack: Vec<UndoStep>,
    pre: Option<String>,
    cmd_active: bool,
    cmd_rev: u64,
    caches: RefCell<Caches>,
}

impl Default for Drawing {
    fn default() -> Drawing {
        Drawing::new("Drawing1")
    }
}

impl Clone for Drawing {
    /// A deep copy of the drawing data (undo history and caches are not carried over).
    fn clone(&self) -> Drawing {
        let mut d = Drawing::new(&self.name);
        d.rev = self.rev;
        d.next_id = self.next_id;
        d.layers = self.layers.clone();
        d.blocks = self.blocks.clone();
        d.settings = self.settings.clone();
        d.model = self.model.clone();
        d.layouts = self.layouts.clone();
        d.solids = self.solids.clone();
        d.views = self.views.clone();
        d.cur = self.cur.clone();
        d.space = self.space.clone();
        d.view = self.view;
        d.dirty = self.dirty;
        d.file_name = self.file_name.clone();
        d
    }
}

impl Drawing {
    pub fn new(name: &str) -> Drawing {
        Drawing {
            name: name.into(),
            rev: 0,
            next_id: 1,
            layers: vec![Layer::new("0")],
            blocks: IndexMap::new(),
            settings: Settings::default(),
            model: Space::default(),
            layouts: Vec::new(),
            solids: Vec::new(),
            views: IndexMap::new(),
            cur: Current::default(),
            space: MODEL.into(),
            view: None,
            dirty: false,
            file_name: None,
            skipped: Vec::new(),
            paper_skipped: 0,
            layer_iso: None,
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
            pre: None,
            cmd_active: false,
            cmd_rev: 0,
            caches: RefCell::new(Caches::default()),
        }
    }

    // ------------------------------------------------------------ caches

    pub fn cache_get_segs(&self, id: u64) -> Option<Rc<Vec<Seg>>> {
        self.caches.borrow().segs.get(&id).cloned()
    }
    pub fn cache_put_segs(&self, id: u64, v: Rc<Vec<Seg>>) {
        self.caches.borrow_mut().segs.insert(id, v);
    }
    pub fn cache_get_bounds(&self, id: u64) -> Option<Option<crate::geom::Bounds>> {
        self.caches.borrow().bounds.get(&id).cloned()
    }
    pub fn cache_put_bounds(&self, id: u64, v: Option<crate::geom::Bounds>) {
        self.caches.borrow_mut().bounds.insert(id, v);
    }
    pub fn cache_get_hatch(&self, id: u64) -> Option<Option<Rc<Vec<[f64; 4]>>>> {
        self.caches.borrow().hatch.get(&id).cloned()
    }
    pub fn cache_put_hatch(&self, id: u64, v: Option<Rc<Vec<[f64; 4]>>>) {
        self.caches.borrow_mut().hatch.insert(id, v);
    }
    pub fn cache_get_snaps(&self, id: u64) -> Option<Rc<Vec<crate::entities::SnapPt>>> {
        self.caches.borrow().snaps.get(&id).cloned()
    }
    pub fn cache_put_snaps(&self, id: u64, v: Rc<Vec<crate::entities::SnapPt>>) {
        self.caches.borrow_mut().snaps.insert(id, v);
    }

    // ------------------------------------------------------------ spaces & entities

    pub fn is_model(&self) -> bool {
        self.space == MODEL
    }

    /// Entities of the active space.
    pub fn entities(&self) -> &Vec<Entity> {
        self.space_entities(&self.space)
    }

    pub fn space_entities(&self, id: &str) -> &Vec<Entity> {
        if id != MODEL {
            if let Some(l) = self.layouts.iter().find(|l| l.name == id) {
                return &l.entities;
            }
        }
        &self.model.entities
    }

    fn space_entities_mut(&mut self, id: &str) -> &mut Vec<Entity> {
        if id != MODEL {
            if let Some(i) = self.layouts.iter().position(|l| l.name == id) {
                return &mut self.layouts[i].entities;
            }
        }
        &mut self.model.entities
    }

    pub fn layout(&self, name: &str) -> Option<&Layout> {
        self.layouts.iter().find(|l| l.name == name)
    }

    /// Every entity in model space, all layouts and all block definitions.
    pub fn all_entities(&self) -> impl Iterator<Item = &Entity> {
        self.model
            .entities
            .iter()
            .chain(self.layouts.iter().flat_map(|l| l.entities.iter()))
            .chain(self.blocks.values().flat_map(|b| b.entities.iter()))
    }

    pub fn all_entities_mut(&mut self) -> impl Iterator<Item = &mut Entity> {
        self.model
            .entities
            .iter_mut()
            .chain(self.layouts.iter_mut().flat_map(|l| l.entities.iter_mut()))
            .chain(self.blocks.values_mut().flat_map(|b| b.entities.iter_mut()))
    }

    pub fn by_id(&self, id: u64) -> Option<&Entity> {
        self.all_entities().find(|e| e.id == id)
    }

    /// Mutable access to an entity. The caller must have called [`Drawing::will_change`] (or use [`Drawing::change`]).
    pub fn by_id_mut(&mut self, id: u64) -> Option<&mut Entity> {
        self.all_entities_mut().find(|e| e.id == id)
    }

    /// Add an entity to `space`, filling in id and the current layer/colour/linetype/lineweight.
    pub fn add_to(&mut self, mut e: Entity, space: &str) -> u64 {
        self.will_change();
        if e.id == 0 {
            e.id = self.next_id;
            self.next_id += 1;
        } else if e.id >= self.next_id {
            self.next_id = e.id + 1;
        }
        if e.layer.is_empty() {
            e.layer = self.cur.layer.clone();
        }
        if e.color.is_none() {
            e.color = self.cur.color;
        }
        if e.ltype.is_none() {
            e.ltype = self.cur.ltype.clone();
        }
        if e.lw.is_none() {
            e.lw = self.cur.lw;
        }
        let id = e.id;
        self.space_entities_mut(space).push(e);
        self.touch();
        id
    }

    /// Add to the active space; returns the new entity id.
    pub fn add(&mut self, e: Entity) -> u64 {
        let sp = self.space.clone();
        self.add_to(e, &sp)
    }

    pub fn add_all(&mut self, list: Vec<Entity>) -> Vec<u64> {
        list.into_iter().map(|e| self.add(e)).collect()
    }

    pub fn remove(&mut self, id: u64) {
        self.remove_all(&[id]);
    }

    pub fn remove_all(&mut self, ids: &[u64]) {
        self.will_change();
        let sp = self.space.clone();
        self.space_entities_mut(&sp).retain(|e| !ids.contains(&e.id));
        self.touch();
    }

    /// Replace an entity in place, keeping its id and draw order. Returns false if `id` is not in the active space.
    pub fn replace(&mut self, id: u64, mut next: Entity) -> bool {
        self.will_change();
        let sp = self.space.clone();
        let arr = self.space_entities_mut(&sp);
        let Some(i) = arr.iter().position(|e| e.id == id) else { return false };
        let old = &arr[i];
        next.id = id;
        if next.layer.is_empty() {
            next.layer = old.layer.clone();
        }
        if next.color.is_none() {
            next.color = old.color;
        }
        if next.ltype.is_none() {
            next.ltype = old.ltype.clone();
        }
        if next.lw.is_none() {
            next.lw = old.lw;
        }
        if next.lts.is_none() {
            next.lts = old.lts;
        }
        if next.grp.is_none() {
            next.grp = old.grp.clone();
        }
        arr[i] = next;
        self.touch();
        true
    }

    /// Position of an entity in the active space's draw order.
    pub fn index_of(&self, id: u64) -> Option<usize> {
        self.entities().iter().position(|e| e.id == id)
    }

    /// Replace `delete` entities starting at `idx` of the active space with `insert`. Inserted entities with id 0
    /// get fresh ids; no defaults (layer, colour...) are filled in. Returns the ids of the inserted entities.
    pub fn splice(&mut self, idx: usize, delete: usize, insert: Vec<Entity>) -> Vec<u64> {
        self.will_change();
        let mut ids = vec![];
        let mut prepared = vec![];
        for mut e in insert {
            if e.id == 0 {
                e.id = self.next_id;
                self.next_id += 1;
            } else if e.id >= self.next_id {
                self.next_id = e.id + 1;
            }
            ids.push(e.id);
            prepared.push(e);
        }
        let sp = self.space.clone();
        let arr = self.space_entities_mut(&sp);
        let idx = idx.min(arr.len());
        let end = (idx + delete).min(arr.len());
        arr.splice(idx..end, prepared);
        self.touch();
        ids
    }

    /// Append an entity to the active space with a fresh id but without filling in current-property defaults.
    pub fn insert_raw(&mut self, e: Entity) -> u64 {
        let n = self.entities().len();
        self.splice(n, 0, vec![e])[0]
    }

    /// Overwrite an entity (any space) keeping its id. Returns false if it does not exist.
    pub fn set_entity(&mut self, id: u64, mut e: Entity) -> bool {
        self.will_change();
        let found = match self.by_id_mut(id) {
            Some(slot) => {
                e.id = id;
                *slot = e;
                true
            }
            None => false,
        };
        self.touch();
        found
    }

    /// Move the given entities to the front (end of the list) or back (start) of the active space's draw order.
    pub fn reorder(&mut self, ids: &[u64], front: bool) {
        self.will_change();
        let sp = self.space.clone();
        let arr = self.space_entities_mut(&sp);
        let (moved, rest): (Vec<Entity>, Vec<Entity>) = arr.drain(..).partition(|e| ids.contains(&e.id));
        *arr = if front { rest.into_iter().chain(moved).collect() } else { moved.into_iter().chain(rest).collect() };
        self.touch();
    }

    /// Run a mutation inside a change notification.
    pub fn change<R>(&mut self, f: impl FnOnce(&mut Drawing) -> R) -> R {
        self.will_change();
        let r = f(self);
        self.touch();
        r
    }

    /// Mutate one entity by id (any space) inside a change notification.
    pub fn modify(&mut self, id: u64, f: impl FnOnce(&mut Entity)) -> bool {
        self.will_change();
        let found = match self.by_id_mut(id) {
            Some(e) => {
                f(e);
                true
            }
            None => false,
        };
        self.touch();
        found
    }

    pub fn touch(&mut self) {
        self.rev += 1;
        self.dirty = true;
        *self.caches.get_mut() = Caches::default();
    }

    // ------------------------------------------------------------ layers

    pub fn layer(&self, name: &str) -> Option<&Layer> {
        let k = name.to_lowercase();
        self.layers.iter().find(|l| l.name.to_lowercase() == k)
    }

    pub fn layer_mut(&mut self, name: &str) -> Option<&mut Layer> {
        let k = name.to_lowercase();
        self.layers.iter_mut().find(|l| l.name.to_lowercase() == k)
    }

    pub fn layer_of(&self, e: &Entity) -> &Layer {
        self.layer(&e.layer).unwrap_or(&self.layers[0])
    }

    pub fn cur_layer(&self) -> &Layer {
        self.layer(&self.cur.layer).unwrap_or(&self.layers[0])
    }

    pub fn is_visible(&self, e: &Entity) -> bool {
        let l = self.layer_of(e);
        l.on && !l.frozen
    }

    pub fn is_locked(&self, e: &Entity) -> bool {
        self.layer_of(e).locked
    }

    pub fn add_layer(&mut self, name: &str) -> Result<&Layer, String> {
        self.add_layer_with(Layer::new(name))
    }

    pub fn add_layer_with(&mut self, layer: Layer) -> Result<&Layer, String> {
        if self.layer(&layer.name).is_some() {
            return Err(format!("A layer named \"{}\" already exists.", layer.name));
        }
        self.will_change();
        self.layers.push(layer);
        self.touch();
        Ok(self.layers.last().expect("just pushed"))
    }

    pub fn ensure_layer(&mut self, name: &str) -> String {
        if let Some(l) = self.layer(name) {
            return l.name.clone();
        }
        self.add_layer(name).map(|l| l.name.clone()).unwrap_or_else(|_| name.to_string())
    }

    pub fn layer_usage(&self, name: &str) -> bool {
        let k = name.to_lowercase();
        self.all_entities().any(|e| e.layer.to_lowercase() == k) || self.solids.iter().any(|s| s.layer.to_lowercase() == k)
    }

    pub fn delete_layer(&mut self, name: &str) -> Result<bool, String> {
        let Some(l) = self.layer(name) else { return Ok(false) };
        let lname = l.name.clone();
        if lname == "0" || lname.eq_ignore_ascii_case("defpoints") {
            return Err(format!("Layer \"{lname}\" cannot be deleted."));
        }
        if lname.eq_ignore_ascii_case(&self.cur.layer) {
            return Err("The current layer cannot be deleted.".into());
        }
        if self.layer_usage(&lname) {
            return Err(format!("Layer \"{lname}\" contains objects and cannot be deleted."));
        }
        self.will_change();
        self.layers.retain(|l| l.name != lname);
        self.touch();
        Ok(true)
    }

    pub fn rename_layer(&mut self, old: &str, new: &str) -> Result<(), String> {
        let Some(l) = self.layer(old) else { return Err("That layer cannot be renamed.".into()) };
        if l.name == "0" {
            return Err("That layer cannot be renamed.".into());
        }
        let lname = l.name.clone();
        if let Some(o) = self.layer(new) {
            if o.name != lname {
                return Err(format!("A layer named \"{new}\" already exists."));
            }
        }
        self.will_change();
        let k = old.to_lowercase();
        for e in self.all_entities_mut() {
            if e.layer.to_lowercase() == k {
                e.layer = new.to_string();
            }
        }
        for s in &mut self.solids {
            if s.layer.to_lowercase() == k {
                s.layer = new.to_string();
            }
        }
        if self.cur.layer.to_lowercase() == k {
            self.cur.layer = new.to_string();
        }
        if let Some(l) = self.layer_mut(&lname) {
            l.name = new.to_string();
        }
        self.touch();
        Ok(())
    }

    pub fn set_current_layer(&mut self, name: &str) -> Result<(), String> {
        let Some(l) = self.layer(name) else { return Err(format!("Layer \"{name}\" not found.")) };
        if l.frozen {
            return Err(format!("Layer \"{}\" is frozen and cannot be made current.", l.name));
        }
        self.cur.layer = l.name.clone();
        Ok(())
    }

    // ------------------------------------------------------------ solids

    pub fn add_solid(&mut self, mut s: Solid) -> u64 {
        self.will_change();
        if s.id == 0 {
            s.id = self.next_id;
            self.next_id += 1;
        } else if s.id >= self.next_id {
            self.next_id = s.id + 1;
        }
        if s.layer.is_empty() {
            s.layer = self.cur.layer.clone();
        }
        if s.color.is_none() {
            if let Some(c @ (Color::Aci(_) | Color::Rgb(..))) = self.cur.color {
                s.color = Some(c);
            }
        }
        let id = s.id;
        self.solids.push(s);
        self.touch();
        id
    }

    /// Add a solid exactly as given (fresh id, no current-property defaults).
    pub fn insert_solid_raw(&mut self, mut s: Solid) -> u64 {
        self.will_change();
        s.id = self.next_id;
        self.next_id += 1;
        let id = s.id;
        self.solids.push(s);
        self.touch();
        id
    }

    pub fn remove_solid(&mut self, id: u64) {
        self.will_change();
        self.solids.retain(|s| s.id != id);
        self.touch();
    }

    pub fn solid_by_id(&self, id: u64) -> Option<&Solid> {
        self.solids.iter().find(|s| s.id == id)
    }

    pub fn solid_by_id_mut(&mut self, id: u64) -> Option<&mut Solid> {
        self.solids.iter_mut().find(|s| s.id == id)
    }

    pub fn solid_visible(&self, s: &Solid) -> bool {
        let l = self.layer(if s.layer.is_empty() { "0" } else { &s.layer }).unwrap_or(&self.layers[0]);
        l.on && !l.frozen
    }

    // ------------------------------------------------------------ blocks

    pub fn define_block(&mut self, name: &str, base: Pt, mut entities: Vec<Entity>) {
        self.will_change();
        for e in &mut entities {
            if e.id == 0 {
                e.id = self.next_id;
                self.next_id += 1;
            } else if e.id >= self.next_id {
                self.next_id = e.id + 1;
            }
        }
        self.blocks.insert(name.to_string(), Block { name: name.to_string(), base, entities });
        self.touch();
    }

    // ------------------------------------------------------------ undo / redo

    /// Call before mutating. Takes the pre-image snapshot once per command.
    pub fn will_change(&mut self) {
        if self.cmd_active && self.pre.is_none() {
            self.pre = Some(self.snapshot());
        }
    }

    pub fn begin_command(&mut self) {
        self.cmd_active = true;
        self.pre = None;
        self.cmd_rev = self.rev;
    }

    /// End a command; if anything changed, the pre-image becomes an undo step.
    pub fn end_command(&mut self, label: &str) {
        if self.cmd_active && self.rev != self.cmd_rev {
            if let Some(snap) = self.pre.take() {
                self.undo_stack.push(UndoStep { snap, label: label.into() });
                self.redo_stack.clear();
                self.trim_undo();
            }
        }
        self.cmd_active = false;
        self.pre = None;
    }

    pub fn command_active(&self) -> bool {
        self.cmd_active
    }

    /// Push an undo step right now (for changes made outside a command).
    pub fn checkpoint(&mut self, label: &str) {
        let snap = self.snapshot();
        self.undo_stack.push(UndoStep { snap, label: label.into() });
        self.redo_stack.clear();
        self.trim_undo();
    }

    fn trim_undo(&mut self) {
        let mut bytes: usize = self.undo_stack.iter().map(|s| s.snap.len()).sum();
        while self.undo_stack.len() > MAX_UNDO || (bytes > MAX_UNDO_BYTES && self.undo_stack.len() > 1) {
            bytes -= self.undo_stack.remove(0).snap.len();
        }
    }

    fn snapshot(&self) -> String {
        let s = Snap {
            layers: self.layers.clone(),
            blocks: self.blocks.clone(),
            settings: self.settings.clone(),
            model: self.model.clone(),
            layouts: self.layouts.clone(),
            solids: self.solids.clone(),
            views: self.views.clone(),
            cur: self.cur.clone(),
            next_id: self.next_id,
        };
        serde_json::to_string(&s).expect("snapshot serialises")
    }

    fn restore(&mut self, snap: &str) {
        let s: Snap = serde_json::from_str(snap).expect("snapshot parses");
        self.layers = s.layers;
        self.blocks = s.blocks;
        self.settings = s.settings;
        self.model = s.model;
        self.layouts = s.layouts;
        self.solids = s.solids;
        self.views = s.views;
        self.cur = s.cur;
        self.next_id = self.next_id.max(s.next_id);
        if self.space != MODEL && !self.layouts.iter().any(|l| l.name == self.space) {
            self.space = MODEL.into();
        }
        self.touch();
    }

    pub fn can_undo(&self) -> bool {
        !self.undo_stack.is_empty()
    }

    pub fn can_redo(&self) -> bool {
        !self.redo_stack.is_empty()
    }

    pub fn undo_depth(&self) -> usize {
        self.undo_stack.len()
    }

    pub fn undo_label(&self) -> Option<&str> {
        self.undo_stack.last().map(|s| s.label.as_str())
    }

    pub fn redo_label(&self) -> Option<&str> {
        self.redo_stack.last().map(|s| s.label.as_str())
    }

    pub fn undo(&mut self) -> Option<String> {
        let step = self.undo_stack.pop()?;
        let now = self.snapshot();
        self.redo_stack.push(UndoStep { snap: now, label: step.label.clone() });
        self.restore(&step.snap);
        Some(step.label)
    }

    pub fn redo(&mut self) -> Option<String> {
        let step = self.redo_stack.pop()?;
        let now = self.snapshot();
        self.undo_stack.push(UndoStep { snap: now, label: step.label.clone() });
        self.restore(&step.snap);
        Some(step.label)
    }

    // ------------------------------------------------------------ persistence

    pub fn to_json(&self) -> Value {
        let f = File {
            format: "cadforge".into(),
            version: FORMAT_VERSION,
            name: self.name.clone(),
            settings: self.settings.clone(),
            layers: self.layers.clone(),
            blocks: self.blocks.clone(),
            model: self.model.clone(),
            layouts: self.layouts.clone(),
            solids: self.solids.clone(),
            views: self.views.clone(),
            cur: self.cur.clone(),
            next_id: self.next_id,
            view: self.view,
        };
        serde_json::to_value(f).expect("drawing serialises")
    }

    pub fn to_json_string(&self) -> String {
        serde_json::to_string(&self.to_json()).expect("drawing serialises")
    }

    pub fn from_json_str(text: &str, name: Option<&str>) -> Result<Drawing, String> {
        let v: Value = serde_json::from_str(text).map_err(|e| format!("Not a CadForge drawing ({e})."))?;
        Drawing::from_json(&v, name)
    }

    pub fn from_json(obj: &Value, name: Option<&str>) -> Result<Drawing, String> {
        if !obj.is_object() {
            return Err("Not a CadForge drawing.".into());
        }
        if obj.get("format").and_then(Value::as_str) != Some("cadforge") {
            return Drawing::from_legacy(obj, name);
        }
        if obj.get("version").and_then(Value::as_u64).unwrap_or(0) > FORMAT_VERSION as u64 {
            return Err("This drawing was saved by a newer version of CadForge.".into());
        }
        let mut o = obj.clone();
        // Tolerate older files that lack optional members.
        if let Some(m) = o.as_object_mut() {
            m.entry("name").or_insert(Value::String("Drawing1".into()));
            for k in ["blocks", "views"] {
                if m.get(k).map_or(true, Value::is_null) {
                    m.insert(k.into(), Value::Object(Default::default()));
                }
            }
            for k in ["layouts", "solids"] {
                if m.get(k).map_or(true, Value::is_null) {
                    m.insert(k.into(), Value::Array(vec![]));
                }
            }
            if m.get("model").map_or(true, Value::is_null) {
                m.insert("model".into(), serde_json::json!({ "entities": [] }));
            }
            if m.get("settings").map_or(true, Value::is_null) {
                m.insert("settings".into(), Value::Object(Default::default()));
            }
            if m.get("cur").map_or(true, Value::is_null) {
                m.insert("cur".into(), serde_json::json!({ "layer": "0" }));
            }
            if m.get("layers").and_then(Value::as_array).map_or(true, |a| a.is_empty()) {
                m.insert("layers".into(), serde_json::to_value(vec![Layer::new("0")]).expect("layer"));
            }
            m.entry("nextId").or_insert(Value::from(1));
            if m.get("view").is_some_and(Value::is_null) {
                m.remove("view");
            }
            m.entry("view").or_insert(Value::Null);
        }
        let f: File = serde_json::from_value(o).map_err(|e| format!("Invalid CadForge drawing: {e}."))?;
        let mut d = Drawing::new(name.unwrap_or(&f.name));
        d.settings = f.settings;
        d.layers = f.layers;
        d.blocks = f.blocks;
        d.model = f.model;
        d.layouts = f.layouts;
        d.solids = f.solids;
        d.views = f.views;
        d.cur = f.cur;
        d.view = f.view;
        d.next_id = f.next_id.max(1);
        let max_id = d.all_entities().map(|e| e.id).chain(d.solids.iter().map(|s| s.id)).max().unwrap_or(0);
        if max_id >= d.next_id {
            d.next_id = max_id + 1;
        }
        d.validate()?;
        d.dirty = false;
        Ok(d)
    }

    /// Structural sanity check run on every load.
    pub fn validate(&self) -> Result<(), String> {
        let fin = |n: f64| n.is_finite();
        let okp = |p: &Pt| fin(p.x) && fin(p.y);
        if self.layers.is_empty() {
            return Err("Drawing has no layers.".into());
        }
        for e in self.all_entities() {
            let ok = match &e.kind {
                Kind::Line { a, b } => okp(a) && okp(b),
                Kind::Circle { c, r } => okp(c) && fin(*r) && *r > 0.0,
                Kind::Arc { c, r, a0, a1 } => okp(c) && fin(*r) && *r > 0.0 && fin(*a0) && fin(*a1),
                Kind::Pline { v, .. } => v.len() > 1 && v.iter().all(|p| fin(p.x) && fin(p.y)),
                Kind::Point { p } => okp(p),
                Kind::Text(t) => okp(&t.p) && fin(t.h) && t.h > 0.0,
                Kind::Insert(i) => okp(&i.p),
                _ => true,
            };
            if !ok {
                return Err(format!("Invalid {} entity (id {}).", kind_name(&e.kind), e.id));
            }
        }
        for s in &self.solids {
            let n = s.vertices.len();
            if s.faces.iter().any(|f| f.iter().any(|&i| i as usize >= n)) {
                return Err("Invalid solid geometry.".into());
            }
        }
        Ok(())
    }

    /// Import a version-1 CadForge project (flat entities, `points[]` arrays, group-based blocks).
    pub fn from_legacy(old: &Value, name: Option<&str>) -> Result<Drawing, String> {
        let (Some(layers), Some(entities)) = (old.get("layers").and_then(Value::as_array), old.get("entities").and_then(Value::as_array)) else {
            return Err("Not a CadForge drawing.".into());
        };
        let mut d = Drawing::new(name.unwrap_or("Drawing1"));
        d.layers = layers
            .iter()
            .filter_map(|l| {
                let n = l.get("name")?.as_str()?;
                let mut layer = Layer::new(n);
                if let Some(c) = l.get("color").and_then(Value::as_str).and_then(Color::from_hex) {
                    layer.color = c;
                }
                layer.on = l.get("visible").and_then(Value::as_bool).unwrap_or(true);
                Some(layer)
            })
            .collect();
        if d.layer("0").is_none() {
            d.layers.insert(0, Layer::new("0"));
        }
        d.cur.layer = d.layers[0].name.clone();

        fn p(v: &Value) -> Option<Pt> {
            Some(Pt { x: v.get("x")?.as_f64()?, y: v.get("y")?.as_f64()? })
        }
        fn conv(o: &Value) -> Option<Entity> {
            let pts: Vec<Pt> = o.get("points").and_then(Value::as_array).map(|a| a.iter().filter_map(p).collect()).unwrap_or_default();
            let mut e = match o.get("type")?.as_str()? {
                "line" => Entity::line(*pts.first()?, *pts.get(1)?),
                "circle" => Entity::circle(p(o.get("center")?)?, o.get("radius")?.as_f64()?),
                "polyline" => Entity::pline_pts(&pts, o.get("closed").and_then(Value::as_bool).unwrap_or(false)),
                "text" => Entity::text(*pts.first()?, o.get("height")?.as_f64()?, o.get("text")?.as_str()?),
                _ => return None,
            };
            if let Some(l) = o.get("layer").and_then(Value::as_str) {
                e.layer = l.to_string();
            }
            Some(e)
        }

        let mut uuid_map: HashMap<String, u64> = HashMap::new();
        for o in entities {
            if let Some(e) = conv(o) {
                let id = d.add_to(e, MODEL);
                if let Some(u) = o.get("uuid").and_then(Value::as_str) {
                    uuid_map.insert(u.to_string(), id);
                }
            }
        }
        if let Some(bs) = old.get("blocks").and_then(Value::as_object) {
            for (bn, b) in bs {
                let items: Vec<Entity> = b.get("items").and_then(Value::as_array).map(|a| a.iter().filter_map(conv).collect()).unwrap_or_default();
                d.define_block(bn, Pt { x: 0.0, y: 0.0 }, items);
            }
        }
        if let Some(ss) = old.get("solids").and_then(Value::as_array) {
            for s in ss {
                let mut s = s.clone();
                if let Some(feats) = s.pointer_mut("/extrusionProgram/features").and_then(Value::as_array_mut) {
                    for f in feats.iter_mut() {
                        let mapped = f.get("profile").and_then(Value::as_str).and_then(|u| uuid_map.get(u)).copied();
                        if let (Some(id), Some(fo)) = (mapped, f.as_object_mut()) {
                            fo.insert("profile".into(), serde_json::json!([id]));
                        }
                    }
                    if let Some(prog) = s.get("extrusionProgram").cloned() {
                        s["program"] = prog;
                    }
                }
                if let Ok(sol) = serde_json::from_value::<Solid>(s) {
                    d.solids.push(sol);
                }
            }
        }
        if let Some(ls) = old.get("layouts").and_then(Value::as_object) {
            for (vn, l) in ls {
                if let Some(v) = l.get("view") {
                    if let (Some(x), Some(y), Some(s)) = (v.get("x").and_then(Value::as_f64), v.get("y").and_then(Value::as_f64), v.get("scale").and_then(Value::as_f64)) {
                        d.views.insert(vn.clone(), NamedView { cx: x, cy: y, scale: s, cz: None, yaw: None, pitch: None });
                    }
                }
            }
        }
        d.dirty = false;
        Ok(d)
    }
}

fn kind_name(k: &Kind) -> &'static str {
    match k {
        Kind::Line { .. } => "line",
        Kind::Circle { .. } => "circle",
        Kind::Arc { .. } => "arc",
        Kind::Pline { .. } => "pline",
        Kind::Ellipse(_) => "ellipse",
        Kind::Spline { .. } => "spline",
        Kind::Point { .. } => "point",
        Kind::Text(_) => "text",
        Kind::Dim(_) => "dim",
        Kind::Leader { .. } => "leader",
        Kind::Hatch(_) => "hatch",
        Kind::Insert(_) => "insert",
        Kind::Xline { .. } => "xline",
        Kind::Ray { .. } => "ray",
        Kind::Viewport(_) => "viewport",
    }
}

/// Lineweight of an entity (`LW_BYLAYER` when unset).
pub fn lw_of(e: &Entity) -> i32 {
    e.lw.unwrap_or(crate::style::LW_BYLAYER)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::geom::pt;

    fn line(a: (f64, f64), b: (f64, f64)) -> Entity {
        Entity::line(pt(a.0, a.1), pt(b.0, b.1))
    }

    #[test]
    fn add_fills_layer_and_ids() {
        let mut d = Drawing::new("t");
        d.add_layer("Walls").unwrap();
        d.set_current_layer("walls").unwrap();
        let id = d.add(line((0.0, 0.0), (1.0, 0.0)));
        let id2 = d.add(line((0.0, 0.0), (2.0, 0.0)));
        assert_eq!((id, id2), (1, 2));
        assert_eq!(d.by_id(1).unwrap().layer, "Walls");
        assert!(d.add_layer("WALLS").is_err());
    }

    #[test]
    fn undo_redo_roundtrip() {
        let mut d = Drawing::new("t");
        d.begin_command();
        d.add(line((0.0, 0.0), (1.0, 0.0)));
        d.end_command("LINE");
        d.begin_command();
        d.add(line((0.0, 0.0), (5.0, 0.0)));
        d.end_command("LINE");
        assert_eq!(d.entities().len(), 2);
        assert_eq!(d.undo().as_deref(), Some("LINE"));
        assert_eq!(d.entities().len(), 1);
        d.redo();
        assert_eq!(d.entities().len(), 2);
        // A command that changes nothing leaves no undo step.
        let depth = d.undo_depth();
        d.begin_command();
        d.end_command("NOOP");
        assert_eq!(d.undo_depth(), depth);
        // Ids never go backwards across undo.
        d.undo();
        d.undo();
        let id = d.add(line((0.0, 0.0), (9.0, 0.0)));
        assert_eq!(id, 3);
    }

    #[test]
    fn replace_keeps_id_and_props() {
        let mut d = Drawing::new("t");
        let id = d.add(line((0.0, 0.0), (1.0, 0.0)).with_color(Color::Aci(1)));
        assert!(d.replace(id, line((0.0, 0.0), (3.0, 0.0))));
        let e = d.by_id(id).unwrap();
        assert_eq!(e.color, Some(Color::Aci(1)));
        assert_eq!(e.id, id);
    }

    #[test]
    fn layer_rules() {
        let mut d = Drawing::new("t");
        d.add_layer("A").unwrap();
        assert!(d.delete_layer("0").is_err());
        d.add_to(line((0.0, 0.0), (1.0, 1.0)).on_layer("A"), MODEL);
        assert!(d.delete_layer("A").is_err());
        d.rename_layer("A", "B").unwrap();
        assert_eq!(d.entities()[0].layer, "B");
        d.layer_mut("B").unwrap().frozen = true;
        assert!(d.set_current_layer("B").is_err());
    }

    #[test]
    fn json_roundtrip() {
        let mut d = Drawing::new("t");
        d.add(line((0.0, 0.0), (1.0, 0.0)));
        d.add(Entity::circle(pt(1.0, 1.0), 2.0));
        d.define_block("B", pt(0.0, 0.0), vec![line((0.0, 0.0), (1.0, 1.0))]);
        d.add(Entity::insert("B", pt(5.0, 5.0)));
        let text = d.to_json_string();
        let d2 = Drawing::from_json_str(&text, None).unwrap();
        assert_eq!(d2.entities().len(), 3);
        assert_eq!(d2.blocks["B"].entities.len(), 1);
        assert_eq!(d2.to_json(), d.to_json());
        assert!(Drawing::from_json_str("[]", None).is_err());
    }

    #[test]
    fn validate_rejects_bad_entities() {
        let v = serde_json::json!({
            "format": "cadforge", "version": 2, "layers": [{"name": "0"}],
            "model": {"entities": [{"id": 1, "type": "circle", "layer": "0", "c": {"x": 0, "y": 0}, "r": -1}]}
        });
        assert!(Drawing::from_json(&v, None).err().unwrap().contains("circle"));
    }

    #[test]
    fn legacy_import() {
        let old = serde_json::json!({
            "layers": [{"name": "Walls", "color": "#ff0000", "visible": true}],
            "entities": [
                {"type": "line", "layer": "Walls", "points": [{"x": 0, "y": 0}, {"x": 10, "y": 0}]},
                {"type": "circle", "layer": "Walls", "center": {"x": 1, "y": 1}, "radius": 2},
                {"type": "polyline", "layer": "Walls", "points": [{"x": 0, "y": 0}, {"x": 1, "y": 1}, {"x": 2, "y": 0}], "closed": true},
                {"type": "text", "layer": "Walls", "points": [{"x": 0, "y": 0}], "height": 2.5, "text": "hi"},
                {"type": "weird"}
            ]
        });
        let d = Drawing::from_json(&old, Some("old")).unwrap();
        assert_eq!(d.entities().len(), 4);
        assert!(d.layer("0").is_some());
        assert_eq!(d.cur.layer, "0");
        assert_eq!(d.name, "old");
    }

    #[test]
    fn caches_clear_on_touch() {
        let mut d = Drawing::new("t");
        let id = d.add(line((0.0, 0.0), (1.0, 0.0)));
        let e = d.by_id(id).unwrap().clone();
        let a = crate::entities::segs_of(&e, &d);
        let b = crate::entities::segs_of(&e, &d);
        assert!(Rc::ptr_eq(&a, &b));
        d.touch();
        let c = crate::entities::segs_of(&e, &d);
        assert!(!Rc::ptr_eq(&a, &c));
    }
}
