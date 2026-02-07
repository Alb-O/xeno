//! Gutter handler static registration via `inventory`.

use super::{GutterCell, GutterLineContext, GutterWidth};

/// Static handler registration entry collected via `inventory`.
pub struct GutterHandlerStatic {
	/// Handler name (must match the KDL gutter name exactly).
	pub name: &'static str,
	/// Crate that defined this handler.
	pub crate_name: &'static str,
	/// Width specification.
	pub width: GutterWidth,
	/// Render function.
	pub render: fn(&GutterLineContext) -> Option<GutterCell>,
}

/// Wrapper for `inventory::collect!`.
pub struct GutterHandlerReg(pub &'static GutterHandlerStatic);

inventory::collect!(GutterHandlerReg);
