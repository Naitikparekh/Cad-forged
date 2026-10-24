//! The AutoCAD-style command set. Each module registers its commands with [`Registry`].
use crate::ctx::Ctx;
use crate::registry::{CommandDef, Registry, RunFn};
use crate::runtime::{Args, CmdResult};
use std::future::Future;
use std::rc::Rc;

pub mod aliases;
pub mod annotate;
pub mod blocks;
pub mod draw;
pub mod ext_annotate;
pub mod ext_blocks;
pub mod ext_modify;
pub mod ext_solids;
pub mod ext_system;
pub mod ext_tail;
pub mod helpers;
pub mod inquiry;
pub mod layer;
pub mod layouts;
pub mod modify;
pub mod solids;
pub mod system;
pub mod ucs;
pub mod view;

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
    annotate::register(&mut r);
    inquiry::register(&mut r);
    layer::register(&mut r);
    view::register(&mut r);
    blocks::register(&mut r);
    solids::register(&mut r);
    system::register(&mut r);
    layouts::register(&mut r);
    ucs::register(&mut r);
    ext_solids::register(&mut r);
    ext_annotate::register(&mut r);
    ext_modify::register(&mut r);
    ext_blocks::register(&mut r);
    ext_tail::register(&mut r);
    ext_system::register(&mut r);
    aliases::install(&mut r);
    r
}
