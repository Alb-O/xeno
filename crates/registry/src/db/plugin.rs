use super::builder::{RegistryDbBuilder, RegistryError};

pub trait XenoPlugin {
	const ID: &'static str;
	fn register(db: &mut RegistryDbBuilder) -> Result<(), RegistryError>;
}

pub struct PluginEntry {
	pub id: &'static str,
	pub register: fn(&mut RegistryDbBuilder) -> Result<(), RegistryError>,
}

pub struct PluginReg(pub &'static PluginEntry);
inventory::collect!(PluginReg);

#[macro_export]
macro_rules! register_plugin {
	($t:ty) => {
		pub static PLUGIN_ENTRY: $crate::db::plugin::PluginEntry =
			$crate::db::plugin::PluginEntry {
				id: <$t>::ID,
				register: <$t>::register,
			};
		inventory::submit! { $crate::db::plugin::PluginReg(&PLUGIN_ENTRY) }
	};
}

pub fn run_plugins(builder: &mut RegistryDbBuilder) -> Result<(), RegistryError> {
	for entry in inventory::iter::<PluginReg> {
		(entry.0.register)(builder)?;
	}
	Ok(())
}
