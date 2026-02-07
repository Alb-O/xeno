use std::marker::PhantomData;

use super::def::{OptionDef, OptionKey};
use crate::core::FromOptionValue;

/// Typed handle to an option definition with compile-time type information.
pub struct TypedOptionKey<T: FromOptionValue> {
	pub(crate) def: &'static OptionDef,
	pub(crate) _marker: PhantomData<T>,
}

impl<T: FromOptionValue> Clone for TypedOptionKey<T> {
	fn clone(&self) -> Self {
		*self
	}
}

impl<T: FromOptionValue> Copy for TypedOptionKey<T> {}

impl<T: FromOptionValue> TypedOptionKey<T> {
	/// Creates a new typed option key from a static definition.
	pub const fn new(def: &'static OptionDef) -> Self {
		Self {
			def,
			_marker: PhantomData,
		}
	}

	/// Returns the underlying option definition.
	pub fn def(&self) -> &'static OptionDef {
		self.def
	}

	/// Returns the KDL key for this option.
	pub fn kdl_key(&self) -> &'static str {
		self.def.kdl_key
	}

	/// Returns the untyped option key for use with [`crate::actions::editor_ctx::OptionAccess::option_raw`].
	pub fn untyped(&self) -> OptionKey {
		self.def
	}
}
