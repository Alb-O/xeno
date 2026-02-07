//! Command handler static registration via `inventory`.
//!
//! Each `command_handler!` invocation creates a `CommandHandlerStatic` and
//! submits it via `inventory::submit!`. At startup, the linking step collects
//! all submitted handlers and pairs them with KDL metadata by name.

use super::def::CommandHandler;

/// Static handler registration entry collected via `inventory`.
pub struct CommandHandlerStatic {
	/// Handler name (must match the KDL command name exactly).
	pub name: &'static str,
	/// Crate that defined this handler.
	pub crate_name: &'static str,
	/// The async handler function pointer.
	pub handler: CommandHandler,
}

/// Wrapper for `inventory::collect!`.
pub struct CommandHandlerReg(pub &'static CommandHandlerStatic);

inventory::collect!(CommandHandlerReg);
