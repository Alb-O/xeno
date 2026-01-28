use super::builder::{RegistryDbBuilder, RegistryError};
use crate::core::plugin::PluginDef;

pub fn run_plugins(builder: &mut RegistryDbBuilder) -> Result<(), RegistryError> {
	let mut plugins: Vec<&'static PluginDef> = inventory::iter::<PluginDef>.into_iter().collect();
	plugins.sort_by(|a, b| {
		b.meta
			.priority
			.cmp(&a.meta.priority)
			.then_with(|| a.meta.source.rank().cmp(&b.meta.source.rank()))
			.then_with(|| a.meta.id.cmp(b.meta.id))
	});

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
