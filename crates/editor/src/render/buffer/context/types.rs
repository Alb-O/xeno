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
///
/// Contains the layout and content lines for both gutter and text columns,
/// optimized for immediate rendering by the TUI backend.
pub struct RenderResult {
	/// Total width of the rendered gutter column.
	pub gutter_width: u16,
	/// Rendered gutter lines. Length matches viewport height.
	pub gutter: Vec<Line<'static>>,
	/// Rendered text content lines. Length matches viewport height.
	pub text: Vec<Line<'static>>,
}

/// Parameters for rendering a buffer.
///
/// This object encapsulates all configuration for a single render pass,
/// preventing positional argument errors ("bool soup") and allowing
/// for future extensions without breaking internal APIs.
pub struct RenderBufferParams<'a> {
	/// The buffer to render.
	pub buffer: &'a Buffer,
	/// The area to render into.
	pub area: Rect,
	/// Whether to use a block cursor (typically for Normal mode).
	pub use_block_cursor: bool,
	/// Whether the buffer should be rendered as focused (e.g. selection visibility).
	pub is_focused: bool,
	/// Gutter selection configuration.
	pub gutter: GutterSelector,
	/// Tab width override.
	pub tab_width: usize,
	/// Whether to highlight the line containing the primary cursor.
	pub cursorline: bool,
	/// The shared render cache for this pass.
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
	/// Syntax manager for accessing parsed trees.
	pub syntax_manager: &'a crate::syntax_manager::SyntaxManager,
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
