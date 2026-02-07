use super::builder::{RegistryDbBuilder, RegistryError};
use crate::core::plugin::PluginDef;

/// Iterates `PluginDef` items collected via `inventory` and registers them.
///
/// Underutilized: redundant with `builtins::register_all` which runs first
/// in `get_db()` and registers the same definitions.
pub fn run_plugins(builder: &mut RegistryDbBuilder) -> Result<(), RegistryError> {
	let mut plugins: Vec<&'static PluginDef> = inventory::iter::<PluginDef>.into_iter().collect();
	plugins.sort_by(|a, b| {
		a.meta
			.priority
			.cmp(&b.meta.priority)
			.then_with(|| a.meta.source.rank().cmp(&b.meta.source.rank()))
			.then_with(|| a.meta.id.cmp(b.meta.id))
	});

	for plugin in plugins {
		builder.register_plugin(plugin)?;
	}

	Ok(())
}
