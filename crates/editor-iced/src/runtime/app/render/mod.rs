use iced::widget::text::Wrapping;
use iced::widget::{column, container, rich_text, span, text};
use iced::{Background, Color, Element, Fill, Font, Pixels, border, font};
use xeno_editor::Editor;
use xeno_editor::render_api::{
	CompletionKind, CompletionRenderItem, CompletionRenderPlan, RenderLine, StatuslineRenderSegment,
	StatuslineRenderStyle,
};
use xeno_primitives::{Color as UiColor, Style as UiStyle};

use super::Message;

pub(super) fn render_document_line(line: &RenderLine<'_>, line_height_px: f32) -> Element<'static, Message> {
	let mut spans = Vec::new();
	let line_color = line.style.and_then(style_fg_to_iced);
	let line_bg = line.style.and_then(style_bg_to_iced);

	for render_span in &line.spans {
		let mut segment = span::<(), _>(render_span.content.as_ref().to_string());
		if let Some(color) = style_fg_to_iced(render_span.style).or(line_color) {
			segment = segment.color(color);
		}
		if let Some(bg) = style_bg_to_iced(render_span.style).or(line_bg) {
			segment = segment.background(Background::Color(bg)).border(border::rounded(0));
		}
		spans.push(segment);
	}

	if spans.is_empty() {
		spans.push(span::<(), _>(String::new()));
	}

	rich_text(spans)
		.font(Font::MONOSPACE)
		.line_height(Pixels(line_height_px))
		.wrapping(Wrapping::None)
		.into()
}

pub(super) fn render_render_lines(lines: &[RenderLine<'_>], line_height_px: f32) -> Element<'static, Message> {
	let mut rows = column![].spacing(0);
	for line in lines {
		rows = rows.push(render_document_line(line, line_height_px));
	}
	rows.into()
}

pub(super) fn render_statusline(editor: &Editor, segments: &[StatuslineRenderSegment], line_height_px: f32) -> Element<'static, Message> {
	let mut spans = Vec::new();

	for segment in segments {
		let mut item = span::<(), _>(segment.text.clone()).font(Font::MONOSPACE);
		let style = editor.statusline_segment_style(segment.style);
		if let Some(color) = style_fg_to_iced(style) {
			item = item.color(color);
		}
		if let Some(bg) = style_bg_to_iced(style) {
			item = item.background(Background::Color(bg)).border(border::rounded(0));
		}
		if matches!(segment.style, StatuslineRenderStyle::Mode) {
			item = item.font(Font {
				weight: font::Weight::Bold,
				..Font::MONOSPACE
			});
		}
		spans.push(item);
	}

	if spans.is_empty() {
		spans.push(span::<(), _>(String::new()).font(Font::MONOSPACE));
	}

	rich_text(spans)
		.font(Font::MONOSPACE)
		.line_height(Pixels(line_height_px))
		.wrapping(Wrapping::None)
		.into()
}

pub(super) fn render_palette_completion_menu(editor: &Editor, plan: &CompletionRenderPlan, line_height_px: f32) -> Element<'static, Message> {
	let popup_bg = editor.config().theme.colors.popup.bg;
	let popup_fg = editor.config().theme.colors.popup.fg;
	let selected_bg = editor.config().theme.colors.ui.selection_bg;
	let selected_fg = editor.config().theme.colors.ui.selection_fg;

	let mut rows = column![].spacing(0).width(Fill);
	for item in &plan.items {
		let row_bg = if item.selected { selected_bg } else { popup_bg };
		let row_fg = if item.selected { selected_fg } else { popup_fg };
		let mut row_text = text(format_palette_completion_row(plan, item))
			.font(Font::MONOSPACE)
			.wrapping(Wrapping::None)
			.line_height(Pixels(line_height_px));
		if let Some(color) = map_ui_color(row_fg) {
			row_text = row_text.color(color);
		}

		rows = rows.push(container(row_text).width(Fill).padding([0, 1]).style(move |_theme| background_style(row_bg)));
	}

	container(rows).width(Fill).style(move |_theme| background_style(popup_bg)).into()
}

pub(super) fn format_palette_completion_row(plan: &CompletionRenderPlan, item: &CompletionRenderItem) -> String {
	let mut line = String::new();
	line.push(' ');
	line.push_str(completion_icon(item.kind));
	line.push(' ');
	line.push_str(&pad_right(&item.label, plan.max_label_width));

	if plan.show_kind {
		line.push(' ');
		line.push('[');
		line.push_str(completion_kind_label(item.kind));
		line.push(']');
	}

	if plan.show_right
		&& let Some(right) = item.right.as_deref()
	{
		line.push(' ');
		line.push_str(right);
	}

	line
}

fn completion_kind_label(kind: CompletionKind) -> &'static str {
	match kind {
		CompletionKind::Command => "Cmd",
		CompletionKind::File => "File",
		CompletionKind::Buffer => "Buf",
		CompletionKind::Snippet => "Snip",
		CompletionKind::Theme => "Theme",
	}
}

