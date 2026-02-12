use xeno_editor::Editor;
use xeno_editor::info_popup::InfoPopupRenderAnchor;
use xeno_editor::render_api::{BufferRenderContext, RenderBufferParams, RenderCtx};
use xeno_editor::window::GutterSelector;
use xeno_tui::layout::Rect;
use xeno_tui::style::Style;
use xeno_tui::widgets::block::Padding;
use xeno_tui::widgets::{Block, Clear, Paragraph};

use crate::layer::SceneBuilder;
use crate::scene::{SurfaceKind, SurfaceOp};

fn compute_popup_rect(anchor: InfoPopupRenderAnchor, content_width: u16, content_height: u16, bounds: Rect) -> Rect {
	let width = content_width.saturating_add(2).min(bounds.width.saturating_sub(4));
	let height = content_height.saturating_add(2).min(bounds.height.saturating_sub(2));

	let (x, y) = match anchor {
		InfoPopupRenderAnchor::Center => (
			bounds.x + bounds.width.saturating_sub(width) / 2,
			bounds.y + bounds.height.saturating_sub(height) / 2,
		),
		InfoPopupRenderAnchor::Point { x, y } => (
			x.max(bounds.x).min(bounds.x + bounds.width.saturating_sub(width)),
			y.max(bounds.y).min(bounds.y + bounds.height.saturating_sub(height)),
		),
	};

	Rect::new(x, y, width, height)
}

pub fn visible(ed: &Editor) -> bool {
	ed.info_popup_count() > 0
}

pub fn push(builder: &mut SceneBuilder, doc_area: Rect) {
	builder.push(SurfaceKind::InfoPopups, 25, doc_area, SurfaceOp::InfoPopups, false);
}

pub fn render(ed: &mut Editor, frame: &mut xeno_tui::Frame, doc_area: Rect, ctx: &RenderCtx) {
	let popups = ed.info_popup_render_plan();

	if popups.is_empty() {
		return;
	}

	let mut cache = std::mem::take(ed.render_cache_mut());
	let language_loader = &ed.config().language_loader;
	let padding = Padding::horizontal(1);

	for popup in popups {
		let max_w = doc_area.width.saturating_sub(2).min(60);
		let max_h = doc_area.height.saturating_sub(2).min(12);
		if max_w == 0 || max_h == 0 {
			continue;
		}
		let width = popup.content_width.min(max_w);
		let height = popup.content_height.min(max_h);
		let rect: Rect = compute_popup_rect(popup.anchor, width, height, doc_area);
		if rect.width == 0 || rect.height == 0 {
			continue;
		}

		frame.render_widget(Clear, rect);

		let block = Block::default().style(Style::default().bg(ctx.theme.colors.popup.bg)).padding(padding);

		let inner = block.inner(rect);
		frame.render_widget(block, rect);

		if inner.width == 0 || inner.height == 0 {
			continue;
		}

		let Some(buffer) = ed.core().buffers.get_buffer(popup.buffer_id) else {
			continue;
		};

		let tab_width = ed.tab_width_for(popup.buffer_id);
		let cursorline = ed.cursorline_for(popup.buffer_id);

		let buffer_ctx = BufferRenderContext {
			theme: &ctx.theme,
			language_loader,
			syntax_manager: ed.syntax_manager(),
			diagnostics: ctx.lsp.diagnostics_for(popup.buffer_id),
			diagnostic_ranges: ctx.lsp.diagnostic_ranges_for(popup.buffer_id),
		};

		let result = buffer_ctx.render_buffer_with_gutter(RenderBufferParams {
			buffer,
			area: inner.into(),
			use_block_cursor: false,
			is_focused: false,
			gutter: GutterSelector::Hidden,
			tab_width,
			cursorline,
			cache: &mut cache,
		});

		let gutter_area = Rect {
			width: result.gutter_width,
			..inner
		};
		let text_area = Rect {
			x: inner.x + result.gutter_width,
			width: inner.width.saturating_sub(result.gutter_width),
			..inner
		};

		let gutter = result.gutter.into_iter().map(|line| line.into_text_line()).collect::<Vec<_>>();
		let text = result.text.into_iter().map(|line| line.into_text_line()).collect::<Vec<_>>();

		frame.render_widget(Paragraph::new(gutter), gutter_area);
		frame.render_widget(Paragraph::new(text), text_area);
	}

	*ed.render_cache_mut() = cache;
}

#[cfg(test)]
mod tests {
	use xeno_editor::info_popup::InfoPopupRenderAnchor;
	use xeno_tui::layout::Rect;

	use super::compute_popup_rect;

	#[test]
	fn popup_rect_centers_in_bounds() {
		let bounds = Rect::new(0, 1, 80, 22);
		let rect = compute_popup_rect(InfoPopupRenderAnchor::Center, 20, 5, bounds);
		assert!(rect.x > bounds.x);
		assert!(rect.y > bounds.y);
		assert!(rect.x + rect.width < bounds.x + bounds.width);
		assert!(rect.y + rect.height < bounds.y + bounds.height);
	}

	#[test]
	fn popup_rect_clamps_point_to_bounds() {
		let bounds = Rect::new(0, 1, 80, 22);
		let rect = compute_popup_rect(InfoPopupRenderAnchor::Point { x: 100, y: 100 }, 20, 5, bounds);
		assert!(rect.x + rect.width <= bounds.x + bounds.width);
		assert!(rect.y + rect.height <= bounds.y + bounds.height);
	}

	#[test]
	fn popup_rect_respects_point_position() {
		let bounds = Rect::new(0, 1, 80, 22);
		let rect = compute_popup_rect(InfoPopupRenderAnchor::Point { x: 10, y: 5 }, 20, 5, bounds);
		assert_eq!(rect.x, 10);
		assert_eq!(rect.y, 5);
	}
}
