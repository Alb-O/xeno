//! Gutter column registry.
//!
//! Gutter columns are defined in static lists and rendered
//! left-to-right based on priority.

use std::path::Path;

use ropey::RopeSlice;

pub use crate::themes::Color;
pub use crate::themes::theme::ThemeDef as Theme;

pub(crate) mod builtins;
mod macros;

pub use xeno_registry_core::{
	RegistryBuilder, RegistryEntry, RegistryIndex, RegistryMeta, RegistryMetadata, RegistrySource,
	impl_registry_entry,
};

// Re-export macros
pub use crate::gutter;

/// Context passed to each gutter render closure (per-line).
pub struct GutterLineContext<'a> {
	/// 0-indexed line number in document.
	pub line_idx: usize,
	/// Total lines in the document.
	pub total_lines: usize,
	/// Current cursor line (0-indexed) - enables relative line numbers.
	pub cursor_line: usize,
	/// Whether this line is the cursor line.
	pub is_cursor_line: bool,
	/// Whether this is a wrapped continuation (not first segment of line).
	pub is_continuation: bool,
	/// Line text (rope slice for efficient access).
	pub line_text: RopeSlice<'a>,
	/// File path if available.
	pub path: Option<&'a Path>,
	/// Per-line annotation data (diagnostics, git, etc.).
	pub annotations: &'a GutterAnnotations,
	/// Theme for color lookups.
	pub theme: &'a Theme,
}

/// Context for width calculation (per-document, not per-line).
#[derive(Debug, Clone, Copy)]
pub struct GutterWidthContext {
	/// Total lines in document.
	pub total_lines: usize,
	/// Maximum viewport width (for constraints).
	pub viewport_width: u16,
}

/// A styled segment within a gutter cell.
#[derive(Debug, Clone)]
pub struct GutterSegment {
	/// Text content.
	pub text: String,
	/// Foreground color (None = default gutter_fg from theme).
	pub fg: Option<Color>,
	/// Whether to dim the text (blend fg toward bg).
	pub dim: bool,
}

/// What a gutter column renders for a single line.
#[derive(Debug, Clone)]
pub struct GutterCell {
	/// Styled segments (concatenated, then right-aligned within column width).
	pub segments: Vec<GutterSegment>,
}

impl GutterCell {
	/// Creates a cell with a single uniformly-styled segment.
	pub fn new(text: impl Into<String>, fg: Option<Color>, dim: bool) -> Self {
		Self {
			segments: vec![GutterSegment {
				text: text.into(),
				fg,
				dim,
			}],
		}
	}

	/// Creates a cell from multiple styled segments.
	pub fn styled(segments: Vec<GutterSegment>) -> Self {
		Self { segments }
	}
}

/// Width calculation strategy.
#[derive(Debug, Clone, Copy)]
pub enum GutterWidth {
	/// Fixed width in characters.
	Fixed(u16),
	/// Dynamic width computed from document state.
	Dynamic(fn(&GutterWidthContext) -> u16),
}

/// Per-line annotation data for gutter columns.
#[derive(Debug, Clone, Default)]
pub struct GutterAnnotations {
	/// Diagnostic severity (0=none, 1=hint, 2=info, 3=warn, 4=error).
	pub diagnostic_severity: u8,
	/// Custom sign character (breakpoint, bookmark, etc.).
	pub sign: Option<char>,
	/// Line number in old file (for diff `-` and context lines).
	pub diff_old_line: Option<u32>,
	/// Line number in new file (for diff `+` and context lines).
	pub diff_new_line: Option<u32>,
}

/// Definition of a gutter column.
pub struct GutterDef {
	/// Common registry metadata.
	pub meta: RegistryMeta,
	/// Whether enabled by default.
	pub default_enabled: bool,
	/// Width strategy.
	pub width: GutterWidth,
	/// Render function - called per visible line.
	pub render: fn(&GutterLineContext) -> Option<GutterCell>,
}

impl core::fmt::Debug for GutterDef {
	fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
		f.debug_struct("GutterDef")
			.field("name", &self.meta.name)
			.field("priority", &self.meta.priority)
			.field("default_enabled", &self.default_enabled)
			.finish()
	}
}

#[cfg(feature = "db")]
pub use crate::db::GUTTERS;

/// Returns enabled gutters sorted by priority (left to right).
#[cfg(feature = "db")]
pub fn enabled_gutters() -> impl Iterator<Item = &'static GutterDef> {
	GUTTERS.iter().filter(|g| g.default_enabled)
}

/// Finds a gutter column by name.
#[cfg(feature = "db")]
pub fn find(name: &str) -> Option<&'static GutterDef> {
	GUTTERS.get(name)
}

/// Returns all registered gutter columns, sorted by priority.
#[cfg(feature = "db")]
pub fn all() -> impl Iterator<Item = &'static GutterDef> {
	GUTTERS.iter()
}

/// Computes the width of a single gutter column.
pub fn column_width(gutter: &GutterDef, ctx: &GutterWidthContext) -> u16 {
	match gutter.width {
		GutterWidth::Fixed(w) => w,
		GutterWidth::Dynamic(f) => f(ctx),
	}
}

/// Computes total gutter width from enabled columns (includes trailing separator).
#[cfg(feature = "db")]
pub fn total_width(ctx: &GutterWidthContext) -> u16 {
	let width: u16 = enabled_gutters().map(|g| column_width(g, ctx)).sum();
	if width > 0 { width + 1 } else { 0 }
}

/// Computes widths for all enabled columns, returning (width, def) pairs sorted by priority.
#[cfg(feature = "db")]
pub fn column_widths(ctx: &GutterWidthContext) -> Vec<(u16, &'static GutterDef)> {
	enabled_gutters()
		.map(|g| (column_width(g, ctx), g))
		.collect()
}

impl_registry_entry!(GutterDef);
