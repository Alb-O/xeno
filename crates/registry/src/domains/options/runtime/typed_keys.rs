use std::marker::PhantomData;

use crate::core::FromOptionValue;

/// Typed handle to an option definition with compile-time type information.
pub struct TypedOptionKey<T: FromOptionValue> {
	canonical_id: &'static str,
	_marker: PhantomData<T>,
}

impl<T: FromOptionValue> Clone for TypedOptionKey<T> {
	fn clone(&self) -> Self {
		*self
	}
}

impl<T: FromOptionValue> Copy for TypedOptionKey<T> {}

impl<T: FromOptionValue> TypedOptionKey<T> {
	/// Creates a new typed option key from a canonical ID string.
	pub const fn new(canonical_id: &'static str) -> Self {
		Self {
			canonical_id,
			_marker: PhantomData,
		}
	}

	/// Returns the canonical ID string for this option.
	pub fn canonical_id(&self) -> &'static str {
		self.canonical_id
	}

	/// Returns the untyped option key.
	pub fn untyped(&self) -> super::OptionKey {
		super::OptionKey::new(self.canonical_id)
	}
}
