//! Motion handler static registration via `inventory`.
//!
//! Each `motion_handler!` invocation creates a `MotionHandlerStatic` and
//! submits it via `inventory::submit!`. At startup, the linking step collects
//! all submitted handlers and pairs them with KDL metadata by name.

use super::MotionHandler;

pub type MotionHandlerStatic = crate::core::HandlerStatic<MotionHandler>;

/// Static handler registration entry collected via `inventory`.
/// Wrapper for `inventory::collect!`.
pub struct MotionHandlerReg(pub &'static MotionHandlerStatic);

inventory::collect!(MotionHandlerReg);
