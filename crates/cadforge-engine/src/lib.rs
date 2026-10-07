//! CadForge engine: input pipeline, object snaps, selection, the async command runtime and the AutoCAD-style
//! command set. UI-agnostic: the egui front end (and the headless tests) drive it through [`app::Handle`].
pub mod app;
pub mod ctx;
pub mod editor;
pub mod memory;
pub mod parse;
pub mod registry;
pub mod runtime;
pub mod select;
pub mod snap;
pub mod solid_pick;
pub mod sysvars;
pub mod view;
pub mod commands;
pub mod sim;
