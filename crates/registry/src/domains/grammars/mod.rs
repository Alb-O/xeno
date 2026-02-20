//! Grammar domain loaders and static specification wiring.

mod domain;
#[path = "compile/loader.rs"]
pub mod loader;
#[path = "contract/spec.rs"]
pub mod spec;

pub use domain::Grammars;
