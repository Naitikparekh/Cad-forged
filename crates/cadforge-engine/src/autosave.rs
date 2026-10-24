//! Autosave and crash recovery.
//!
//! The host calls [`App::autosave_tick`] regularly. Every `SAVETIME` minutes it gets back the drawings that changed
//! since their last autosave (as project JSON, to be stored wherever the host keeps recovery files) and the keys of
//! recovery files that are no longer needed (the drawing was saved or closed). After a crash the host hands the stored
//! files to [`App::recover_autosave`], which reopens each as an unsaved drawing and keeps its file until it is saved.
use crate::app::App;
use cadforge_core::drawing::Drawing;
use std::collections::HashMap;

/// Bookkeeping, one per [`App`].
#[derive(Default)]
pub struct AutoSaveState {
    /// When the last autosave pass ran (`App::now`, milliseconds).
    last_pass: Option<f64>,
    /// Editor id -> (recovery file key, drawing revision that was written).
    written: HashMap<u64, (String, u64)>,
    /// Keys of recovery files whose drawing was closed.
    orphans: Vec<String>,
    /// Makes keys unique across sessions (set by the host, e.g. from the clock at start-up).
    pub salt: String,
}

impl AutoSaveState {
    pub(crate) fn editor_closed(&mut self, editor_id: u64) {
        if let Some((key, _)) = self.written.remove(&editor_id) {
            self.orphans.push(key);
        }
    }
}

/// One recovery file to store.
#[derive(Clone, Debug, PartialEq)]
pub struct AutoSaveFile {
    pub key: String,
    pub name: String,
    pub json: String,
}

#[derive(Debug, Default, PartialEq)]
pub struct AutoSavePlan {
    pub write: Vec<AutoSaveFile>,
    pub remove: Vec<String>,
}

fn file_safe(name: &str) -> String {
    let s: String = name.chars().map(|c| if c.is_ascii_alphanumeric() || c == '-' || c == '_' { c } else { '_' }).collect();
    s.chars().take(40).collect()
}

impl App {
    /// Work out what to write and what to delete. `force` ignores the timer (used when the page is about to close).
    pub fn autosave_tick(&mut self, force: bool) -> AutoSavePlan {
        let mut plan = AutoSavePlan { write: vec![], remove: std::mem::take(&mut self.autosave.orphans) };
        // saved drawings no longer need their recovery file
        for ed in &self.editors {
            if !ed.doc.dirty {
                if let Some((key, _)) = self.autosave.written.remove(&ed.id) {
                    plan.remove.push(key);
                }
            }
        }
        let minutes = self.sysvars.real("SAVETIME");
        if minutes <= 0.0 && !force {
            return plan;
        }
        let due = match self.autosave.last_pass {
            None => {
                self.autosave.last_pass = Some(self.now);
                false
            }
            Some(t) => self.now - t >= minutes * 60_000.0,
        };
        if !(due || force) {
            return plan;
        }
        self.autosave.last_pass = Some(self.now);
        for ed in &self.editors {
            if !ed.doc.dirty {
                continue;
            }
            if self.autosave.written.get(&ed.id).is_some_and(|(_, rev)| *rev == ed.doc.rev) {
                continue;
            }
            let key = match self.autosave.written.get(&ed.id) {
                Some((k, _)) => k.clone(),
                None => format!("{}-{}-{}", file_safe(&self.autosave.salt), ed.id, file_safe(&ed.doc.name)),
            };
            plan.write.push(AutoSaveFile { key: key.clone(), name: ed.doc.name.clone(), json: ed.doc.to_json_string() });
            self.autosave.written.insert(ed.id, (key, ed.doc.rev));
        }
        plan
    }

    /// Reopen a recovery file left by a session that did not end cleanly. The drawing opens unsaved and keeps its
    /// recovery file until it is saved or closed.
    pub fn recover_autosave(&mut self, key: &str, json: &str) -> Result<String, String> {
        let mut doc = Drawing::from_json_str(json, None)?;
        doc.name = format!("{} (recovered)", doc.name.trim_end_matches(" (recovered)"));
        let name = doc.name.clone();
        let id = self.add_drawing(doc);
        if let Some(ed) = self.editors.iter_mut().find(|e| e.id == id) {
            ed.doc.dirty = true;
            self.autosave.written.insert(id, (key.to_string(), ed.doc.rev));
        }
        Ok(name)
    }

