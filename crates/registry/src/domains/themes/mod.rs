//! Theme and syntax highlighting registry

pub use xeno_primitives::{Color, Mode, Modifier, Style};

#[path = "runtime/syntax/mod.rs"]
pub mod syntax;
#[path = "runtime/theme/mod.rs"]
pub mod theme;

mod domain;
#[path = "compile/link.rs"]
pub mod link;
#[path = "compile/loader.rs"]
pub mod loader;
#[path = "contract/spec.rs"]
pub mod spec;

pub use domain::Themes;
pub use syntax::{SyntaxStyle, SyntaxStyles};
pub use theme::{LinkedThemeDef, ThemeDef as Theme, *};

/// Registers compiled themes from the embedded spec.
pub fn register_compiled(db: &mut crate::db::builder::RegistryDbBuilder) {
	let spec = loader::load_themes_spec();
	let linked = link::link_themes(&spec);

	for def in linked {
		db.push_domain::<Themes>(ThemeInput::Linked(def));
	}
}

pub fn register_builtins(builder: &mut crate::db::builder::RegistryDbBuilder) {
	register_compiled(builder);
}

fn register_builtins_reg(builder: &mut crate::db::builder::RegistryDbBuilder) -> Result<(), crate::db::builder::RegistryError> {
	register_builtins(builder);
	Ok(())
}

inventory::submit!(crate::db::builtins::BuiltinsReg {
	ordinal: 60,
	f: register_builtins_reg,
});
