//! The AutoCAD-style command set. Each module registers its commands with [`Registry`].
use crate::ctx::Ctx;
use crate::registry::{CommandDef, Registry, RunFn};
use crate::runtime::{Args, CmdResult};
use std::future::Future;
use std::rc::Rc;

pub mod draw;
pub mod helpers;
pub mod modify;

/// Wrap an `async fn(Ctx, Args) -> CmdResult<()>` as a registry entry point.
pub fn run<F, Fut>(f: F) -> RunFn
where
    F: Fn(Ctx, Args) -> Fut + 'static,
    Fut: Future<Output = CmdResult<()>> + 'static,
{
    Rc::new(move |c, a| Box::pin(f(c, a)))
}

/// Start a definition in a command group (`Draw`, `Modify`...).
pub fn def(group: &str, name: &str, f: RunFn) -> CommandDef {
    CommandDef::new(name, f).group(group)
}

/// A registry holding every command, aliases and synonyms.
pub fn build_registry() -> Registry {
    let mut r = Registry::default();
    draw::register(&mut r);
    modify::register(&mut r);
    r
}
