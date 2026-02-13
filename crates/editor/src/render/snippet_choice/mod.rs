use unicode_width::UnicodeWidthStr;

use crate::Editor;
use crate::geometry::Rect;
use crate::snippet::{SnippetChoiceOverlay, SnippetChoiceRenderItem, SnippetChoiceRenderPlan};

fn choice_window(total: usize, selected: usize, visible_rows: usize) -> (usize, usize) {
	if total == 0 {
		return (0, 0);
	}
	let rows = visible_rows.max(1).min(total);
	if total <= rows {
		return (0, total);
	}

	let clamped_selected = selected.min(total.saturating_sub(1));
	let half = rows / 2;
	let max_start = total.saturating_sub(rows);
	let start = clamped_selected.saturating_sub(half).min(max_start);
	(start, start + rows)
}

impl Editor {
	/// Returns whether the snippet choice popup should be rendered.
	pub fn snippet_choice_popup_visible(&self) -> bool {
		if self.overlay_kind().is_some() {
			return false;
		}

		self.overlays()
			.get::<SnippetChoiceOverlay>()
			.is_some_and(|overlay| overlay.active && overlay.buffer_id == self.focused_view() && !overlay.options.is_empty())
	}

	/// Returns the computed snippet-choice popup area in document coordinates.
	///
	/// The area is clamped to the focused view and follows cursor-relative
	/// placement policy used by frontend popup renderers.
	pub fn snippet_choice_popup_area(&self) -> Option<Rect> {
		if !self.snippet_choice_popup_visible() {
			return None;
		}

		let overlay = self.overlays().get::<SnippetChoiceOverlay>().cloned().unwrap_or_default();
		if !overlay.active || overlay.buffer_id != self.focused_view() || overlay.options.is_empty() {
			return None;
		}

		let buffer = self.get_buffer(overlay.buffer_id)?;
		let tab_width = self.tab_width_for(overlay.buffer_id);
		let (cursor_row, cursor_col) = buffer.doc_to_screen_position(buffer.cursor, tab_width)?;

		let view_area = self.view_area(overlay.buffer_id);
		if view_area.width < 12 || view_area.height < 3 {
			return None;
		}

		let max_option_width = overlay.options.iter().map(|option| option.width()).max().unwrap_or(1);
		let width = (max_option_width + 3).max(12);
		let height = overlay.options.len().clamp(1, 10);

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

	/// Builds a data-only snippet choice render plan for frontend layers.
	pub fn snippet_choice_render_plan(&self) -> Option<SnippetChoiceRenderPlan> {
		let area = self.snippet_choice_popup_area()?;

		let overlay = self.overlays().get::<SnippetChoiceOverlay>().cloned().unwrap_or_default();
		if !overlay.active || overlay.buffer_id != self.focused_view() || overlay.options.is_empty() {
			return None;
		}

		let target_row_width = area.width.saturating_sub(1) as usize;
		let (window_start, window_end) = choice_window(overlay.options.len(), overlay.selected, area.height as usize);
		let selected = overlay.selected.min(overlay.options.len().saturating_sub(1));
		let max_option_width = overlay.options.iter().map(|option| option.width()).max().unwrap_or(1).min(target_row_width);

		let items = overlay.options[window_start..window_end]
			.iter()
			.enumerate()
			.map(|(idx, option)| SnippetChoiceRenderItem {
				option: option.clone(),
				selected: window_start + idx == selected,
			})
			.collect();

		Some(SnippetChoiceRenderPlan {
			items,
			max_option_width,
			target_row_width,
		})
	}
}

#[cfg(test)]
mod tests;
