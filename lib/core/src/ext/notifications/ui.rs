use ratatui::layout::{Alignment, Rect};
use ratatui::prelude::*;
use ratatui::style::{Color, Modifier, Style};
use ratatui::widgets::Paragraph;
use ratatui::widgets::block::Padding;
use ratatui::widgets::paragraph::Wrap;

use crate::ext::notifications::types::Level;

const ICON_INFO: &str = "󰋼";
const ICON_WARN: &str = "󰀪";
const ICON_ERROR: &str = "󰅚";
const ICON_DEBUG: &str = "󰃭";
const ICON_TRACE: &str = "󰗋";

const DEFAULT_BLOCK_STYLE: Style = Style::new();
const DEFAULT_TITLE_STYLE: Style = Style::new().add_modifier(Modifier::BOLD);
const DEFAULT_BORDER_STYLE: Style = Style::new().fg(Color::DarkGray);

const INFO_BORDER_STYLE: Style = Style::new().fg(Color::Green);
const WARN_BORDER_STYLE: Style = Style::new().fg(Color::Yellow);
const ERROR_BORDER_STYLE: Style = Style::new().fg(Color::Red);
const DEBUG_BORDER_STYLE: Style = Style::new().fg(Color::Blue);
const TRACE_BORDER_STYLE: Style = Style::new().fg(Color::Magenta);

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

pub fn resolve_styles(
	level: Option<Level>,
	block_style: Option<Style>,
	border_style: Option<Style>,
	title_style: Option<Style>,
) -> (Style, Style, Style) {
	let mut final_block_style = DEFAULT_BLOCK_STYLE;
	let mut final_border_style = DEFAULT_BORDER_STYLE;
	let mut final_title_style = DEFAULT_TITLE_STYLE;

	if let Some(lvl) = level {
		let level_border_style = match lvl {
			Level::Info => INFO_BORDER_STYLE,
			Level::Warn => WARN_BORDER_STYLE,
			Level::Error => ERROR_BORDER_STYLE,
			Level::Debug => DEBUG_BORDER_STYLE,
			Level::Trace => TRACE_BORDER_STYLE,
		};
		final_border_style = level_border_style;
		final_title_style = final_title_style.patch(level_border_style);
	}

	if let Some(bs) = block_style {
		final_block_style = bs;
	}

	if let Some(bs) = border_style {
		final_border_style = bs;
		final_title_style = final_title_style.patch(bs);
	}

	if let Some(ts) = title_style {
		final_title_style = ts;
	}

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
