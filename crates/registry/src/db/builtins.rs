use crate::db::builder::{RegistryDbBuilder, RegistryError};

/// Registers all built-in registry items with the provided builder.
pub fn register_all(builder: &mut RegistryDbBuilder) -> Result<(), RegistryError> {
	crate::actions::register_builtins(builder);
	crate::commands::register_builtins(builder);
	crate::motions::register_builtins(builder);
	crate::textobj::register_builtins(builder);
	crate::options::register_builtins(builder);
	crate::themes::register_builtins(builder);
	crate::gutter::register_builtins(builder);
	crate::statusline::register_builtins(builder);
	crate::hooks::register_builtins(builder);
	crate::notifications::register_builtins(builder);
	Ok(())
}
