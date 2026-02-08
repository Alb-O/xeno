//! Text object handler static registration via `inventory`.

use super::TextObjectHandler;

/// Handler configuration carrying inner/around selection functions.
#[derive(Clone, Copy)]
pub struct TextObjectHandlers {
	/// Inner selection handler.
	pub inner: TextObjectHandler,
	/// Around selection handler.
	pub around: TextObjectHandler,
}

pub type TextObjectHandlerStatic = crate::core::HandlerStatic<TextObjectHandlers>;

/// Wrapper for `inventory::collect!`.
pub struct TextObjectHandlerReg(pub &'static TextObjectHandlerStatic);

inventory::collect!(TextObjectHandlerReg);
