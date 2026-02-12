use crate::completion::CompletionState;
use crate::geometry::Rect;
use crate::Editor;
use unicode_width::UnicodeWidthStr;

impl Editor {
	/// Returns whether the LSP completion popup should be rendered.
	pub fn completion_popup_visible(&self) -> bool {
		#[cfg(not(feature = "lsp"))]
		{
			false
		}

		#[cfg(feature = "lsp")]
		{
			let completions = self.overlays().get::<crate::CompletionState>().cloned().unwrap_or_default();
			if !completions.active || completions.items.is_empty() {
				return false;
			}

			let Some(menu_state) = self.overlays().get::<crate::lsp::LspMenuState>().and_then(|state| state.active()) else {
				return false;
			};

			let buffer_id = match menu_state {
				crate::lsp::LspMenuKind::Completion { buffer_id, .. } => *buffer_id,
				crate::lsp::LspMenuKind::CodeAction { buffer_id, .. } => *buffer_id,
			};

			buffer_id == self.focused_view()
		}
	}

	/// Returns the computed completion popup area in document coordinates.
	///
	/// The area is clamped to the focused view and follows cursor-relative
	/// placement policy used by frontend popup renderers.
	pub fn completion_popup_area(&self) -> Option<Rect> {
		if !self.completion_popup_visible() {
			return None;
		}

		let completions = self.overlays().get::<CompletionState>().cloned().unwrap_or_default();
		if !completions.active || completions.items.is_empty() {
			return None;
		}

		let buffer_id = self.focused_view();
		let buffer = self.get_buffer(buffer_id)?;
		let tab_width = self.tab_width_for(buffer_id);
		let (cursor_row, cursor_col) = buffer.doc_to_screen_position(buffer.cursor, tab_width)?;

		let view_area = self.view_area(buffer_id);
		if view_area.width < 12 || view_area.height < 3 {
			return None;
		}

		let show_kind = view_area.width >= 24;
		let max_label_width = completions.items.iter().map(|it| it.label.width()).max().unwrap_or(0);
		let border_cols = 1;
		let icon_cols = 4;
		let kind_cols = if show_kind { 7 } else { 0 };
		let width = (border_cols + icon_cols + max_label_width + kind_cols).max(12);
		let height = completions.items.len().clamp(1, CompletionState::MAX_VISIBLE);

		let mut x = view_area.x.saturating_add(cursor_col);
		let mut y = view_area.y.saturating_add(cursor_row.saturating_add(1));

		let width_u16 = width.min(view_area.width.saturating_sub(1) as usize) as u16;
		let height_u16 = height.min(view_area.height.saturating_sub(1) as usize) as u16;
		if width_u16 == 0 || height_u16 == 0 {
			return None;
		}

		if x + width_u16 > view_area.right() {
			x = view_area.right().saturating_sub(width_u16);
		}
		if y + height_u16 > view_area.bottom() {
			let above = view_area.y.saturating_add(cursor_row).saturating_sub(height_u16);
			y = above.max(view_area.y);
		}

		Some(Rect::new(x, y, width_u16, height_u16))
	}
}
