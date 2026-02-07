//! Statusline segment handler static registration via `inventory`.

use super::{RenderedSegment, StatuslineContext};

/// Static handler registration entry collected via `inventory`.
pub struct StatuslineHandlerStatic {
	/// Handler name (must match the KDL segment name exactly).
	pub name: &'static str,
	/// Crate that defined this handler.
	pub crate_name: &'static str,
	/// Render function.
	pub render: fn(&StatuslineContext) -> Option<RenderedSegment>,
}

/// Wrapper for `inventory::collect!`.
pub struct StatuslineHandlerReg(pub &'static StatuslineHandlerStatic);

inventory::collect!(StatuslineHandlerReg);
