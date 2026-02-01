use super::builder::{RegistryDbBuilder, RegistryError};
use crate::core::plugin::PluginDef;
use crate::core::traits::RegistryEntry;

/// Iterates `PluginDef` items collected via `inventory` and registers them.
///
/// Underutilized: redundant with `builtins::register_all` which runs first
/// in `get_db()` and registers the same definitions.
pub fn run_plugins(builder: &mut RegistryDbBuilder) -> Result<(), RegistryError> {
	let mut plugins: Vec<&'static PluginDef> = inventory::iter::<PluginDef>.into_iter().collect();
	plugins.sort_by(|a, b| a.total_order_cmp(b));

	for plugin in plugins {
		builder.register_plugin(plugin)?;
	}

	Ok(())
}

/// Legacy plugin trait — unused.
pub trait XenoPlugin {
	const ID: &'static str;
	fn register(db: &mut RegistryDbBuilder) -> Result<(), RegistryError>;
}
