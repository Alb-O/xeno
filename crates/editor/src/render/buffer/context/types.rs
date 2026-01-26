use xeno_registry::themes::Theme;
use xeno_runtime_language::LanguageLoader;
use xeno_tui::style::Style;
use xeno_tui::widgets::Paragraph;

use super::super::cell_style::CursorStyleSet;
use super::super::diagnostics::{DiagnosticLineMap, DiagnosticRangeMap};
use crate::extensions::StyleOverlays;

/// Result of rendering a buffer's content.
pub struct RenderResult {
	/// The rendered paragraph widget ready for display.
	pub widget: Paragraph<'static>,
}

/// Context for rendering a buffer.
///
/// Contains all shared resources needed to render any buffer.
/// This allows the same rendering logic to be applied to any buffer
/// in the editor, enabling proper split view support.
pub struct BufferRenderContext<'a> {
	/// The current theme.
	pub theme: &'a Theme,
	/// Language loader for syntax highlighting.
	pub language_loader: &'a LanguageLoader,
	/// Style overlays (e.g., zen mode dimming).
	pub style_overlays: &'a StyleOverlays,
	/// Optional diagnostic line map for gutter signs.
	pub diagnostics: Option<&'a DiagnosticLineMap>,
	/// Optional diagnostic range map for underlines.
	pub diagnostic_ranges: Option<&'a DiagnosticRangeMap>,
}

/// Cursor styling configuration for rendering.
pub struct CursorStyles {
	/// Style for the primary (main) cursor.
	pub primary: Style,
	/// Style for secondary (additional) cursors in multi-cursor mode.
	pub secondary: Style,
	/// Base text style.
	pub base: Style,
	/// Selection highlight style.
	pub selection: Style,
	/// Style for cursors in unfocused buffers (dimmed like secondary cursors).
	pub unfocused: Style,
}

impl CursorStyles {
	/// Extracts the cursor style set for cell style resolution.
	pub fn to_cursor_set(&self) -> CursorStyleSet {
		CursorStyleSet {
			primary: self.primary,
			secondary: self.secondary,
			unfocused: self.unfocused,
		}
	}
}
