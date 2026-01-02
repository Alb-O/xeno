//!
//! Bridges the registry's PanelDef type to manifest's RegistryMetadata trait.

use evildoer_registry::panels::PanelDef;

impl crate::RegistryMetadata for PanelDef {
	fn id(&self) -> &'static str {
		self.id
	}

	fn name(&self) -> &'static str {
		self.name
	}

	fn priority(&self) -> i16 {
		self.priority
	}

	fn source(&self) -> crate::RegistrySource {
		match self.source {
			evildoer_registry::RegistrySource::Builtin => crate::RegistrySource::Builtin,
			evildoer_registry::RegistrySource::Crate(name) => crate::RegistrySource::Crate(name),
			evildoer_registry::RegistrySource::Runtime => crate::RegistrySource::Runtime,
		}
	}
}
