//! Diff line type detection and styling.
//!
//! Determines line types in diff files for full-line background styling.

use xeno_registry::themes::Theme;
use xeno_tui::style::Color;

/// Type of line in a diff file, used for full-line background styling.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiffLineType {
	/// Added line (starts with `+` but not `+++`).
	Addition,
	/// Deleted line (starts with `-` but not `---`).
	Deletion,
	/// Hunk header (starts with `@@`).
	Hunk,
	/// Context or other line.
	Context,
}

impl DiffLineType {
	/// Detects the diff line type from line content.
	pub fn from_line(line: &str) -> Self {
		if line.starts_with("@@") {
			Self::Hunk
		} else if line.starts_with('+') && !line.starts_with("+++") {
			Self::Addition
		} else if line.starts_with('-') && !line.starts_with("---") {
			Self::Deletion
		} else {
			Self::Context
		}
	}

	/// Returns the background color for this diff line type.
	///
	/// Uses the theme's syntax colors for diff highlighting.
	pub fn bg_color(self, theme: &Theme) -> Option<Color> {
		match self {
			Self::Addition => theme.colors.syntax.diff_plus.bg,
			Self::Deletion => theme.colors.syntax.diff_minus.bg,
			Self::Hunk => theme.colors.syntax.diff_delta.bg,
			Self::Context => None,
		}
	}

	/// Returns whether this line type has a special background.
	#[allow(dead_code, reason = "utility method for callers")]
	pub fn has_background(self) -> bool {
		!matches!(self, Self::Context)
	}
}

/// Computes the diff line background for a line in a diff file.
///
/// Returns `None` if the file is not a diff file or the line is context.
pub fn diff_line_bg(is_diff_file: bool, line_text: &str, theme: &Theme) -> Option<Color> {
	if !is_diff_file {
		return None;
	}
	DiffLineType::from_line(line_text).bg_color(theme)
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn detect_addition() {
		assert_eq!(DiffLineType::from_line("+added line"), DiffLineType::Addition);
		assert_eq!(DiffLineType::from_line("+ "), DiffLineType::Addition);
	}

	#[test]
	fn detect_deletion() {
		assert_eq!(DiffLineType::from_line("-removed line"), DiffLineType::Deletion);
		assert_eq!(DiffLineType::from_line("- "), DiffLineType::Deletion);
	}

	#[test]
	fn detect_hunk() {
		assert_eq!(
			DiffLineType::from_line("@@ -1,3 +1,4 @@"),
			DiffLineType::Hunk
		);
	}

	#[test]
	fn detect_context() {
		assert_eq!(DiffLineType::from_line(" context line"), DiffLineType::Context);
		assert_eq!(DiffLineType::from_line("plain line"), DiffLineType::Context);
	}

	#[test]
	fn file_headers_are_context() {
		assert_eq!(DiffLineType::from_line("+++"), DiffLineType::Context);
		assert_eq!(DiffLineType::from_line("+++ a/file.rs"), DiffLineType::Context);
		assert_eq!(DiffLineType::from_line("---"), DiffLineType::Context);
		assert_eq!(DiffLineType::from_line("--- b/file.rs"), DiffLineType::Context);
	}
}
