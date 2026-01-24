//! Diagnostic types for buffer rendering.
//!
//! This module provides types for mapping diagnostics to line-based
//! structures suitable for gutter signs and inline underlines.
//!
//! Builder functions that convert from LSP diagnostics live in the
//! `lsp::diagnostics` module to keep LSP dependencies out of the render path.

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
