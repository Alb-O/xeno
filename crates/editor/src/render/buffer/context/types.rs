use xeno_registry::themes::Theme;
use xeno_runtime_language::LanguageLoader;
use xeno_tui::layout::Rect;
use xeno_tui::style::Style;
use xeno_tui::text::Line;

use super::super::cell_style::CursorStyleSet;
use super::super::diagnostics::{DiagnosticLineMap, DiagnosticRangeMap};
use super::super::gutter::GutterLayout;
use crate::buffer::Buffer;
use crate::render::cache::RenderCache;
use crate::window::GutterSelector;

/// Result of rendering a buffer's content.
pub struct RenderResult {
	/// Total width of the rendered gutter column.
	pub gutter_width: u16,
	/// Rendered gutter lines (length == viewport height).
	pub gutter: Vec<Line<'static>>,
	/// Rendered text content lines (length == viewport height).
	pub text: Vec<Line<'static>>,
}

/// Parameters for rendering a buffer.
pub struct RenderBufferParams<'a> {
	/// The buffer to render.
	pub buffer: &'a Buffer,
	/// The area to render into.
	pub area: Rect,
	/// Whether to use a block cursor.
	pub use_block_cursor: bool,
	/// Whether the buffer is focused.
	pub is_focused: bool,
	/// Gutter selection configuration.
	pub gutter: GutterSelector,
	/// Tab width override.
	pub tab_width: usize,
	/// Whether to highlight the cursor line.
	pub cursorline: bool,
	/// The render cache.
	pub cache: &'a mut RenderCache,
}

/// Derived layout constants for a render pass.
pub struct RenderLayout {
	pub total_lines: usize,
	pub gutter_layout: GutterLayout,
	pub text_width: usize,
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
