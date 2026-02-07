//! Hook handler static registration via `inventory`.

use super::types::{HookHandler, HookMutability, HookPriority};
use crate::HookEvent;

/// Static handler registration entry collected via `inventory`.
pub struct HookHandlerStatic {
	/// Handler name (must match the KDL hook name exactly).
	pub name: &'static str,
	/// Crate that defined this handler.
	pub crate_name: &'static str,
	/// Event this hook listens to.
	pub event: HookEvent,
	/// Mutability requirement.
	pub mutability: HookMutability,
	/// Execution priority.
	pub execution_priority: HookPriority,
	/// Handler function.
	pub handler: HookHandler,
}

/// Wrapper for `inventory::collect!`.
pub struct HookHandlerReg(pub &'static HookHandlerStatic);

inventory::collect!(HookHandlerReg);
