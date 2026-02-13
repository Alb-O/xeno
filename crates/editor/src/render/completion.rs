use unicode_width::UnicodeWidthStr;

use crate::Editor;
use crate::completion::{CompletionKind, CompletionRenderItem, CompletionRenderPlan, CompletionState};
use crate::geometry::Rect;
use crate::overlay::{OverlayControllerKind, WindowRole};

#[derive(Debug, Clone)]
pub struct OverlayCompletionMenuTarget {
	pub rect: Rect,
	pub plan: CompletionRenderPlan,
}

fn command_query_is_exact_alias(query: &str, label: &str) -> bool {
	let query = query.trim();
	if query.is_empty() {
		return false;
	}

	let Some(command) = xeno_registry::commands::find_command(query) else {
		return false;
	};

	!command.name_str().eq_ignore_ascii_case(query) && command.name_str().eq_ignore_ascii_case(label)
}

impl Editor {
	/// Returns visible completion row count for a bounded menu viewport.
	pub fn completion_visible_rows(&self, max_visible_rows: usize) -> usize {
		let mut completions = self.overlays().get::<CompletionState>().cloned().unwrap_or_default();
		if !completions.active || completions.items.is_empty() {
			return 0;
		}

		let normalized_rows = max_visible_rows.max(1);
		completions.ensure_selected_visible_with_limit(normalized_rows);
		completions.visible_range_with_limit(normalized_rows).len()
	}

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

	/// Builds a data-only completion render plan for a target menu width.
	///
	/// The plan applies selection visibility and row-windowing policy so
	/// frontends can render list rows without reading completion internals.
	pub fn completion_render_plan(&self, menu_width: u16, max_visible_rows: usize) -> Option<CompletionRenderPlan> {
		let mut completions = self.overlays().get::<CompletionState>().cloned().unwrap_or_default();
		if !completions.active || completions.items.is_empty() {
			return None;
		}

		let normalized_rows = max_visible_rows.max(1);
		completions.ensure_selected_visible_with_limit(normalized_rows);

		let max_label_width = completions.items.iter().map(|item| item.label.width()).max().unwrap_or(0);
		let show_kind = completions.show_kind && menu_width >= 24;
		let show_right = !completions.show_kind && menu_width >= 30;
		let visible_range = completions.visible_range_with_limit(normalized_rows);
		let selected_idx = completions.selected_idx;

		let items = completions
			.items
			.iter()
			.enumerate()
			.filter(|(idx, _)| visible_range.contains(idx))
			.map(|(idx, item)| CompletionRenderItem {
				label: item.label.clone(),
				kind: item.kind,
				right: item.right.clone(),
				match_indices: item.match_indices.clone(),
				selected: Some(idx) == selected_idx,
				command_alias_match: item.kind == CompletionKind::Command && command_query_is_exact_alias(&completions.query, &item.label),
			})
			.collect();

		Some(CompletionRenderPlan {
			items,
			max_label_width,
			target_row_width: menu_width.saturating_sub(1) as usize,
			show_kind,
			show_right,
		})
	}

	/// Builds a data-only completion render plan for cursor-anchored popups.
	pub fn completion_popup_render_plan(&self) -> Option<CompletionRenderPlan> {
		let area = self.completion_popup_area()?;
		self.completion_render_plan(area.width, CompletionState::MAX_VISIBLE)
	}

	/// Completion menu target for command palette/file picker overlays.
	pub fn overlay_completion_menu_target(&self) -> Option<OverlayCompletionMenuTarget> {
		if !matches!(
			self.overlay_kind(),
			Some(OverlayControllerKind::CommandPalette | OverlayControllerKind::FilePicker)
		) {
			return None;
		}

		let input_rect = self.overlay_pane_rect(WindowRole::Input)?;
		let panel_height = self.utility_overlay_height_hint()?;
		let panel_top = input_rect.y.saturating_sub(panel_height.saturating_sub(1));
		let visible_rows = self.completion_visible_rows(CompletionState::MAX_VISIBLE) as u16;
		let rect = overlay_menu_rect_above_input(panel_top, input_rect, visible_rows)?;
		let plan = self.completion_render_plan(rect.width, rect.height as usize)?;

		Some(OverlayCompletionMenuTarget { rect, plan })
	}
}

