//! Hook handler static registration via `inventory`.

use super::types::{HookHandler, HookMutability, HookPriority};
use crate::HookEvent;

/// Handler configuration carried by the static registration.
///
/// Contains the executable logic and structural properties (event, mutability, priority)
/// that are code-dependent and cannot be expressed in NUON.
#[derive(Clone, Copy)]
pub struct HookHandlerConfig {
	/// Event this hook listens to.
	pub event: HookEvent,
	/// Mutability requirement.
	pub mutability: HookMutability,
	/// Execution priority.
	pub execution_priority: HookPriority,
	/// Handler function.
	pub handler: HookHandler,
}

pub type HookHandlerStatic = crate::core::HandlerStatic<HookHandlerConfig>;

/// Wrapper for `inventory::collect!`.
pub struct HookHandlerReg(pub &'static HookHandlerStatic);

inventory::collect!(HookHandlerReg);
