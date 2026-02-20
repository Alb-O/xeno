//! Test helpers for downstream crates that need to construct registry types.
//!
//! Gated behind the `test-support` feature so production builds never expose
//! internal constructors.

#[cfg(feature = "keymap")]
use std::sync::Arc;

#[cfg(feature = "keymap")]
use crate::{ActionId, CompiledBinding, CompiledBindingTarget};

/// Builds a [`CompiledBinding`] targeting an action with the given name and
/// count. Useful for unit-testing input handler logic without a full keymap.
#[cfg(feature = "keymap")]
pub fn action_binding(name: &str, count: usize, extend: bool, register: Option<char>) -> CompiledBinding {
	CompiledBinding::new(
		CompiledBindingTarget::Action {
			id: ActionId(0),
			count,
			extend,
			register,
		},
		Arc::from(name),
		Arc::from(""),
		Arc::from(""),
		Vec::new(),
	)
}
