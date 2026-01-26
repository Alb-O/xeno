//! Theme and syntax highlighting registry

pub use xeno_primitives::{Color, Mode, Modifier, Style};
use xeno_registry_core::impl_registry_entry;

pub mod syntax;
pub mod theme;

pub use syntax::{SyntaxStyle, SyntaxStyles};
pub use theme::{ThemeDef as Theme, *};

impl_registry_entry!(ThemeDef);
