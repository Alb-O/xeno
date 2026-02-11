mod buffer;
mod edit;
mod help;
mod quit;
mod registry;
mod set;
mod theme;
mod write;

use crate::db::builder::RegistryDbBuilder;

pub fn register_builtins(builder: &mut RegistryDbBuilder) {
	crate::commands::register_compiled(builder);
}

fn register_builtins_reg(builder: &mut RegistryDbBuilder) -> Result<(), crate::db::builder::RegistryError> {
	register_builtins(builder);
	Ok(())
}

inventory::submit!(crate::db::builtins::BuiltinsReg {
	ordinal: 20,
	f: register_builtins_reg,
});
