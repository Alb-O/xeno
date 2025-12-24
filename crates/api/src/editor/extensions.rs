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

	/// Insert extension data. Panics in debug if already present.
	pub fn insert<T: Any + Send + Sync>(&mut self, val: T) {
		let type_id = TypeId::of::<T>();
		#[cfg(debug_assertions)]
		if self.inner.contains_key(&type_id) {
			panic!(
				"Extension state for type {} already registered",
				std::any::type_name::<T>()
			);
		}
		self.inner.insert(type_id, Box::new(val));
	}

	pub fn get<T: Any + Send + Sync>(&self) -> Option<&T> {
		self.inner.get(&TypeId::of::<T>())?.downcast_ref()
	}

	pub fn get_mut<T: Any + Send + Sync>(&mut self) -> Option<&mut T> {
		self.inner.get_mut(&TypeId::of::<T>())?.downcast_mut()
	}

	/// Get extension state or panic if not found.
	pub fn expect<T: Any + Send + Sync>(&self) -> &T {
		self.get::<T>().unwrap_or_else(|| {
			panic!(
				"Extension state for type {} missing",
				std::any::type_name::<T>()
			)
		})
	}

	/// Get extension state mutably or panic if not found.
	pub fn expect_mut<T: Any + Send + Sync>(&mut self) -> &mut T {
		self.get_mut::<T>().unwrap_or_else(|| {
			panic!(
				"Extension state for type {} missing",
				std::any::type_name::<T>()
			)
		})
	}

	/// Get extension state, initializing it if missing.
	pub fn get_or_init<T: Any + Send + Sync, F: FnOnce() -> T>(&mut self, f: F) -> &mut T {
		let type_id = TypeId::of::<T>();
		if !self.inner.contains_key(&type_id) {
			self.inner.insert(type_id, Box::new(f()));
		}
		self.inner
			.get_mut(&type_id)
			.unwrap()
			.downcast_mut()
			.unwrap()
	}
}

/// Definition for extension initialization.
pub struct ExtensionInitDef {
	/// Extension identifier (for debugging).
	pub id: &'static str,
	/// Priority (lower runs first).
	pub priority: i16,
	/// Initialization function.
	pub init: fn(&mut ExtensionMap),
}

/// Registry of all terminal-side extensions.
#[distributed_slice]
pub static EXTENSIONS: [ExtensionInitDef];

pub struct ExtensionTickDef {
	/// Priority (lower runs first).
	pub priority: i16,
	pub tick: fn(&mut crate::editor::Editor),
}

/// Extensions that need to run on every editor tick.
#[distributed_slice]
pub static TICK_EXTENSIONS: [ExtensionTickDef];