fn completion_icon(kind: CompletionKind) -> &'static str {
	match kind {
		CompletionKind::Command => "C",
		CompletionKind::File => "F",
		CompletionKind::Buffer => "B",
		CompletionKind::Snippet => "S",
		CompletionKind::Theme => "T",
	}
}

fn pad_right(value: &str, min_width: usize) -> String {
	let width = value.chars().count();
	let pad = min_width.saturating_sub(width);
	if pad == 0 {
		return value.to_string();
	}

	let mut out = String::with_capacity(value.len() + pad);
	out.push_str(value);
	out.push_str(&" ".repeat(pad));
	out
}

pub(super) fn style_fg_to_iced(style: UiStyle) -> Option<Color> {
	style.fg.and_then(map_ui_color)
}

pub(super) fn style_bg_to_iced(style: UiStyle) -> Option<Color> {
	style.bg.and_then(map_ui_color)
}

pub(super) fn map_ui_color(color: UiColor) -> Option<Color> {
	match color {
		UiColor::Reset => None,
		UiColor::Black => Some(Color::from_rgb8(0x00, 0x00, 0x00)),
		UiColor::Red => Some(Color::from_rgb8(0x80, 0x00, 0x00)),
		UiColor::Green => Some(Color::from_rgb8(0x00, 0x80, 0x00)),
		UiColor::Yellow => Some(Color::from_rgb8(0x80, 0x80, 0x00)),
		UiColor::Blue => Some(Color::from_rgb8(0x00, 0x00, 0x80)),
		UiColor::Magenta => Some(Color::from_rgb8(0x80, 0x00, 0x80)),
		UiColor::Cyan => Some(Color::from_rgb8(0x00, 0x80, 0x80)),
		UiColor::Gray => Some(Color::from_rgb8(0xC0, 0xC0, 0xC0)),
		UiColor::DarkGray => Some(Color::from_rgb8(0x80, 0x80, 0x80)),
		UiColor::LightRed => Some(Color::from_rgb8(0xFF, 0x00, 0x00)),
		UiColor::LightGreen => Some(Color::from_rgb8(0x00, 0xFF, 0x00)),
		UiColor::LightYellow => Some(Color::from_rgb8(0xFF, 0xFF, 0x00)),
		UiColor::LightBlue => Some(Color::from_rgb8(0x00, 0x00, 0xFF)),
		UiColor::LightMagenta => Some(Color::from_rgb8(0xFF, 0x00, 0xFF)),
		UiColor::LightCyan => Some(Color::from_rgb8(0x00, 0xFF, 0xFF)),
		UiColor::White => Some(Color::from_rgb8(0xFF, 0xFF, 0xFF)),
		UiColor::Rgb(r, g, b) => Some(Color::from_rgb8(r, g, b)),
		UiColor::Indexed(index) => Some(map_indexed_color(index)),
	}
}

pub(super) fn background_style(bg: UiColor) -> container::Style {
	let bg = map_ui_color(bg).unwrap_or(Color::BLACK);
	container::Style::default().background(bg)
}

fn map_indexed_color(index: u8) -> Color {
	const BASE: [(u8, u8, u8); 16] = [
		(0x00, 0x00, 0x00),
		(0x80, 0x00, 0x00),
		(0x00, 0x80, 0x00),
		(0x80, 0x80, 0x00),
		(0x00, 0x00, 0x80),
		(0x80, 0x00, 0x80),
		(0x00, 0x80, 0x80),
		(0xC0, 0xC0, 0xC0),
		(0x80, 0x80, 0x80),
		(0xFF, 0x00, 0x00),
		(0x00, 0xFF, 0x00),
		(0xFF, 0xFF, 0x00),
		(0x00, 0x00, 0xFF),
		(0xFF, 0x00, 0xFF),
		(0x00, 0xFF, 0xFF),
		(0xFF, 0xFF, 0xFF),
	];
	const CUBE: [u8; 6] = [0, 95, 135, 175, 215, 255];

	if index < 16 {
		let (r, g, b) = BASE[index as usize];
		return Color::from_rgb8(r, g, b);
	}

	if (16..=231).contains(&index) {
		let value = index - 16;
		let r = CUBE[(value / 36) as usize];
		let g = CUBE[((value % 36) / 6) as usize];
		let b = CUBE[(value % 6) as usize];
		return Color::from_rgb8(r, g, b);
	}

	let gray = 8u8.saturating_add((index - 232) * 10);
	Color::from_rgb8(gray, gray, gray)
}

#[cfg(test)]
mod tests;
