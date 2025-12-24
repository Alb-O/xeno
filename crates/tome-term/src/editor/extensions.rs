use std::any::{Any, TypeId};
use std::collections::HashMap;

use linkme::distributed_slice;

/// A type-safe map for storing extension state.
#[derive(Default)]
pub struct ExtensionMap {
	inner: HashMap<TypeId, Box<dyn Any + Send + Sync>>,
}

impl ExtensionMap {
	pub fn new() -> Self {
		Self::default()
	}

	pub fn insert<T: Any + Send + Sync>(&mut self, val: T) {
		self.inner.insert(TypeId::of::<T>(), Box::new(val));
	}

	pub fn get<T: Any + Send + Sync>(&self) -> Option<&T> {
		self.inner.get(&TypeId::of::<T>())?.downcast_ref()
	}

	pub fn get_mut<T: Any + Send + Sync>(&mut self) -> Option<&mut T> {
		self.inner.get_mut(&TypeId::of::<T>())?.downcast_mut()
	}
}

/// Definition for extension initialization.
pub struct ExtensionInitDef {
	pub init: fn(&mut ExtensionMap),
}

/// Registry of all terminal-side extensions.
#[distributed_slice]
pub static EXTENSIONS: [ExtensionInitDef];

pub struct ExtensionTickDef {
	pub tick: fn(&mut crate::editor::Editor),
}

/// Extensions that need to run on every editor tick.
#[distributed_slice]
pub static TICK_EXTENSIONS: [ExtensionTickDef];
