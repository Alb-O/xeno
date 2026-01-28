//! Theme and syntax highlighting registry

pub use xeno_primitives::{Color, Mode, Modifier, Style};

use crate::impl_registry_entry;

pub mod syntax;
pub mod theme;

pub use syntax::{SyntaxStyle, SyntaxStyles};
pub use theme::{ThemeDef as Theme, *};

impl_registry_entry!(ThemeDef);

pub fn register_builtins(builder: &mut crate::db::builder::RegistryDbBuilder) {
	builder.register_theme(&theme::DEFAULT_THEME);
}

use crate::error::RegistryError;

pub fn register_plugin(
	db: &mut crate::db::builder::RegistryDbBuilder,
) -> Result<(), RegistryError> {
	register_builtins(db);
	Ok(())
}

inventory::submit! {
	crate::PluginDef::new(
		crate::RegistryMeta::minimal("themes-builtin", "Themes Builtin", "Builtin theme set"),
		register_plugin
	)
}
