use regex::Regex;
use xeno_primitives::Selection;
use xeno_primitives::range::CharIdx;
use xeno_tui::layout::Rect;
use xeno_tui::widgets::BorderType;
use xeno_tui::widgets::block::Padding;

use crate::buffer::ViewId;
use crate::window::{FloatingStyle, WindowId};

#[derive(Debug, Clone)]
pub struct Prompt {
	pub window_id: WindowId,
	pub buffer_id: ViewId,
	pub kind: PromptKind,
}

#[derive(Debug, Clone)]
pub enum PromptKind {
	Rename {
		target_buffer: ViewId,
		position: CharIdx,
	},
	Search {
		target_buffer: ViewId,
		reverse: bool,
	},
}

/// Runtime state for active search prompts.
#[derive(Debug)]
pub struct SearchPromptRuntime {
	/// Cursor position where search was initiated.
	pub origin_cursor: CharIdx,
	/// Selection state where search was initiated.
	pub origin_selection: Selection,
	/// Last processed input text to avoid redundant updates.
	pub last_input: String,
	/// Last match range shown in the preview.
	pub last_preview: Option<xeno_primitives::range::Range>,
	/// Cached regex pattern and compiled object.
	pub cached: Option<(String, Regex)>,
	/// Last regex compilation error message.
	pub last_error: Option<String>,
}

#[derive(Debug, Default)]
pub enum PromptState {
	#[default]
	Closed,
	Open {
		prompt: Prompt,
		search: Option<SearchPromptRuntime>,
	},
}

impl PromptState {
	pub fn is_open(&self) -> bool {
		matches!(self, Self::Open { .. })
	}

	pub fn active(&self) -> Option<&Prompt> {
		match self {
			Self::Open { prompt, .. } => Some(prompt),
			Self::Closed => None,
		}
	}

	pub fn active_mut(&mut self) -> Option<(&mut Prompt, Option<&mut SearchPromptRuntime>)> {
		match self {
			Self::Open { prompt, search } => Some((prompt, search.as_mut())),
			Self::Closed => None,
		}
	}

	pub fn take_open(&mut self) -> Option<(Prompt, Option<SearchPromptRuntime>)> {
		match std::mem::replace(self, PromptState::Closed) {
			PromptState::Open { prompt, search } => Some((prompt, search)),
			PromptState::Closed => None,
		}
	}

	pub fn window_id(&self) -> Option<WindowId> {
		self.active().map(|prompt| prompt.window_id)
	}

	pub fn buffer_id(&self) -> Option<ViewId> {
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
