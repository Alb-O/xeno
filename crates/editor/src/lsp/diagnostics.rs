use xeno_lsp::lsp_position_to_char;
use xeno_lsp::lsp_types::{Diagnostic, DiagnosticSeverity};
use xeno_primitives::Selection;
use xeno_registry::notifications::keys;

use crate::impls::Editor;
use crate::render::{DiagnosticLineMap, DiagnosticRangeMap, DiagnosticSpan};

/// Builds a diagnostic line map from LSP diagnostics.
///
/// Converts LSP severity to gutter severity and keeps only the highest
/// severity per line.
pub fn build_diagnostic_line_map(diagnostics: &[Diagnostic]) -> DiagnosticLineMap {
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
///
/// # Boundary Logic
/// - Skips zero-length diagnostics (start == end).
/// - Excludes the final line if a multi-line diagnostic ends at character 0
///   of that line, preventing phantom underlines on empty lines.
pub fn build_diagnostic_range_map(diagnostics: &[Diagnostic]) -> DiagnosticRangeMap {
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
		let start_char = diag.range.start.character as usize;
		let end_char = diag.range.end.character as usize;

		if start_line == end_line && start_char == end_char {
			continue;
		}

		let effective_end_line = if end_line > start_line && end_char == 0 {
			end_line - 1
		} else {
			end_line
		};

		for line in start_line..=effective_end_line {
			let line_start_char = if line == start_line { start_char } else { 0 };
			let line_end_char = if line == end_line {
				end_char
			} else {
				usize::MAX
			};
			map.entry(line).or_default().push(DiagnosticSpan {
				start_char: line_start_char,
				end_char: line_end_char,
				severity,
			});
		}
	}

	map
}

enum NavDirection {
	Next,
	Prev,
}

impl Editor {
	pub fn goto_next_diagnostic(&mut self) {
		self.goto_diagnostic(NavDirection::Next);
	}

	pub fn goto_prev_diagnostic(&mut self) {
		self.goto_diagnostic(NavDirection::Prev);
	}

	fn goto_diagnostic(&mut self, direction: NavDirection) {
		let buffer_id = self.focused_view();
		let Some(buffer) = self.state.core.buffers.get_buffer(buffer_id) else {
			return;
		};
		let diagnostics = self.state.lsp.get_diagnostics(buffer);
		if diagnostics.is_empty() {
			self.notify(keys::info("No diagnostics"));
			return;
		}

		let encoding = self.state.lsp.offset_encoding_for_buffer(buffer);
		let mut positions: Vec<_> = buffer.with_doc(|doc| {
			diagnostics
				.iter()
				.filter_map(|diag| lsp_position_to_char(doc.content(), diag.range.start, encoding))
				.collect()
		});
		positions.sort_unstable();
		positions.dedup();

		if positions.is_empty() {
			self.notify(keys::info("No diagnostics"));
			return;
		}

		let cursor = buffer.cursor;
		let next_pos = match direction {
			NavDirection::Next => positions
				.iter()
				.find(|&&pos| pos > cursor)
				.copied()
				.unwrap_or_else(|| positions[0]),
			NavDirection::Prev => positions
				.iter()
				.rev()
				.find(|&&pos| pos < cursor)
				.copied()
				.unwrap_or_else(|| *positions.last().unwrap()),
		};

		let Some(buffer) = self.state.core.buffers.get_buffer_mut(buffer_id) else {
			return;
		};
		buffer.set_cursor_and_selection(next_pos, Selection::point(next_pos));
		buffer.establish_goal_column();
		self.state.frame.needs_redraw = true;
	}
}
