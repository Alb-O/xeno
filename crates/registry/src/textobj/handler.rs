//! Text object handler static registration via `inventory`.

use super::TextObjectHandler;

/// Static handler registration entry collected via `inventory`.
pub struct TextObjectHandlerStatic {
	/// Handler name (must match the KDL text object name exactly).
	pub name: &'static str,
	/// Crate that defined this handler.
	pub crate_name: &'static str,
	/// Inner selection handler.
	pub inner: TextObjectHandler,
	/// Around selection handler.
	pub around: TextObjectHandler,
}

/// Wrapper for `inventory::collect!`.
pub struct TextObjectHandlerReg(pub &'static TextObjectHandlerStatic);

inventory::collect!(TextObjectHandlerReg);
