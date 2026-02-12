use xeno_editor::overlay::{OverlayControllerKind, OverlayPaneRenderTarget, WindowRole};
use xeno_editor::render_api::{BufferRenderContext, GutterLayout, RenderBufferParams, RenderCtx, ensure_buffer_cursor_visible};
use xeno_editor::window::SurfaceStyle;
use xeno_editor::{Editor, FocusTarget};
use xeno_tui::layout::Rect;
use xeno_tui::style::Style;
use xeno_tui::widgets::{Block, Borders, Paragraph};

use crate::render_adapter::to_tui_lines;

fn clamp_rect(rect: Rect, bounds: Rect) -> Option<Rect> {
	let x1 = rect.x.max(bounds.x);
	let y1 = rect.y.max(bounds.y);
	let x2 = rect.right().min(bounds.right());
	let y2 = rect.bottom().min(bounds.bottom());

	if x2 <= x1 || y2 <= y1 {
		return None;
	}

	Some(Rect {
		x: x1,
		y: y1,
		width: x2.saturating_sub(x1),
		height: y2.saturating_sub(y1),
	})
}

fn pane_content_area(rect: Rect, style: &SurfaceStyle) -> Rect {
	let mut area = rect;
	if area.width == 0 || area.height == 0 {
		return Rect::new(0, 0, 0, 0);
	}

	// Reserve one column for the docked stripe.
	if area.width > 0 {
		area.x = area.x.saturating_add(1);
		area.width = area.width.saturating_sub(1);
	}

	area.x = area.x.saturating_add(style.padding.left);
	area.y = area.y.saturating_add(style.padding.top);
	area.width = area.width.saturating_sub(style.padding.left.saturating_add(style.padding.right));
	area.height = area.height.saturating_sub(style.padding.top.saturating_add(style.padding.bottom));

	area
}

fn render_palette_completion_menu(ed: &mut Editor, frame: &mut xeno_tui::Frame, area: Rect) {
	if !matches!(
		ed.overlay_kind(),
		Some(OverlayControllerKind::CommandPalette | OverlayControllerKind::FilePicker)
	) {
		return;
	}

	let input_rect = ed.overlay_pane_rect(WindowRole::Input).map(|rect| -> Rect { rect.into() });
	let Some(input_rect) = input_rect else {
		return;
	};

	let panel_top = area.y;
	let menu_bottom = input_rect.y;
	if panel_top >= menu_bottom {
		return;
	}

	let visible_rows = ed.completion_visible_rows(10) as u16;
	let available_rows = menu_bottom.saturating_sub(panel_top);
	let menu_height = visible_rows.min(available_rows);
	if menu_height == 0 {
		return;
	}

	let menu_y = menu_bottom.saturating_sub(menu_height);
	let menu_rect = Rect::new(input_rect.x, menu_y, input_rect.width, menu_height);
	let Some(plan) = ed.completion_render_plan(menu_rect.width, menu_height as usize) else {
		return;
	};
	crate::layers::completion::render_completion_menu(ed, frame, menu_rect, plan);
}

pub fn render_utility_panel_overlay(ed: &mut Editor, frame: &mut xeno_tui::Frame, area: Rect, ctx: &RenderCtx) {
	let panes: Vec<OverlayPaneRenderTarget> = ed.overlay_pane_render_plan();

	if panes.is_empty() {
		return;
	}

	for pane in &panes {
		let pane_rect: Rect = pane.rect.into();
		let Some(rect) = clamp_rect(pane_rect, area) else {
			continue;
		};
		let content_area = pane_content_area(rect, &pane.style);
		if content_area.width == 0 || content_area.height == 0 {
			continue;
		}

		let tab_width = ed.tab_width_for(pane.buffer);
		let scroll_margin = ed.scroll_margin_for(pane.buffer);
		if let Some(buffer) = ed.get_buffer_mut(pane.buffer) {
			let total_lines = buffer.with_doc(|doc| doc.content().len_lines());
			let is_diff_file = buffer.file_type().is_some_and(|ft| ft == "diff");
			let effective_gutter = if is_diff_file {
				BufferRenderContext::diff_gutter_selector(pane.gutter)
			} else {
				pane.gutter
			};

			let gutter_layout = GutterLayout::from_selector(effective_gutter, total_lines, content_area.width);
			let text_width = content_area.width.saturating_sub(gutter_layout.total_width) as usize;

			ensure_buffer_cursor_visible(buffer, content_area.into(), text_width, tab_width, scroll_margin);
		}
	}

	let focused_overlay = match ed.focus() {
		FocusTarget::Overlay { buffer } => Some(*buffer),
		_ => None,
	};

	let mut cache = std::mem::take(ed.render_cache_mut());
	let language_loader = &ed.config().language_loader;

	for pane in panes {
		let pane_rect: Rect = pane.rect.into();
		let Some(rect) = clamp_rect(pane_rect, area) else {
			continue;
		};

		let stripe_style = Style::default().fg(ctx.theme.colors.mode.normal.bg);
		let stripe_border_set = xeno_tui::symbols::border::Set {
			top_left: "▏",
			vertical_left: "▏",
			bottom_left: "▏",
			..xeno_tui::symbols::border::EMPTY
		};
		let block = Block::default()
			.style(Style::default().bg(ctx.theme.colors.popup.bg))
			.borders(Borders::LEFT)
			.border_set(stripe_border_set)
			.border_style(stripe_style);

		let content_area = pane_content_area(rect, &pane.style);
		frame.render_widget(block, rect);

		if content_area.width == 0 || content_area.height == 0 {
			continue;
		}

		if let Some(buffer) = ed.get_buffer(pane.buffer) {
			let tab_width = ed.tab_width_for(pane.buffer);
			let cursorline = ed.cursorline_for(pane.buffer);

			let buffer_ctx = BufferRenderContext {
				theme: &ctx.theme,
				language_loader,
				syntax_manager: ed.syntax_manager(),
				diagnostics: ctx.lsp.diagnostics_for(pane.buffer),
				diagnostic_ranges: ctx.lsp.diagnostic_ranges_for(pane.buffer),
			};
			let result = buffer_ctx.render_buffer_with_gutter(RenderBufferParams {
				buffer,
				area: content_area.into(),
				use_block_cursor: true,
				is_focused: focused_overlay == Some(pane.buffer),
				gutter: pane.gutter,
				tab_width,
				cursorline,
				cache: &mut cache,
			});

			let gutter_area = Rect {
				width: result.gutter_width,
				..content_area
			};
			let text_area = Rect {
				x: content_area.x + result.gutter_width,
				width: content_area.width.saturating_sub(result.gutter_width),
				..content_area
			};

			let gutter = to_tui_lines(result.gutter);
			let text = to_tui_lines(result.text);

			frame.render_widget(Paragraph::new(gutter), gutter_area);
			frame.render_widget(Paragraph::new(text), text_area);
		}
	}

	*ed.render_cache_mut() = cache;
	render_palette_completion_menu(ed, frame, area);
}
