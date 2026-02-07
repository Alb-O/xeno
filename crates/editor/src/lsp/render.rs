//! LSP UI rendering (completion popup, diagnostic maps).

use super::system::LspSystem;
use crate::buffer::Buffer;
use crate::render::{DiagnosticLineMap, DiagnosticRangeMap};

#[cfg(feature = "lsp")]
impl LspSystem {
	#[allow(dead_code)]
	pub fn get_diagnostic_line_map(&self, buffer: &Buffer) -> DiagnosticLineMap {
		use crate::lsp::diagnostics::build_diagnostic_line_map;
		let diagnostics = self.get_diagnostics(buffer);
		build_diagnostic_line_map(&diagnostics)
	}

	#[allow(dead_code)]
	pub fn get_diagnostic_range_map(&self, buffer: &Buffer) -> DiagnosticRangeMap {
		use crate::lsp::diagnostics::build_diagnostic_range_map;
		let diagnostics = self.get_diagnostics(buffer);
		build_diagnostic_range_map(&diagnostics)
	}

	/// Renders the LSP completion popup if active.
	pub fn render_completion_popup(
		&self,
		editor: &crate::impls::Editor,
		frame: &mut xeno_tui::Frame,
	) {
		use xeno_tui::layout::Rect;

		use crate::completion::CompletionState;
		use crate::lsp::{LspMenuKind, LspMenuState};

		let completions = editor
			.overlays()
			.get::<CompletionState>()
			.cloned()
			.unwrap_or_default();
		if !completions.active || completions.items.is_empty() {
			return;
		}

		let Some(menu_state) = editor
			.overlays()
			.get::<LspMenuState>()
			.and_then(|s: &LspMenuState| s.active())
		else {
			return;
		};
		let buffer_id = match menu_state {
			LspMenuKind::Completion { buffer_id, .. } => *buffer_id,
			LspMenuKind::CodeAction { buffer_id, .. } => *buffer_id,
		};
		if buffer_id != editor.focused_view() {
			return;
		}

		let Some(buffer) = editor.get_buffer(buffer_id) else {
			return;
		};
		let tab_width = editor.tab_width_for(buffer_id);
		let Some((cursor_row, cursor_col)) =
			buffer.doc_to_screen_position(buffer.cursor, tab_width)
		else {
			return;
		};

		let max_label_len = completions
			.items
			.iter()
			.map(|it| it.label.len())
			.max()
			.unwrap_or(0);
		let width = (max_label_len + 10).max(12);
		let height = completions
			.items
			.len()
			.clamp(1, CompletionState::MAX_VISIBLE);

		let view_area = editor.focused_view_area();
		let mut x = view_area.x.saturating_add(cursor_col);
		let mut y = view_area.y.saturating_add(cursor_row.saturating_add(1));

		let width_u16 = width.min(view_area.width as usize) as u16;
		let height_u16 = height.min(view_area.height as usize) as u16;

		if x + width_u16 > view_area.right() {
			x = view_area.right().saturating_sub(width_u16);
		}
		if y + height_u16 > view_area.bottom() {
			let above = view_area
				.y
				.saturating_add(cursor_row)
				.saturating_sub(height_u16);
			y = above.max(view_area.y);
		}

		let area = Rect::new(x, y, width_u16, height_u16);
		frame.render_widget(editor.render_completion_menu(area), area);
	}
}

#[cfg(not(feature = "lsp"))]
impl LspSystem {
	#[allow(dead_code)]
	pub fn get_diagnostic_line_map(&self, _buffer: &Buffer) -> DiagnosticLineMap {
		DiagnosticLineMap::new()
	}

	#[allow(dead_code)]
	pub fn get_diagnostic_range_map(&self, _buffer: &Buffer) -> DiagnosticRangeMap {
		DiagnosticRangeMap::new()
	}

	/// Renders the LSP completion popup if active.
	pub fn render_completion_popup(
		&self,
		_editor: &crate::impls::Editor,
		_frame: &mut xeno_tui::Frame,
	) {
		// No-op when LSP is disabled
	}
}
