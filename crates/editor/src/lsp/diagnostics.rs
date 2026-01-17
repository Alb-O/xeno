use xeno_lsp::lsp_position_to_char;
use xeno_primitives::Selection;
use xeno_registry_notifications::keys;

use crate::impls::Editor;

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
		let Some(buffer) = self.core.buffers.get_buffer(buffer_id) else {
			return;
		};
		let diagnostics = self.lsp.get_diagnostics(buffer);
		if diagnostics.is_empty() {
			self.notify(keys::info("No diagnostics"));
			return;
		}

		let encoding = self.lsp.offset_encoding_for_buffer(buffer);
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

		let Some(buffer) = self.core.buffers.get_buffer_mut(buffer_id) else {
			return;
		};
		buffer.set_cursor_and_selection(next_pos, Selection::point(next_pos));
		buffer.establish_goal_column();
		self.frame.needs_redraw = true;
	}
}
