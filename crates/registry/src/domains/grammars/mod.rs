//! Grammar domain loaders and static specification wiring.

#[path = "compile/loader.rs"]
pub mod loader;
#[path = "contract/spec.rs"]
pub mod spec;
mod domain;

pub use domain::Grammars;
