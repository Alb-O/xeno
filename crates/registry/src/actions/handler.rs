//! Action handler static registration via `inventory`.
//!
//! Each `action_handler!` invocation creates an `ActionHandlerStatic` and
//! submits it via `inventory::submit!`. At startup, the linking step collects
//! all submitted handlers and pairs them with KDL metadata by name.

use super::def::ActionHandler;

/// Static handler registration entry collected via `inventory`.
pub struct ActionHandlerStatic {
	/// Handler name (must match the KDL action name exactly).
	pub name: &'static str,
	/// Crate that defined this handler.
	pub crate_name: &'static str,
	/// The handler function pointer.
	pub handler: ActionHandler,
}

/// Wrapper for `inventory::collect!`.
pub struct ActionHandlerReg(pub &'static ActionHandlerStatic);

inventory::collect!(ActionHandlerReg);
