use xeno_primitives::range::CharIdx;
use xeno_tui::layout::Rect;
use xeno_tui::widgets::BorderType;
use xeno_tui::widgets::block::Padding;

use crate::buffer::BufferId;
use crate::window::{FloatingStyle, WindowId};

#[derive(Debug, Clone)]
pub struct Prompt {
	pub window_id: WindowId,
	pub buffer_id: BufferId,
	pub kind: PromptKind,
}

#[derive(Debug, Clone)]
pub enum PromptKind {
	Rename {
		target_buffer: BufferId,
		position: CharIdx,
	},
}

#[derive(Debug, Default)]
pub enum PromptState {
	#[default]
	Closed,
	Open(Prompt),
}

impl PromptState {
	pub fn is_open(&self) -> bool {
		matches!(self, Self::Open(_))
	}

	pub fn active(&self) -> Option<&Prompt> {
		match self {
			Self::Open(prompt) => Some(prompt),
			Self::Closed => None,
		}
	}

	pub fn window_id(&self) -> Option<WindowId> {
		self.active().map(|prompt| prompt.window_id)
	}

	pub fn buffer_id(&self) -> Option<BufferId> {
		self.active().map(|prompt| prompt.buffer_id)
	}
}

pub fn prompt_style(title: &str) -> FloatingStyle {
	FloatingStyle {
		border: true,
		border_type: BorderType::Stripe,
		padding: Padding::horizontal(1),
		shadow: false,
		title: Some(title.to_string()),
	}
}

pub fn prompt_rect(screen_width: u16, screen_height: u16) -> Rect {
	let width = screen_width.saturating_sub(20).clamp(40, 80);
	let height = 3;
	let x = (screen_width.saturating_sub(width)) / 2;
	let y = screen_height / 5;

	Rect::new(x, y, width, height)
}
