use super::{BufferRenderContext, GutterLayout, RenderLine, ensure_buffer_cursor_visible};
use crate::Editor;
use crate::buffer::ViewId;
use crate::geometry::Rect;

#[derive(Debug, Clone)]
pub struct BufferViewRenderPlan {
	pub gutter_width: u16,
	pub gutter: Vec<RenderLine<'static>>,
	pub text: Vec<RenderLine<'static>>,
}

impl Editor {
	/// Renders a single view into data-only gutter and text lines.
	pub fn buffer_view_render_plan(&mut self, view: ViewId, area: Rect, use_block_cursor: bool, is_focused: bool) -> Option<BufferViewRenderPlan> {
		self.buffer_view_render_plan_with_gutter(view, area, use_block_cursor, is_focused, crate::window::GutterSelector::Registry)
	}

	/// Renders a single view into data-only gutter and text lines with an explicit gutter policy.
	pub fn buffer_view_render_plan_with_gutter(
		&mut self,
		view: ViewId,
		area: Rect,
		use_block_cursor: bool,
		is_focused: bool,
		gutter: crate::window::GutterSelector,
	) -> Option<BufferViewRenderPlan> {
		self.ensure_syntax_for_buffers();
		if area.width == 0 || area.height == 0 {
			return None;
		}

		let tab_width = self.tab_width_for(view);
		let mouse_drag_active = self.layout().text_selection_origin.is_some();
		let scroll_margin = if mouse_drag_active { 0 } else { self.scroll_margin_for(view) };

		{
			let buffer = self.get_buffer_mut(view)?;
			let total_lines = buffer.with_doc(|doc| doc.content().len_lines());
			let is_diff_file = buffer.file_type().is_some_and(|ft| ft == "diff");
			let effective_gutter = if is_diff_file {
				BufferRenderContext::diff_gutter_selector(gutter)
			} else {
				gutter
			};

			let gutter_layout = GutterLayout::from_selector(effective_gutter, total_lines, area.width);
			let text_width = area.width.saturating_sub(gutter_layout.total_width) as usize;
			ensure_buffer_cursor_visible(buffer, area, text_width, tab_width, scroll_margin);
		}

		let render_ctx = self.render_ctx();
		let mut cache = std::mem::take(self.render_cache_mut());
		let cursorline = self.cursorline_for(view);

		let buffer = self.get_buffer(view)?;
		let buffer_ctx = BufferRenderContext {
			theme: &render_ctx.theme,
			language_loader: &self.config().language_loader,
			syntax_manager: self.syntax_manager(),
			diagnostics: render_ctx.lsp.diagnostics_for(view),
			diagnostic_ranges: render_ctx.lsp.diagnostic_ranges_for(view),
		};

		let result = buffer_ctx.render_buffer(buffer, area, use_block_cursor, is_focused, tab_width, cursorline, &mut cache);
		*self.render_cache_mut() = cache;

		Some(BufferViewRenderPlan {
			gutter_width: result.gutter_width,
			gutter: result.gutter,
			text: result.text,
		})
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn buffer_view_render_plan_renders_for_focused_view_area() {
		let mut editor = Editor::new_scratch();
		editor.handle_window_resize(80, 24);
		let view = editor.focused_view();
		let area = editor.view_area(view);

		let plan = editor.buffer_view_render_plan(view, area, true, true).expect("render plan for focused view");
		assert!(!plan.text.is_empty());
	}

	#[test]
	fn buffer_view_render_plan_returns_none_for_missing_view() {
		let mut editor = Editor::new_scratch();
		editor.handle_window_resize(80, 24);

		let area = Rect::new(0, 0, 80, 24);
		assert!(editor.buffer_view_render_plan(ViewId(u64::MAX), area, true, false).is_none());
	}

	#[test]
	fn buffer_view_render_plan_gutter_width_fits_area() {
		let mut editor = Editor::new_scratch();
		editor.handle_window_resize(40, 10);
		let view = editor.focused_view();
		let area = editor.view_area(view);

		let plan = editor.buffer_view_render_plan(view, area, true, true).expect("render plan for focused view");
		assert!(plan.gutter_width <= area.width);
	}

	#[test]
	fn buffer_view_render_plan_with_gutter_renders_with_requested_policy() {
		let mut editor = Editor::new_scratch();
		editor.handle_window_resize(40, 10);
		let view = editor.focused_view();
		let area = editor.view_area(view);

		let plan = editor
			.buffer_view_render_plan_with_gutter(view, area, true, true, crate::window::GutterSelector::Registry)
			.expect("render plan for focused view");
		assert!(plan.gutter_width <= area.width);
	}
}
