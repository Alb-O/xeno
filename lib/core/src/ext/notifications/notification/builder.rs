use ratatui::prelude::*;
use ratatui::widgets::BorderType;
use ratatui::widgets::block::Padding;

use crate::ext::notifications::notification::Notification;
use crate::ext::notifications::types::{
	Anchor, Animation, AutoDismiss, Level, NotificationError, SizeConstraint, SlideDirection,
	Timing,
};

/// Maximum allowed characters in notification content.
const MAX_CONTENT_CHARS: usize = 1000;

#[derive(Debug, Clone)]
pub struct NotificationBuilder {
	notification: Notification,
}

impl NotificationBuilder {
	pub fn new(content: impl Into<Text<'static>>) -> Self {
		Self {
			notification: Notification {
				content: content.into(),
				..Default::default()
			},
		}
	}

	pub fn title(mut self, title: impl Into<Line<'static>>) -> Self {
		self.notification.title = Some(title.into());
		self
	}

	pub fn level(mut self, level: Level) -> Self {
		self.notification.level = Some(level);
		self
	}

	pub fn anchor(mut self, anchor: Anchor) -> Self {
		self.notification.anchor = anchor;
		self
	}

	pub fn animation(mut self, animation: Animation) -> Self {
		self.notification.animation = animation;
		self
	}

	pub fn slide_direction(mut self, direction: SlideDirection) -> Self {
		self.notification.slide_direction = direction;
		self
	}

	pub fn timing(mut self, slide_in: Timing, dwell: Timing, slide_out: Timing) -> Self {
		self.notification.slide_in_timing = slide_in;
		self.notification.dwell_timing = dwell;
		self.notification.slide_out_timing = slide_out;
		self
	}

	pub fn auto_dismiss(mut self, auto_dismiss: AutoDismiss) -> Self {
		self.notification.auto_dismiss = auto_dismiss;
		self
	}

	pub fn max_size(mut self, width: SizeConstraint, height: SizeConstraint) -> Self {
		self.notification.max_width = Some(width);
		self.notification.max_height = Some(height);
		self
	}

	pub fn padding(mut self, padding: Padding) -> Self {
		self.notification.padding = padding;
		self
	}

	pub fn margin(mut self, margin: u16) -> Self {
		self.notification.exterior_margin = margin;
		self
	}

	pub fn style(mut self, style: Style) -> Self {
		self.notification.block_style = Some(style);
		self
	}

	pub fn border_style(mut self, style: Style) -> Self {
		self.notification.border_style = Some(style);
		self
	}

	pub fn title_style(mut self, style: Style) -> Self {
		self.notification.title_style = Some(style);
		self
	}

	pub fn border_type(mut self, border_type: BorderType) -> Self {
		self.notification.border_type = Some(border_type);
		self
	}

	pub fn entry_position(mut self, position: Position) -> Self {
		self.notification.custom_entry_position = Some(position);
		self
	}

	pub fn exit_position(mut self, position: Position) -> Self {
		self.notification.custom_exit_position = Some(position);
		self
	}

	pub fn fade(mut self, enable: bool) -> Self {
		self.notification.fade_effect = enable;
		self
	}

	pub fn build(self) -> Result<Notification, NotificationError> {
		let content_str = self.notification.content.to_string();
		let char_count = content_str.chars().count();

		if char_count > MAX_CONTENT_CHARS {
			return Err(NotificationError::ContentTooLarge(
				char_count,
				MAX_CONTENT_CHARS,
			));
		}

		Ok(self.notification)
	}
}
