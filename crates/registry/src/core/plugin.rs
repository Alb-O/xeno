use crate::core::meta::RegistryMetaStatic;
use crate::db::builder::RegistryDbBuilder;
use crate::error::RegistryError;

/// A plugin descriptor that registers multiple items into the registry.
///
/// Underutilized: all three submitted plugins (themes, statusline, options)
/// are already registered by `builtins::register_all`, making the
/// `inventory`-driven `run_plugins` path redundant.
pub struct PluginDef {
	/// Metadata for the plugin itself.
	pub meta: RegistryMetaStatic,
	/// Function called during registry build to register all items from this plugin.
	pub register: fn(&mut RegistryDbBuilder) -> Result<(), RegistryError>,
}

inventory::collect!(PluginDef);

impl PluginDef {
	/// Creates a new plugin definition.
	pub const fn new(
		meta: RegistryMetaStatic,
		register: fn(&mut RegistryDbBuilder) -> Result<(), RegistryError>,
	) -> Self {
		Self { meta, register }
	}
}