fn overlay_menu_rect_above_input(panel_top: u16, input: Rect, visible_rows: u16) -> Option<Rect> {
	if input.width == 0 || visible_rows == 0 || panel_top >= input.y {
		return None;
	}

	let available_rows = input.y.saturating_sub(panel_top);
	let menu_height = visible_rows.min(available_rows);
	if menu_height == 0 {
		return None;
	}

	let menu_y = input.y.saturating_sub(menu_height);
	Some(Rect::new(input.x, menu_y, input.width, menu_height))
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::completion::CompletionItem;

	fn item(label: &str, kind: CompletionKind) -> CompletionItem {
		CompletionItem {
			label: label.to_string(),
			insert_text: label.to_string(),
			detail: None,
			filter_text: None,
			kind,
			match_indices: None,
			right: Some("meta".to_string()),
		}
	}

	#[test]
	fn completion_render_plan_keeps_selected_item_visible_with_limit() {
		let mut editor = Editor::new_scratch();

		let state = editor.overlays_mut().get_or_default::<CompletionState>();
		state.active = true;
		state.items = vec![
			item("item0", CompletionKind::Command),
			item("item1", CompletionKind::Command),
			item("item2", CompletionKind::Command),
			item("item3", CompletionKind::Command),
			item("item4", CompletionKind::Command),
		];
		state.selected_idx = Some(3);
		state.scroll_offset = 0;
		state.show_kind = true;

		let plan = editor.completion_render_plan(40, 2).expect("plan should exist");
		assert_eq!(plan.items.len(), 2);
		assert_eq!(plan.items[0].label, "item2");
		assert_eq!(plan.items[1].label, "item3");
		assert!(!plan.items[0].selected);
		assert!(plan.items[1].selected);
	}

	#[test]
	fn completion_render_plan_applies_width_column_policy() {
		let mut editor = Editor::new_scratch();

		let state = editor.overlays_mut().get_or_default::<CompletionState>();
		state.active = true;
		state.items = vec![item("entry", CompletionKind::File)];
		state.selected_idx = Some(0);
		state.show_kind = true;

		let narrow = editor.completion_render_plan(20, 10).expect("plan should exist");
		assert!(!narrow.show_kind);
		assert!(!narrow.show_right);

		let state = editor.overlays_mut().get_or_default::<CompletionState>();
		state.show_kind = false;

		let wide = editor.completion_render_plan(31, 10).expect("plan should exist");
		assert!(!wide.show_kind);
		assert!(wide.show_right);
	}

	#[test]
	fn overlay_menu_rect_returns_none_when_no_space_above_input() {
		let input = Rect::new(10, 12, 30, 1);
		assert_eq!(overlay_menu_rect_above_input(12, input, 5), None);
	}

	#[test]
	fn overlay_menu_rect_clamps_height_to_available_rows() {
		let input = Rect::new(10, 15, 30, 1);
		let menu = overlay_menu_rect_above_input(10, input, 20).expect("menu should exist");
		assert_eq!(menu.y, 10);
		assert_eq!(menu.height, 5);
	}

	#[test]
	fn overlay_menu_rect_returns_none_for_zero_width_or_rows() {
		assert_eq!(overlay_menu_rect_above_input(0, Rect::new(10, 5, 0, 1), 4), None);
		assert_eq!(overlay_menu_rect_above_input(0, Rect::new(10, 5, 20, 1), 0), None);
	}

	#[test]
	fn overlay_completion_menu_target_uses_utility_panel_bounds() {
		let mut editor = Editor::new_scratch();
		editor.handle_window_resize(80, 24);
		assert!(editor.open_command_palette());

		let completion_count = 16;
		let state = editor.overlays_mut().get_or_default::<CompletionState>();
		state.active = true;
		state.items = (0..completion_count).map(|idx| item(&format!("item-{idx}"), CompletionKind::Command)).collect();
		state.selected_idx = Some(0);
		state.scroll_offset = 0;

		let input = editor.overlay_pane_rect(WindowRole::Input).expect("input pane");
		let hint = editor.utility_overlay_height_hint().expect("utility overlay height hint");
		let visible_rows = editor.completion_visible_rows(CompletionState::MAX_VISIBLE) as u16;

		let target = editor.overlay_completion_menu_target().expect("overlay completion target");
		assert_eq!(target.rect.x, input.x);
		assert_eq!(target.rect.width, input.width);
		assert_eq!(target.rect.y + target.rect.height, input.y);

		let panel_top = input.y.saturating_sub(hint.saturating_sub(1));
		assert!(target.rect.y >= panel_top);
		assert_eq!(target.rect.height, hint.saturating_sub(1).min(visible_rows));
		assert_eq!(target.plan.items.len() as u16, target.rect.height);
	}
}
