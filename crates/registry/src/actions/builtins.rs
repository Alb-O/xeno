pub(crate) mod editing;
pub(crate) mod find;
pub(crate) mod insert;
pub(crate) mod misc;
pub(crate) mod modes;
pub(crate) mod navigation;
pub(crate) mod scrolling;
pub(crate) mod search;
pub(crate) mod selection;
pub(crate) mod text_objects;
pub(crate) mod window;

pub use navigation::{cursor_motion, selection_motion};

use crate::db::builder::RegistryDbBuilder;

pub fn register_builtins(builder: &mut RegistryDbBuilder) {
	builder.register_compiled_actions();
}

fn register_builtins_reg(
	builder: &mut RegistryDbBuilder,
) -> Result<(), crate::db::builder::RegistryError> {
	register_builtins(builder);
	Ok(())
}

inventory::submit!(crate::db::builtins::BuiltinsReg {
	ordinal: 10,
	f: register_builtins_reg,
});
