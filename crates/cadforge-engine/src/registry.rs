//! Command registry: definitions, aliases (acad.pgp style, last definition wins) and synonyms
//! (port of `src/engine/registry.js`).
use crate::ctx::Ctx;
use crate::runtime::{Args, CmdFuture};
use indexmap::IndexMap;
use std::collections::HashMap;
use std::rc::Rc;

pub type RunFn = Rc<dyn Fn(Ctx, Args) -> CmdFuture>;

#[derive(Clone)]
pub struct CommandDef {
    pub name: String,
    pub aliases: Vec<String>,
    pub run: RunFn,
    pub transparent: bool,
    pub pickfirst: bool,
    pub group: String,
    pub title: String,
    pub desc: String,
    pub icon: String,
    pub repeatable: bool,
    /// Do not echo `Command: NAME` in the history.
    pub silent: bool,
}

impl CommandDef {
    pub fn new(name: &str, run: RunFn) -> CommandDef {
        CommandDef {
            name: name.to_uppercase(),
            aliases: vec![],
            run,
            transparent: false,
            pickfirst: true,
            group: String::new(),
            title: name.to_string(),
            desc: String::new(),
            icon: String::new(),
            repeatable: true,
            silent: false,
        }
    }
    pub fn aliases(mut self, a: &[&str]) -> Self {
        self.aliases = a.iter().map(|s| s.to_string()).collect();
        self
    }
    pub fn transparent(mut self) -> Self {
        self.transparent = true;
        self
    }
    pub fn no_pickfirst(mut self) -> Self {
        self.pickfirst = false;
        self
    }
    pub fn title(mut self, t: &str) -> Self {
        self.title = t.into();
        self
    }
    pub fn desc(mut self, d: &str) -> Self {
        self.desc = d.into();
        self
    }
    pub fn icon(mut self, i: &str) -> Self {
        self.icon = i.into();
        self
    }
    pub fn group(mut self, g: &str) -> Self {
        self.group = g.into();
        self
    }
    pub fn once(mut self) -> Self {
        self.repeatable = false;
        self
    }
    pub fn silent(mut self) -> Self {
        self.silent = true;
        self
    }
}

#[derive(Default)]
pub struct Registry {
    commands: IndexMap<String, Rc<CommandDef>>,
    aliases: HashMap<String, String>,
    /// User-defined aliases (edited from the UI while the registry is shared).
    pub user_aliases: std::cell::RefCell<HashMap<String, String>>,
    /// Commands defined at run time by Lisp (`(defun c:name ...)`); they take precedence over built-in ones.
    pub user_commands: std::cell::RefCell<IndexMap<String, Rc<CommandDef>>>,
    pub synonyms: HashMap<String, String>,
}

impl Registry {
    pub fn define(&mut self, def: CommandDef) {
        let name = def.name.to_uppercase();
        for a in &def.aliases {
            self.aliases.insert(a.to_uppercase(), name.clone());
        }
        self.commands.insert(name, Rc::new(def));
    }

    pub fn alias(&mut self, alias: &str, name: &str) {
        self.aliases
            .insert(alias.to_uppercase(), name.to_uppercase());
    }

    pub fn has_command(&self, name: &str) -> bool {
        let n = name.to_uppercase();
        self.commands.contains_key(&n) || self.user_commands.borrow().contains_key(&n)
    }

    pub fn has_alias(&self, alias: &str) -> bool {
        self.aliases.contains_key(&alias.to_uppercase())
    }

    pub fn get(&self, name: &str) -> Option<Rc<CommandDef>> {
        let n = name.to_uppercase();
        self.user_commands
            .borrow()
            .get(&n)
            .cloned()
            .or_else(|| self.commands.get(&n).cloned())
    }

    /// Add (or replace) a command defined while the program runs.
    pub fn define_user(&self, def: CommandDef) {
        self.user_commands
            .borrow_mut()
            .insert(def.name.to_uppercase(), Rc::new(def));
    }

    /// Resolve typed text to a command definition (exact name, user alias, alias, synonym).
    pub fn find(&self, text: &str) -> Option<Rc<CommandDef>> {
        let t = text.trim().to_uppercase();
        if t.is_empty() {
            return None;
        }
        if let Some(d) = self.user_commands.borrow().get(&t) {
            return Some(d.clone());
        }
        let key = self
            .user_aliases
            .borrow()
            .get(&t)
            .cloned()
            .or_else(|| self.aliases.get(&t).cloned())
            .or_else(|| self.commands.contains_key(&t).then(|| t.clone()))
            .or_else(|| self.synonyms.get(&t).cloned());
        match key {
            Some(k) => self.commands.get(&k).cloned(),
            None if t.starts_with('-') => self.commands.get(&t).cloned(),
            None => None,
        }
    }

    /// Every default alias with its command (sorted).
    pub fn all_aliases(&self) -> Vec<(String, String)> {
        let mut v: Vec<(String, String)> = self
            .aliases
            .iter()
            .map(|(a, c)| (a.clone(), c.clone()))
            .collect();
        v.sort();
        v
    }

    /// The shortest alias of a command (for tooltips).
    pub fn alias_of(&self, name: &str) -> Option<String> {
        let n = name.to_uppercase();
        self.aliases
            .iter()
            .filter(|(_, c)| **c == n)
            .map(|(a, _)| a.clone())
            .min_by(|a, b| a.len().cmp(&b.len()).then_with(|| a.cmp(b)))
    }

    pub fn names(&self) -> Vec<String> {
        let mut v: Vec<String> = self.commands.keys().cloned().collect();
        v.sort();
        v
    }

    pub fn defs(&self) -> impl Iterator<Item = &Rc<CommandDef>> {
        self.commands.values()
    }

    /// Typed-input suggestions: commands first by prefix, then mid-string matches.
    pub fn suggest(
        &self,
        text: &str,
        mid_string: bool,
        extra: &[String],
        limit: usize,
    ) -> Vec<String> {
        let t = text.trim().to_uppercase();
        if t.is_empty() {
            return vec![];
        }
        let user: Vec<String> = self.user_commands.borrow().keys().cloned().collect();
        let pool: Vec<String> = self
            .commands
            .keys()
            .cloned()
            .chain(user)
            .chain(extra.iter().cloned())
            .collect();
        let mut starts: Vec<&String> = pool.iter().filter(|n| n.starts_with(&t)).collect();
        starts.sort_by(|a, b| a.len().cmp(&b.len()).then_with(|| a.cmp(b)));
        let mut mid: Vec<&String> = if mid_string {
            pool.iter()
                .filter(|n| !n.starts_with(&t) && n.contains(&t))
                .collect()
        } else {
            vec![]
        };
        mid.sort();
        let target = self
            .user_aliases
            .borrow()
            .get(&t)
            .cloned()
            .or_else(|| self.aliases.get(&t).cloned())
            .or_else(|| self.commands.contains_key(&t).then(|| t.clone()));
        let mut out: Vec<String> = vec![];
        for s in target
            .into_iter()
            .chain(starts.into_iter().cloned())
            .chain(mid.into_iter().cloned())
        {
            if !out.contains(&s) {
                out.push(s);
            }
        }
        out.truncate(limit);
        out
    }
}
