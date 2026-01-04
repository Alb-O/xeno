//! Gutter column registry.
//!
//! Gutter columns are registered at compile-time using [`linkme`] and rendered
//! left-to-right based on priority. Each column provides a closure that computes
//! the display text for each line, enabling arbitrary formatting (absolute,
//! relative, hexadecimal, custom symbols, etc.).
//!
//! # Built-in Columns
//!
//! - `line_numbers` - Absolute line numbers (enabled by default)
//! - `relative_line_numbers` - Distance from cursor line (disabled by default)
//! - `hybrid_line_numbers` - Absolute on cursor, relative elsewhere (disabled)
//! - `signs` - Sign column for diagnostics/breakpoints (enabled by default)

use std::path::Path;

use linkme::distributed_slice;
use ropey::RopeSlice;

mod impls;
mod macros;

pub use xeno_registry_core::{RegistryMetadata, RegistrySource, impl_registry_metadata};

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
}

/// Context for width calculation (per-document, not per-line).
#[derive(Debug, Clone, Copy)]
pub struct GutterWidthContext {
	/// Total lines in document.
	pub total_lines: usize,
	/// Maximum viewport width (for constraints).
	pub viewport_width: u16,
}

/// What a gutter column renders for a single line.
#[derive(Debug, Clone)]
pub struct GutterCell {
	/// Text content (right-aligned within column width by renderer).
	pub text: String,
	/// Style hint.
	pub style: GutterStyle,
}

/// Style hints for gutter cells.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum GutterStyle {
	/// Normal line number color.
	#[default]
	Normal,
	/// Dimmed (continuations, empty lines).
	Dim,
	/// Highlighted (cursor line).
	Cursor,
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
///
/// Extended by LSP/git extensions via optional fields. Start with a minimal
/// set and add fields as needed for diagnostics, git hunks, etc.
#[derive(Debug, Clone, Default)]
pub struct GutterAnnotations {
	/// Diagnostic severity (0=none, 1=hint, 2=info, 3=warn, 4=error).
	pub diagnostic_severity: u8,
	/// Git hunk type.
	pub git_status: Option<GitHunkStatus>,
	/// Custom sign character (breakpoint, bookmark, etc.).
	pub sign: Option<char>,
}

/// Git hunk status for gutter display.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GitHunkStatus {
	/// Line was added.
	Added,
	/// Line was modified.
	Modified,
	/// Line was removed (shown on adjacent line).
	Removed,
}

/// Definition of a gutter column.
pub struct GutterDef {
	/// Unique identifier (e.g., "xeno_registry_gutter::line_numbers").
	pub id: &'static str,
	/// Column name (e.g., "line_numbers").
	pub name: &'static str,
	/// Short description.
	pub description: &'static str,
	/// Priority: lower = renders further left.
	pub priority: i16,
	/// Whether enabled by default.
	pub default_enabled: bool,
	/// Width strategy.
	pub width: GutterWidth,
	/// Render function - called per visible line.
	pub render: fn(&GutterLineContext) -> Option<GutterCell>,
	/// Origin of the column.
	pub source: RegistrySource,
}

impl core::fmt::Debug for GutterDef {
	fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
		f.debug_struct("GutterDef")
			.field("name", &self.name)
			.field("priority", &self.priority)
			.field("default_enabled", &self.default_enabled)
			.finish()
	}
}

/// Registry of all gutter column definitions.
#[distributed_slice]
pub static GUTTERS: [GutterDef];

/// Returns enabled gutters sorted by priority (left to right).
pub fn enabled_gutters() -> impl Iterator<Item = &'static GutterDef> {
	let mut gutters: Vec<_> = GUTTERS.iter().filter(|g| g.default_enabled).collect();
	gutters.sort_by_key(|g| g.priority);
	gutters.into_iter()
}

/// Finds a gutter column by name.
pub fn find(name: &str) -> Option<&'static GutterDef> {
	GUTTERS.iter().find(|g| g.name == name)
}

/// Returns all registered gutter columns.
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

/// Computes total gutter width from enabled columns.
pub fn total_width(ctx: &GutterWidthContext) -> u16 {
	let columns_width: u16 = enabled_gutters().map(|g| column_width(g, ctx)).sum();
	if columns_width > 0 {
		columns_width + 1 // trailing separator space
	} else {
		0
	}
}

/// Computes widths for all enabled columns, returning (width, def) pairs sorted by priority.
pub fn column_widths(ctx: &GutterWidthContext) -> Vec<(u16, &'static GutterDef)> {
	enabled_gutters()
		.map(|g| (column_width(g, ctx), g))
		.collect()
}

impl_registry_metadata!(GutterDef);