    /// Recovery files that exist for the open drawings, newest pass first (for DRAWINGRECOVERY).
    pub fn autosave_status(&self) -> Vec<(String, String)> {
        self.editors.iter().filter_map(|e| self.autosave.written.get(&e.id).map(|(k, _)| (e.doc.name.clone(), k.clone()))).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::commands::build_registry;
    use cadforge_core::entities::Entity;
    use cadforge_core::geom::pt;
    use std::rc::Rc;

    fn app() -> App {
        let mut a = App::new(Rc::new(build_registry()));
        a.autosave.salt = "s1".into();
        a.new_drawing(None, false);
        a
    }

    fn draw(a: &mut App) {
        let ed = a.editor_mut().unwrap();
        ed.doc.add(Entity::line(pt(0., 0.), pt(1., 1.)));
        ed.doc.dirty = true;
    }

    #[test]
    fn nothing_is_written_before_the_interval_or_for_clean_drawings() {
        let mut a = app();
        a.now = 0.0;
        assert_eq!(a.autosave_tick(false), AutoSavePlan::default(), "first pass only starts the clock");
        draw(&mut a);
        a.now = 9.0 * 60_000.0;
        assert!(a.autosave_tick(false).write.is_empty(), "SAVETIME is 10 minutes");
        a.now = 10.0 * 60_000.0;
        let plan = a.autosave_tick(false);
        assert_eq!(plan.write.len(), 1);
        assert!(plan.write[0].key.starts_with("s1-"));
        assert_eq!(plan.write[0].name, "Drawing1");
        // a clean drawing is never written
        let mut b = app();
        b.now = 0.0;
        b.autosave_tick(false);
        b.now = 11.0 * 60_000.0;
        assert!(b.autosave_tick(false).write.is_empty());
    }

    #[test]
    fn a_drawing_is_written_again_only_after_it_changed() {
        let mut a = app();
        a.now = 0.0;
        a.autosave_tick(false);
        draw(&mut a);
        a.now = 10.0 * 60_000.0;
        let first = a.autosave_tick(false);
        assert_eq!(first.write.len(), 1);
        a.now = 20.0 * 60_000.0;
        assert!(a.autosave_tick(false).write.is_empty(), "unchanged since the last pass");
        a.editor_mut().unwrap().doc.touch();
        a.now = 30.0 * 60_000.0;
        let again = a.autosave_tick(false);
        assert_eq!(again.write.len(), 1);
        assert_eq!(again.write[0].key, first.write[0].key, "the same recovery file is rewritten");
    }

    #[test]
    fn saving_or_closing_removes_the_recovery_file() {
        let mut a = app();
        a.now = 0.0;
        a.autosave_tick(false);
        draw(&mut a);
        a.now = 10.0 * 60_000.0;
        let key = a.autosave_tick(false).write[0].key.clone();
        // saved: the drawing is clean
        a.editor_mut().unwrap().doc.dirty = false;
        assert_eq!(a.autosave_tick(false).remove, vec![key.clone()]);
        assert!(a.autosave_tick(false).remove.is_empty(), "reported once");
        // dirty again, written again, then the tab is closed
        draw(&mut a);
        a.now = 20.0 * 60_000.0;
        let key2 = a.autosave_tick(false).write[0].key.clone();
        a.close_editor(0);
        assert_eq!(a.autosave_tick(false).remove, vec![key2]);
    }

    #[test]
    fn savetime_zero_turns_it_off_but_force_still_writes() {
        let mut a = app();
        a.sysvars.set("SAVETIME", 0.0, None).unwrap();
        a.now = 0.0;
        a.autosave_tick(false);
        draw(&mut a);
        a.now = 60.0 * 60_000.0;
        assert!(a.autosave_tick(false).write.is_empty());
        assert_eq!(a.autosave_tick(true).write.len(), 1, "forced (page closing)");
    }

    #[test]
    fn a_recovery_file_reopens_unsaved_and_survives_until_the_drawing_is_saved() {
        let mut a = app();
        a.now = 0.0;
        a.autosave_tick(false);
        draw(&mut a);
        a.now = 10.0 * 60_000.0;
        let file = a.autosave_tick(false).write.remove(0);

        // a new session finds the file
        let mut b = App::new(Rc::new(build_registry()));
        b.autosave.salt = "s2".into();
        let name = b.recover_autosave(&file.key, &file.json).unwrap();
        assert_eq!(name, "Drawing1 (recovered)");
        let ed = b.editor().unwrap();
        assert!(ed.doc.dirty, "unsaved");
        assert_eq!(ed.doc.model.entities.len(), 1);
        b.now = 0.0;
        b.autosave_tick(false);
        b.now = 11.0 * 60_000.0;
        assert!(b.autosave_tick(false).remove.is_empty(), "the file stays while the drawing is unsaved");
        assert_eq!(b.autosave_status(), vec![("Drawing1 (recovered)".to_string(), file.key.clone())]);
        // saving removes it
        b.editor_mut().unwrap().doc.dirty = false;
        assert_eq!(b.autosave_tick(false).remove, vec![file.key]);
        assert!(b.recover_autosave("k", "not json").is_err());
    }
}

/// The backup copy kept when a drawing is saved over an existing file (ISAVEBAK): `plan.dwg` becomes `plan.bak`.
pub fn backup_path(path: &std::path::Path) -> std::path::PathBuf {
    path.with_extension("bak")
}

/// Write a drawing file. With `keep_backup` (ISAVEBAK 1) the file it replaces is first copied to its backup name.
/// Returns the backup's path when one was made.
#[cfg(not(target_arch = "wasm32"))]
pub fn write_keeping_backup(path: &std::path::Path, bytes: &[u8], keep_backup: bool) -> std::io::Result<Option<std::path::PathBuf>> {
    let mut bak = None;
    if keep_backup && path.is_file() {
        let b = backup_path(path);
        if b != path {
            std::fs::copy(path, &b)?;
            bak = Some(b);
        }
    }
    std::fs::write(path, bytes)?;
    Ok(bak)
}
