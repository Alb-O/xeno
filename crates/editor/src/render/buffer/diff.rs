//! Diff line type detection and styling.
//!
//! Determines line types in diff files for full-line background styling.
//! Also provides hunk header parsing and line number mapping for diff gutter.

use ropey::Rope;
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
}

/// Parsed hunk header from `@@ -old_start,count +new_start,count @@`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HunkHeader {
	/// Starting line number in the old file.
	pub old_start: u32,
	/// Number of lines in the old hunk.
	pub old_count: u32,
	/// Starting line number in the new file.
	pub new_start: u32,
	/// Number of lines in the new hunk.
	pub new_count: u32,
}

impl HunkHeader {
	/// Parses a hunk header line.
	///
	/// Handles formats:
	/// - `@@ -1,3 +1,4 @@` (standard)
	/// - `@@ -1 +1 @@` (count defaults to 1)
	/// - `@@ -0,0 +1,5 @@` (new file)
	pub fn parse(line: &str) -> Option<Self> {
		let line = line.trim();
		if !line.starts_with("@@") {
			return None;
		}

		let inner = line.strip_prefix("@@")?.split("@@").next()?.trim();
		let mut parts = inner.split_whitespace();

		let (old_start, old_count) = Self::parse_range(parts.next()?.strip_prefix('-')?)?;
		let (new_start, new_count) = Self::parse_range(parts.next()?.strip_prefix('+')?)?;

		Some(Self {
			old_start,
			old_count,
			new_start,
			new_count,
		})
	}

	fn parse_range(s: &str) -> Option<(u32, u32)> {
		match s.split_once(',') {
			Some((start, count)) => Some((start.parse().ok()?, count.parse().ok()?)),
			None => Some((s.parse().ok()?, 1)),
		}
	}
}

/// Line number mapping for a single diff line.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct DiffLineNumbers {
	/// Line number in original file (for `-` and context lines).
	pub old: Option<u32>,
	/// Line number in new file (for `+` and context lines).
	pub new: Option<u32>,
}

