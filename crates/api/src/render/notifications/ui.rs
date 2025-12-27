use tome_manifest::notifications::Level;
use tome_tui::layout::{Alignment, Rect};
use tome_tui::prelude::*;
use tome_tui::style::{Modifier, Style};
use tome_tui::widgets::Paragraph;
use tome_tui::widgets::block::Padding;
use tome_tui::widgets::paragraph::Wrap;

const ICON_INFO: &str = "󰋼";
const ICON_WARN: &str = "󰀪";
const ICON_ERROR: &str = "󰅚";
const ICON_DEBUG: &str = "󰃭";
const ICON_TRACE: &str = "󰗋";

pub const ICON_CELL_WIDTH: u16 = 2;
pub const GUTTER_LEFT_PAD: u16 = 0;
pub const GUTTER_RIGHT_PAD: u16 = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GutterLayout {
	pub left_pad: u16,
	pub icon_width: u16,
	pub right_pad: u16,
}

impl Default for GutterLayout {
	fn default() -> Self {
		Self {
			left_pad: GUTTER_LEFT_PAD,
			icon_width: ICON_CELL_WIDTH,
			right_pad: GUTTER_RIGHT_PAD,
		}
	}
}

impl GutterLayout {
	pub fn total_width(self) -> u16 {
		self.left_pad + self.icon_width + self.right_pad
	}
}

pub fn get_level_icon(level: Option<Level>) -> Option<&'static str> {
	match level {
		Some(Level::Info) => Some(ICON_INFO),
		Some(Level::Warn) => Some(ICON_WARN),
		Some(Level::Error) => Some(ICON_ERROR),
		Some(Level::Debug) => Some(ICON_DEBUG),
		Some(Level::Trace) => Some(ICON_TRACE),
		None => None,
	}
}

/// Resolve final styles for notification rendering.
/// The theme always provides styles via block_style; border and title derive from it.
pub fn resolve_styles(
	_level: Option<Level>,
	block_style: Option<Style>,
	border_style: Option<Style>,
	title_style: Option<Style>,
) -> (Style, Style, Style) {
	// Block style from theme (always provided by Editor::notify)
	let final_block_style = block_style.unwrap_or_default();

	// Border style: use explicit override, or derive fg from block_style
	let final_border_style = border_style
		.unwrap_or_else(|| Style::default().fg(final_block_style.fg.unwrap_or_default()));

	// Title style: use explicit override, or derive from border + bold
	let final_title_style =
		title_style.unwrap_or_else(|| final_border_style.add_modifier(Modifier::BOLD));

	(final_block_style, final_border_style, final_title_style)
}

pub fn gutter_layout(level: Option<Level>) -> Option<GutterLayout> {
	get_level_icon(level).map(|_| GutterLayout::default())
}

pub fn padding_with_gutter(padding: Padding, gutter: Option<GutterLayout>) -> Padding {
	match gutter {
		Some(g) => Padding {
			left: padding.left.saturating_add(g.total_width()),
			..padding
		},
		None => padding,
	}
}

pub fn split_inner(inner: Rect, gutter: GutterLayout) -> (Rect, Rect) {
	let gutter_width = gutter.total_width().min(inner.width);
	let gutter_rect = Rect {
		x: inner.x,
		y: inner.y,
		width: gutter_width,
		height: inner.height,
	};
	let content_rect = Rect {
		x: inner.x.saturating_add(gutter_width),
		y: inner.y,
		width: inner.width.saturating_sub(gutter_width),
		height: inner.height,
	};
	(gutter_rect, content_rect)
}

pub fn render_icon_gutter(
	frame: &mut Frame<'_>,
	area: Rect,
	gutter: GutterLayout,
	icon: &str,
	style: Style,
) {
	if area.width == 0 || area.height == 0 {
		return;
	}

	let mut icon_area = Rect {
		x: area.x,
		y: area.y,
		width: area.width,
		height: 1,
	};
	icon_area.x = icon_area.x.saturating_add(gutter.left_pad);
	icon_area.width = icon_area.width.saturating_sub(gutter.left_pad);

	let paragraph = Paragraph::new(icon).style(style).alignment(Alignment::Left);
	frame.render_widget(paragraph, icon_area);
}

pub fn render_body(frame: &mut Frame<'_>, area: Rect, content: Text<'static>, style: Style) {
	if area.width == 0 || area.height == 0 {
		return;
	}

	let paragraph = Paragraph::new(content)
		.wrap(Wrap { trim: true })
		.style(style);
	frame.render_widget(paragraph, area);
}
