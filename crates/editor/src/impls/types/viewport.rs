//! Terminal viewport dimensions.

use xeno_tui::layout::Rect;

/// Terminal window dimensions and computed document area.
///
/// Groups viewport-related fields that change together on resize events.
#[derive(Default, Clone, Copy)]
pub struct Viewport {
	/// Window width in columns.
	pub width: Option<u16>,
	/// Window height in rows.
	pub height: Option<u16>,
	/// Last computed document area (excludes chrome like menu/status bars).
	pub doc_area: Option<Rect>,
}
