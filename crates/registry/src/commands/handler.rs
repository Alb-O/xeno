//! Command handler static registration via `inventory`.
//!
//! Each `command_handler!` invocation creates a `CommandHandlerStatic` and
//! submits it via `inventory::submit!`. At startup, the linking step collects
//! all submitted handlers and pairs them with KDL metadata by name.

use super::def::CommandHandler;

pub type CommandHandlerStatic = crate::core::HandlerStatic<CommandHandler>;

/// Static handler registration entry collected via `inventory`.
/// Wrapper for `inventory::collect!`.
pub struct CommandHandlerReg(pub &'static CommandHandlerStatic);

inventory::collect!(CommandHandlerReg);
