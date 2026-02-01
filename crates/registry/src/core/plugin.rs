use crate::RegistryMeta;
use crate::db::builder::RegistryDbBuilder;
use crate::error::RegistryError;
use crate::traits::RegistryEntry;

/// A plugin descriptor that registers multiple items into the registry.
///
/// Underutilized: all three submitted plugins (themes, statusline, options)
/// are already registered by `builtins::register_all`, making the
/// `inventory`-driven `run_plugins` path redundant.
pub struct PluginDef {
	/// Metadata for the plugin itself.
	pub meta: RegistryMeta,
	/// Function called during registry build to register all items from this plugin.
	pub register: fn(&mut RegistryDbBuilder) -> Result<(), RegistryError>,
}

inventory::collect!(PluginDef);

impl PluginDef {
	/// Creates a new plugin definition.
	pub const fn new(
		meta: RegistryMeta,
		register: fn(&mut RegistryDbBuilder) -> Result<(), RegistryError>,
	) -> Self {
		Self { meta, register }
	}
}

impl RegistryEntry for PluginDef {
	fn meta(&self) -> &RegistryMeta {
		&self.meta
	}
}
