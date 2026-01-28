use super::builder::{RegistryDbBuilder, RegistryError};
use crate::core::plugin::PluginDef;
use crate::core::traits::RegistryEntry;

pub fn run_plugins(builder: &mut RegistryDbBuilder) -> Result<(), RegistryError> {
	let mut plugins: Vec<&'static PluginDef> = inventory::iter::<PluginDef>.into_iter().collect();
	// Sort by total order (higher priority wins/runs later if we want it to override?
	// Actually total_order_cmp is used for winner selection.
	// Let's just use it for stable ordering.
	plugins.sort_by(|a, b| a.total_order_cmp(b));

	for plugin in plugins {
		builder.register_plugin(plugin)?;
	}

	Ok(())
}

// Keep legacy stuff for now if needed, but we'll move away from it
pub trait XenoPlugin {
	const ID: &'static str;
	fn register(db: &mut RegistryDbBuilder) -> Result<(), RegistryError>;
}
