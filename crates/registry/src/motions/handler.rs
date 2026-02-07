//! Motion handler static registration via `inventory`.
//!
//! Each `motion_handler!` invocation creates a `MotionHandlerStatic` and
//! submits it via `inventory::submit!`. At startup, the linking step collects
//! all submitted handlers and pairs them with KDL metadata by name.

use super::MotionHandler;

/// Static handler registration entry collected via `inventory`.
pub struct MotionHandlerStatic {
	/// Handler name (must match the KDL motion name exactly).
	pub name: &'static str,
	/// Crate that defined this handler.
	pub crate_name: &'static str,
	/// The motion handler function pointer.
	pub handler: MotionHandler,
}

/// Wrapper for `inventory::collect!`.
pub struct MotionHandlerReg(pub &'static MotionHandlerStatic);

inventory::collect!(MotionHandlerReg);
