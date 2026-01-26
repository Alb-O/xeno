//! Theme and syntax highlighting registry

pub use xeno_primitives::{Color, Mode, Modifier, Style};
use xeno_registry_core::impl_registry_entry;

mod syntax;
mod theme;

pub use syntax::{SyntaxStyle, SyntaxStyles};
pub use theme::*;

impl_registry_entry!(Theme);
