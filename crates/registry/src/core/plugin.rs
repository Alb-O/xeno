use crate::RegistryMeta;
use crate::db::builder::RegistryDbBuilder;

/// A plugin descriptor that registers multiple items into the registry.
pub struct PluginDef {
	/// Metadata for the plugin itself.
	pub meta: RegistryMeta,
	/// Function called during registry build to register all items from this plugin.
	pub register: fn(&mut RegistryDbBuilder),
}

inventory::collect!(PluginDef);

impl PluginDef {
	/// Creates a new plugin definition.
	pub const fn new(meta: RegistryMeta, register: fn(&mut RegistryDbBuilder)) -> Self {
		Self { meta, register }
	}
}
