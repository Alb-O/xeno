//! Theme and syntax highlighting registry

pub use xeno_primitives::{Color, Mode, Modifier, Style};

pub mod syntax;
pub mod theme;

pub use syntax::{SyntaxStyle, SyntaxStyles};
pub use theme::{ThemeDef as Theme, *};

pub fn register_builtins(builder: &mut crate::db::builder::RegistryDbBuilder) {
	builder.register_theme(&theme::DEFAULT_THEME);
}
