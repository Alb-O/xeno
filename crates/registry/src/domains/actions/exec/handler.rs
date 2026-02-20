//! Action handler static registration via `inventory`.
//!
//! Each `action_handler!` invocation creates an `ActionHandlerStatic` and
//! submits it via `inventory::submit!`. At startup, the linking step collects
//! all submitted handlers and pairs them with NUON metadata by name.

use super::def::ActionHandler;

pub type ActionHandlerStatic = crate::core::HandlerStatic<ActionHandler>;

/// Static handler registration entry collected via `inventory`.
/// Wrapper for `inventory::collect!`.
pub struct ActionHandlerReg(pub &'static ActionHandlerStatic);

inventory::collect!(ActionHandlerReg);
