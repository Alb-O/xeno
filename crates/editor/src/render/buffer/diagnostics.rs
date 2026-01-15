//! Diagnostic types and builders for buffer rendering.
//!
//! This module provides types for mapping LSP diagnostics to line-based
//! structures suitable for gutter signs and inline underlines.

use std::collections::HashMap;

/// Map from line number (0-indexed) to diagnostic severity (gutter format).
///
/// Severity values match `GutterAnnotations::diagnostic_severity`:
/// - 4 = Error
/// - 3 = Warning
/// - 2 = Information
/// - 1 = Hint
/// - 0 = None
pub type DiagnosticLineMap = HashMap<usize, u8>;

/// A diagnostic span covering a character range within a single line.
#[derive(Debug, Clone, Copy)]
pub struct DiagnosticSpan {
	/// Start character (column) on this line (0-indexed).
	pub start_char: usize,
	/// End character (column) on this line (exclusive, 0-indexed).
	pub end_char: usize,
	/// Severity level (same as gutter format: 4=Error, 3=Warning, 2=Info, 1=Hint).
	pub severity: u8,
}

/// Map from line number to diagnostic spans on that line.
///
/// Used for rendering underlines under diagnostic ranges.
pub type DiagnosticRangeMap = HashMap<usize, Vec<DiagnosticSpan>>;

/// Builds a diagnostic line map from LSP diagnostics.
///
/// Converts LSP severity to gutter severity and keeps only the highest
/// severity per line.
#[cfg(feature = "lsp")]
pub fn build_diagnostic_line_map(
	diagnostics: &[xeno_lsp::lsp_types::Diagnostic],
) -> DiagnosticLineMap {
	use xeno_lsp::lsp_types::DiagnosticSeverity;

	let mut map = DiagnosticLineMap::new();

	for diag in diagnostics {
		let line = diag.range.start.line as usize;
		// LSP: 1=Error, 2=Warning, 3=Info, 4=Hint → Gutter: 4, 3, 2, 1
		let severity = match diag.severity {
			Some(DiagnosticSeverity::ERROR) => 4,
			Some(DiagnosticSeverity::WARNING) => 3,
			Some(DiagnosticSeverity::INFORMATION) => 2,
			Some(DiagnosticSeverity::HINT) => 1,
			_ => 0,
		};
		map.entry(line)
			.and_modify(|e| *e = (*e).max(severity))
			.or_insert(severity);
	}

	map
}

/// Builds a diagnostic range map from LSP diagnostics.
///
/// Creates per-line spans with character ranges for rendering underlines.
#[cfg(feature = "lsp")]
pub fn build_diagnostic_range_map(
	diagnostics: &[xeno_lsp::lsp_types::Diagnostic],
) -> DiagnosticRangeMap {
	use xeno_lsp::lsp_types::DiagnosticSeverity;

	let mut map = DiagnosticRangeMap::new();

	for diag in diagnostics {
		let severity = match diag.severity {
			Some(DiagnosticSeverity::ERROR) => 4,
			Some(DiagnosticSeverity::WARNING) => 3,
			Some(DiagnosticSeverity::INFORMATION) => 2,
			Some(DiagnosticSeverity::HINT) => 1,
			_ => 0,
		};

		if severity == 0 {
			continue;
		}

		let start_line = diag.range.start.line as usize;
		let end_line = diag.range.end.line as usize;

		for line in start_line..=end_line {
			let start_char = if line == start_line {
				diag.range.start.character as usize
			} else {
				0
			};
			let end_char = if line == end_line {
				diag.range.end.character as usize
			} else {
				usize::MAX
			};
			map.entry(line).or_default().push(DiagnosticSpan {
				start_char,
				end_char,
				severity,
			});
		}
	}

	map
}
