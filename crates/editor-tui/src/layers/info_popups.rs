use xeno_editor::info_popup::{InfoPopupStore, PopupAnchor};
use xeno_editor::render::{BufferRenderContext, RenderBufferParams, RenderCtx};
use xeno_editor::window::GutterSelector;
use xeno_editor::{Editor, ViewId};
use xeno_registry::options::keys;
use xeno_tui::layout::Rect;
use xeno_tui::style::Style;
use xeno_tui::widgets::block::Padding;
use xeno_tui::widgets::{Block, Clear, Paragraph};

use crate::layer::SceneBuilder;
use crate::scene::{SurfaceKind, SurfaceOp};

fn compute_popup_rect(anchor: PopupAnchor, content_width: u16, content_height: u16, bounds: Rect) -> Rect {
	let width = content_width.saturating_add(2).min(bounds.width.saturating_sub(4));
	let height = content_height.saturating_add(2).min(bounds.height.saturating_sub(2));

	let (x, y) = match anchor {
		PopupAnchor::Center => (
			bounds.x + bounds.width.saturating_sub(width) / 2,
			bounds.y + bounds.height.saturating_sub(height) / 2,
		),
		PopupAnchor::Point { x, y } => (
			x.max(bounds.x).min(bounds.x + bounds.width.saturating_sub(width)),
			y.max(bounds.y).min(bounds.y + bounds.height.saturating_sub(height)),
		),
		PopupAnchor::Window(_) => (
			bounds.x + bounds.width.saturating_sub(width) / 2,
			bounds.y + bounds.height.saturating_sub(height) / 2,
		), // TODO: position adjacent to window
	};

	Rect::new(x, y, width, height)
}

pub fn visible(ed: &Editor) -> bool {
	ed.overlays().get::<InfoPopupStore>().is_some_and(|store| !store.is_empty())
}

pub fn push(builder: &mut SceneBuilder, doc_area: Rect) {
	builder.push(SurfaceKind::InfoPopups, 25, doc_area, SurfaceOp::InfoPopups, false);
}

pub fn render(ed: &mut Editor, frame: &mut xeno_tui::Frame, doc_area: Rect, ctx: &RenderCtx) {
	let mut popups: Vec<(u64, ViewId, PopupAnchor, u16, u16)> = ed
		.overlays()
		.get::<InfoPopupStore>()
		.map(|store| {
			store
				.ids()
				.filter_map(|id| {
					store
						.get(id)
						.map(|popup| (id.0, popup.buffer_id, popup.anchor, popup.content_width, popup.content_height))
				})
				.collect()
		})
		.unwrap_or_default();

	popups.sort_by_key(|(id, ..)| *id);

	if popups.is_empty() {
		return;
	}

	let mut cache = std::mem::take(ed.render_cache_mut());
	let language_loader = &ed.config().language_loader;
	let padding = Padding::horizontal(1);

	for (_, buffer_id, anchor, content_width, content_height) in popups {
		let max_w = doc_area.width.saturating_sub(2).min(60);
		let max_h = doc_area.height.saturating_sub(2).min(12);
		if max_w == 0 || max_h == 0 {
			continue;
		}
		let width = content_width.min(max_w);
		let height = content_height.min(max_h);
		let rect: Rect = compute_popup_rect(anchor, width, height, doc_area);
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

		let Some(buffer) = ed.core().buffers.get_buffer(buffer_id) else {
			continue;
		};

		let tab_width = (buffer.option(keys::TAB_WIDTH, ed) as usize).max(1);
		let cursorline = buffer.option(keys::CURSORLINE, ed);

		let buffer_ctx = BufferRenderContext {
			theme: &ctx.theme,
			language_loader,
			syntax_manager: ed.syntax_manager(),
			diagnostics: ctx.lsp.diagnostics_for(buffer_id),
			diagnostic_ranges: ctx.lsp.diagnostic_ranges_for(buffer_id),
		};

		let result = buffer_ctx.render_buffer_with_gutter(RenderBufferParams {
			buffer,
			area: inner,
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

		frame.render_widget(Paragraph::new(result.gutter), gutter_area);
		frame.render_widget(Paragraph::new(result.text), text_area);
	}

	*ed.render_cache_mut() = cache;
}

#[cfg(test)]
mod tests {
	use xeno_editor::info_popup::PopupAnchor;
	use xeno_tui::layout::Rect;

	use super::compute_popup_rect;

	#[test]
	fn popup_rect_centers_in_bounds() {
		let bounds = Rect::new(0, 1, 80, 22);
		let rect = compute_popup_rect(PopupAnchor::Center, 20, 5, bounds);
		assert!(rect.x > bounds.x);
		assert!(rect.y > bounds.y);
		assert!(rect.x + rect.width < bounds.x + bounds.width);
		assert!(rect.y + rect.height < bounds.y + bounds.height);
	}

	#[test]
	fn popup_rect_clamps_point_to_bounds() {
		let bounds = Rect::new(0, 1, 80, 22);
		let rect = compute_popup_rect(PopupAnchor::Point { x: 100, y: 100 }, 20, 5, bounds);
		assert!(rect.x + rect.width <= bounds.x + bounds.width);
		assert!(rect.y + rect.height <= bounds.y + bounds.height);
	}

	#[test]
	fn popup_rect_respects_point_position() {
		let bounds = Rect::new(0, 1, 80, 22);
		let rect = compute_popup_rect(PopupAnchor::Point { x: 10, y: 5 }, 20, 5, bounds);
		assert_eq!(rect.x, 10);
		assert_eq!(rect.y, 5);
	}
}
