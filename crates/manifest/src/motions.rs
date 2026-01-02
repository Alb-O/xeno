//! Motion primitive definitions.
//!
//! Re-exports from [`evildoer_registry::motions`] for backward compatibility.

pub use evildoer_registry::motions::{
	all, find, flags, movement, MotionDef, MotionHandler, MOTIONS,
};

impl crate::RegistryMetadata for MotionDef {
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
