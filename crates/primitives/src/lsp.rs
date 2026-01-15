use std::path::PathBuf;

/// LSP position in line/character coordinates.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LspPosition {
	/// Zero-based line index.
	pub line: u32,
	/// Zero-based character offset in the line.
	pub character: u32,
}

impl LspPosition {
	/// Creates a new LSP position.
	pub const fn new(line: u32, character: u32) -> Self {
		Self { line, character }
	}
}

/// LSP range with start and end positions.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LspRange {
	/// Start position (inclusive).
	pub start: LspPosition,
	/// End position (exclusive).
	pub end: LspPosition,
}

impl LspRange {
	/// Creates a new LSP range.
	pub const fn new(start: LspPosition, end: LspPosition) -> Self {
		Self { start, end }
	}

	/// Creates a zero-length range at a position.
	pub const fn point(pos: LspPosition) -> Self {
		Self {
			start: pos,
			end: pos,
		}
	}
}

/// A pre-computed LSP document change ready for sync.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LspDocumentChange {
	/// The range in the document that was replaced (pre-change positions).
	pub range: LspRange,
	/// The text that replaced the range.
	pub new_text: String,
}

/// Accumulated changes for a single document version bump.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LspChangeSet {
	/// Path to the document.
	pub path: PathBuf,
	/// Language ID.
	pub language: String,
	/// Individual changes for this update.
	pub changes: Vec<LspDocumentChange>,
}
