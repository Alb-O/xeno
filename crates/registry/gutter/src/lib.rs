//! Gutter column registry.
//!
//! Gutter columns are defined in static lists and rendered
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
use std::sync::LazyLock;

use ropey::RopeSlice;

mod impls;
mod macros;

pub use xeno_registry_core::{
	RegistryBuilder, RegistryEntry, RegistryIndex, RegistryMeta, RegistryMetadata, RegistryReg,
	RegistrySource, impl_registry_entry,
};

/// Registry wrapper for gutter definitions.
pub struct GutterReg(pub &'static GutterDef);
inventory::collect!(GutterReg);

impl RegistryReg<GutterDef> for GutterReg {
	fn def(&self) -> &'static GutterDef {
		self.0
	}
}

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
	/// Normal gutter foreground color.
	#[default]
	Normal,
	/// Dimmed (continuations, empty lines).
	Dim,
	/// Highlighted (cursor line).
	Cursor,
	/// Error diagnostic (theme error color).
	Error,
	/// Warning diagnostic (theme warning color).
	Warning,
	/// Info diagnostic (theme info color).
	Info,
	/// Hint diagnostic (dimmed).
	Hint,
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
/// Populated by the rendering layer with diagnostic info, custom signs, etc.
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

/// Indexed collection of all registered gutter columns.
pub static GUTTERS: LazyLock<RegistryIndex<GutterDef>> = LazyLock::new(|| {
	RegistryBuilder::new("gutters")
		.extend_inventory::<GutterReg>()
		.sort_by(|a, b| a.meta.priority.cmp(&b.meta.priority))
		.build()
});

/// Returns enabled gutters sorted by priority (left to right).
pub fn enabled_gutters() -> impl Iterator<Item = &'static GutterDef> {
	GUTTERS.iter().filter(|g| g.default_enabled)
}

/// Finds a gutter column by name.
pub fn find(name: &str) -> Option<&'static GutterDef> {
	GUTTERS.get(name)
}

/// Returns all registered gutter columns, sorted by priority.
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

impl_registry_entry!(GutterDef);
