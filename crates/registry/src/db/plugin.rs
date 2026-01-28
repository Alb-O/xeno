use super::builder::{RegistryDbBuilder, RegistryError};
use crate::core::plugin::PluginDef;

pub fn run_plugins(builder: &mut RegistryDbBuilder) -> Result<(), RegistryError> {
	for entry in inventory::iter::<PluginDef> {
		(entry.register)(builder);
	}
	Ok(())
}

// Keep legacy stuff for now if needed, but we'll move away from it
pub trait XenoPlugin {
	const ID: &'static str;
	fn register(db: &mut RegistryDbBuilder) -> Result<(), RegistryError>;
}
