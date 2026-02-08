//! Theme and syntax highlighting registry

pub use xeno_primitives::{Color, Mode, Modifier, Style};

pub mod syntax;
pub mod theme;

pub mod link;
pub mod loader;
pub mod spec;

pub use syntax::{SyntaxStyle, SyntaxStyles};
pub use theme::{LinkedThemeDef, ThemeDef as Theme, *};

pub fn register_builtins(builder: &mut crate::db::builder::RegistryDbBuilder) {
	builder.register_compiled_themes();
}

fn register_builtins_reg(
	builder: &mut crate::db::builder::RegistryDbBuilder,
) -> Result<(), crate::db::builder::RegistryError> {
	register_builtins(builder);
	Ok(())
}

inventory::submit!(crate::db::builtins::BuiltinsReg {
	ordinal: 60,
	f: register_builtins_reg,
});
