//!
//! Bridges the registry's HookDef type to manifest's RegistryMetadata trait.

use evildoer_registry::hooks::HookDef;

impl crate::RegistryMetadata for HookDef {
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
