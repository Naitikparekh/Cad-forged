//! Commands CadForge lacked against other open-source CAD programs (parity batch: ext_parity).
//!
//! COUNT, COUNTLIST, COUNTTABLE, BURST, NCOPY and BLOCKREPLACE (`blocks`); REVERSE, TEXTTOFRONT and HATCHTOBACK
//! (`edit`); DIMOVERRIDE and DIMJOGLINE (`dims`).
use crate::runtime::PromptSpec;

mod blocks;
mod dims;
mod edit;

fn ps(m: &str) -> PromptSpec {
    PromptSpec::new(m)
}

fn plural(n: usize, word: &str) -> String {
    format!("{n} {word}{}", if n == 1 { "" } else { "s" })
}

pub fn register(r: &mut crate::registry::Registry) {
    blocks::register(r);
    edit::register(r);
    dims::register(r);
}
