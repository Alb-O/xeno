//! Theme and syntax highlighting registry

pub use xeno_primitives::{Color, Mode, Modifier, Style};

pub mod syntax;
pub mod theme;

pub mod link;
pub mod loader;
pub mod spec;

pub use syntax::{SyntaxStyle, SyntaxStyles};
pub use theme::{LinkedThemeDef, ThemeDef as Theme, *};

pub fn register_plugin(
	db: &mut crate::db::builder::RegistryDbBuilder,
) -> Result<(), crate::error::RegistryError> {
	register_compiled(db);
	Ok(())
}

/// Registers compiled themes from the embedded spec.
pub fn register_compiled(db: &mut crate::db::builder::RegistryDbBuilder) {
	let spec = loader::load_themes_spec();
	let linked = link::link_themes(&spec);

	for def in linked {
		db.push_domain::<Themes>(ThemeInput::Linked(def));
	}
}

pub struct Themes;

impl crate::db::domain::DomainSpec for Themes {
	type Input = ThemeInput;
	type Entry = ThemeEntry;
	type Id = crate::core::ThemeId;
	type StaticDef = ThemeDef;
	type LinkedDef = LinkedThemeDef;
	const LABEL: &'static str = "themes";

	fn static_to_input(def: &'static Self::StaticDef) -> Self::Input {
		ThemeInput::Static(*def)
	}

	fn linked_to_input(def: Self::LinkedDef) -> Self::Input {
		ThemeInput::Linked(def)
	}

	fn builder(
		db: &mut crate::db::builder::RegistryDbBuilder,
	) -> &mut crate::core::index::RegistryBuilder<Self::Input, Self::Entry, Self::Id> {
		&mut db.themes
	}
}

pub fn register_builtins(builder: &mut crate::db::builder::RegistryDbBuilder) {
	register_compiled(builder);
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
