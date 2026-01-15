//! Type-erased storage for UI overlay state.
//!
//! [`OverlayManager`] stores arbitrary UI overlay state without requiring
//! fields on [`Editor`]. New overlay types can be added without modifying
//! the Editor definition.
//!
//! [`Editor`]: crate::editor::Editor

use std::any::{Any, TypeId};
use std::collections::HashMap;

/// Type-erased storage for UI overlay state.
///
/// Similar to [`ExtensionMap`] but for transient UI overlays like popups,
/// palettes, and completion menus.
///
/// [`ExtensionMap`]: crate::editor::extensions::ExtensionMap
#[derive(Default)]
pub struct OverlayManager {
	inner: HashMap<TypeId, Box<dyn Any + Send + Sync>>,
}

impl OverlayManager {
	/// Creates a new empty overlay manager.
	pub fn new() -> Self {
		Self::default()
	}

	/// Returns a reference to overlay state of type `T`, if present.
	pub fn get<T: Any + Send + Sync>(&self) -> Option<&T> {
		self.inner.get(&TypeId::of::<T>())?.downcast_ref()
	}

	/// Returns a mutable reference to overlay state, inserting the default if absent.
	pub fn get_or_default<T: Any + Send + Sync + Default>(&mut self) -> &mut T {
		let type_id = TypeId::of::<T>();
		self.inner
			.entry(type_id)
			.or_insert_with(|| Box::<T>::default());
		self.inner
			.get_mut(&type_id)
			.unwrap()
			.downcast_mut()
			.unwrap()
	}

	/// Inserts overlay state, replacing any existing state of the same type.
	pub fn insert<T: Any + Send + Sync>(&mut self, val: T) {
		self.inner.insert(TypeId::of::<T>(), Box::new(val));
	}
}