/// Computes diff line numbers for all lines in a diff buffer.
///
/// Returns a vector where each index corresponds to a display line,
/// containing the source file line numbers for that line.
pub fn compute_diff_line_numbers(text: &Rope) -> Vec<DiffLineNumbers> {
	let mut result = Vec::with_capacity(text.len_lines());
	let mut old_line: Option<u32> = None;
	let mut new_line: Option<u32> = None;

	for line_idx in 0..text.len_lines() {
		let line_str: String = text.line(line_idx).chars().take(256).collect();

		if let Some(header) = HunkHeader::parse(&line_str) {
			old_line = (header.old_count > 0).then_some(header.old_start);
			new_line = (header.new_count > 0).then_some(header.new_start);
			result.push(DiffLineNumbers::default());
			continue;
		}

		match DiffLineType::from_line(&line_str) {
			DiffLineType::Addition => {
				result.push(DiffLineNumbers {
					old: None,
					new: new_line,
				});
				new_line = new_line.map(|n| n + 1);
			}
			DiffLineType::Deletion => {
				result.push(DiffLineNumbers {
					old: old_line,
					new: None,
				});
				old_line = old_line.map(|n| n + 1);
			}
			DiffLineType::Context if old_line.is_some() || new_line.is_some() => {
				result.push(DiffLineNumbers {
					old: old_line,
					new: new_line,
				});
				old_line = old_line.map(|n| n + 1);
				new_line = new_line.map(|n| n + 1);
			}
			DiffLineType::Context | DiffLineType::Hunk => {
				result.push(DiffLineNumbers::default());
			}
		}
	}

	result
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
		assert_eq!(
			DiffLineType::from_line("+added line"),
			DiffLineType::Addition
		);
		assert_eq!(DiffLineType::from_line("+ "), DiffLineType::Addition);
	}

	#[test]
	fn detect_deletion() {
		assert_eq!(
			DiffLineType::from_line("-removed line"),
			DiffLineType::Deletion
		);
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
		assert_eq!(
			DiffLineType::from_line(" context line"),
			DiffLineType::Context
		);
		assert_eq!(DiffLineType::from_line("plain line"), DiffLineType::Context);
	}

	#[test]
	fn file_headers_are_context() {
		assert_eq!(DiffLineType::from_line("+++"), DiffLineType::Context);
		assert_eq!(
			DiffLineType::from_line("+++ a/file.rs"),
			DiffLineType::Context
		);
		assert_eq!(DiffLineType::from_line("---"), DiffLineType::Context);
		assert_eq!(
			DiffLineType::from_line("--- b/file.rs"),
			DiffLineType::Context
		);
	}

	#[test]
	fn parse_hunk_header_standard() {
		let header = HunkHeader::parse("@@ -1,3 +1,4 @@").unwrap();
		assert_eq!(header.old_start, 1);
		assert_eq!(header.old_count, 3);
		assert_eq!(header.new_start, 1);
		assert_eq!(header.new_count, 4);
	}

	#[test]
	fn parse_hunk_header_no_count() {
		let header = HunkHeader::parse("@@ -1 +1 @@").unwrap();
		assert_eq!(header.old_start, 1);
		assert_eq!(header.old_count, 1);
		assert_eq!(header.new_start, 1);
		assert_eq!(header.new_count, 1);
	}

	#[test]
	fn parse_hunk_header_new_file() {
		let header = HunkHeader::parse("@@ -0,0 +1,5 @@").unwrap();
		assert_eq!(header.old_start, 0);
		assert_eq!(header.old_count, 0);
		assert_eq!(header.new_start, 1);
		assert_eq!(header.new_count, 5);
	}

	#[test]
	fn parse_hunk_header_with_context() {
		// Some tools include function context after the @@ markers
		let header = HunkHeader::parse("@@ -10,7 +10,8 @@ fn main() {").unwrap();
		assert_eq!(header.old_start, 10);
		assert_eq!(header.old_count, 7);
		assert_eq!(header.new_start, 10);
		assert_eq!(header.new_count, 8);
	}

	#[test]
	fn parse_hunk_header_invalid() {
		assert!(HunkHeader::parse("not a hunk").is_none());
		assert!(HunkHeader::parse("--- a/file.rs").is_none());
		assert!(HunkHeader::parse("+++ b/file.rs").is_none());
	}

	#[test]
	fn compute_line_numbers_simple() {
		let diff = r#"diff --git a/file.rs b/file.rs
--- a/file.rs
+++ b/file.rs
@@ -1,3 +1,4 @@
 context
+added
 context
 context
"#;
		let rope = Rope::from_str(diff);
		let nums = compute_diff_line_numbers(&rope);

		// Lines 0-2: file headers (no line numbers)
		assert_eq!(nums[0], DiffLineNumbers::default());
		assert_eq!(nums[1], DiffLineNumbers::default());
		assert_eq!(nums[2], DiffLineNumbers::default());

		// Line 3: hunk header (no line numbers)
		assert_eq!(nums[3], DiffLineNumbers::default());

		// Line 4: context (old:1, new:1)
		assert_eq!(
			nums[4],
			DiffLineNumbers {
				old: Some(1),
				new: Some(1)
			}
		);

		// Line 5: addition (new:2 only)
		assert_eq!(
			nums[5],
			DiffLineNumbers {
				old: None,
				new: Some(2)
			}
		);

		// Line 6: context (old:2, new:3)
		assert_eq!(
			nums[6],
			DiffLineNumbers {
				old: Some(2),
				new: Some(3)
			}
		);

		// Line 7: context (old:3, new:4)
		assert_eq!(
			nums[7],
			DiffLineNumbers {
				old: Some(3),
				new: Some(4)
			}
		);
	}

	#[test]
	fn compute_line_numbers_deletion() {
		let diff = r#"@@ -1,3 +1,2 @@
 context
-deleted
 context
"#;
		let rope = Rope::from_str(diff);
		let nums = compute_diff_line_numbers(&rope);

		// Line 0: hunk header
		assert_eq!(nums[0], DiffLineNumbers::default());

		// Line 1: context (old:1, new:1)
		assert_eq!(
			nums[1],
			DiffLineNumbers {
				old: Some(1),
				new: Some(1)
			}
		);

		// Line 2: deletion (old:2 only)
		assert_eq!(
			nums[2],
			DiffLineNumbers {
				old: Some(2),
				new: None
			}
		);

		// Line 3: context (old:3, new:2)
		assert_eq!(
			nums[3],
			DiffLineNumbers {
				old: Some(3),
				new: Some(2)
			}
		);
	}
}
