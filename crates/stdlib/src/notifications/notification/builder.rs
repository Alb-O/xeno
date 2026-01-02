//! Builder pattern for constructing notifications.

use evildoer_base::{BorderKind, Padding, Position, Style};
use evildoer_registry::notifications::{
	find_notification_type, Anchor, Animation, AutoDismiss, Level, NotificationError,
	SizeConstraint, SlideDirection, Timing,
};

use super::{Notification, MAX_CONTENT_CHARS};

/// Builder for constructing `Notification` instances.
#[derive(Debug, Clone)]
pub struct NotificationBuilder {
	notification: Notification,
}

impl NotificationBuilder {
	/// Creates a new builder with the given content.
	pub fn new(content: impl Into<String>) -> Self {
		Self {
			notification: Notification {
				content: content.into(),
				..Default::default()
			},
		}
	}

	/// Creates a builder pre-configured from a registered notification type.
	///
	/// Looks up the notification type by name in the registry and applies
	/// its default settings (level, animation, timing, auto_dismiss).
	/// The style must be set separately by the caller since it requires theme access.
	pub fn from_registry(name: &str, content: impl Into<String>) -> Self {
		let mut builder = Self::new(content);
		if let Some(t) = find_notification_type(name) {
			builder = builder
				.level(t.level)
				.auto_dismiss(t.auto_dismiss)
				.animation(t.animation)
				.timing(t.timing.0, t.timing.1, t.timing.2);
		}
		builder
	}

	/// Sets the notification title.
	pub fn title(mut self, title: impl Into<String>) -> Self {
		self.notification.title = Some(title.into());
		self
	}

	/// Sets the severity level.
	pub fn level(mut self, level: Level) -> Self {
		self.notification.level = Some(level);
		self
	}

	/// Sets the anchor position.
	pub fn anchor(mut self, anchor: Anchor) -> Self {
		self.notification.anchor = anchor;
		self
	}

	/// Sets the animation style.
	pub fn animation(mut self, animation: Animation) -> Self {
		self.notification.animation = animation;
		self
	}

	/// Sets the slide direction for slide animations.
	pub fn slide_direction(mut self, direction: SlideDirection) -> Self {
		self.notification.slide_direction = direction;
		self
	}

	/// Sets all three animation timing phases.
	pub fn timing(mut self, slide_in: Timing, dwell: Timing, slide_out: Timing) -> Self {
		self.notification.slide_in_timing = slide_in;
		self.notification.dwell_timing = dwell;
		self.notification.slide_out_timing = slide_out;
		self
	}

	/// Sets when to automatically dismiss the notification.
	pub fn auto_dismiss(mut self, auto_dismiss: AutoDismiss) -> Self {
		self.notification.auto_dismiss = auto_dismiss;
		self
	}

	/// Sets the maximum size constraints.
	pub fn max_size(mut self, width: SizeConstraint, height: SizeConstraint) -> Self {
		self.notification.max_width = Some(width);
		self.notification.max_height = Some(height);
		self
	}

	/// Sets the internal padding.
	pub fn padding(mut self, padding: Padding) -> Self {
		self.notification.padding = padding;
		self
	}

	/// Sets the external margin.
	pub fn margin(mut self, margin: u16) -> Self {
		self.notification.exterior_margin = margin;
		self
	}

	/// Sets the block/background style.
	pub fn style(mut self, style: Style) -> Self {
		self.notification.block_style = Some(style);
		self
	}

	/// Sets the border style.
	pub fn border_style(mut self, style: Style) -> Self {
		self.notification.border_style = Some(style);
		self
	}

	/// Sets the title style.
	pub fn title_style(mut self, style: Style) -> Self {
		self.notification.title_style = Some(style);
		self
	}

	/// Sets the border kind.
	pub fn border_kind(mut self, kind: BorderKind) -> Self {
		self.notification.border_kind = kind;
		self
	}

	/// Sets the custom entry animation position.
	pub fn entry_position(mut self, position: Position) -> Self {
		self.notification.custom_entry_position = Some(position);
		self
	}

	/// Sets the custom exit animation position.
	pub fn exit_position(mut self, position: Position) -> Self {
		self.notification.custom_exit_position = Some(position);
		self
	}

	/// Enables or disables the fade effect.
	pub fn fade(mut self, enable: bool) -> Self {
		self.notification.fade_effect = enable;
		self
	}

	/// Builds the notification, validating the configuration.
	pub fn build(self) -> Result<Notification, NotificationError> {
		let char_count = self.notification.content.chars().count();
		if char_count > MAX_CONTENT_CHARS {
			return Err(NotificationError::ContentTooLarge(
				char_count,
				MAX_CONTENT_CHARS,
			));
		}
		Ok(self.notification)
	}
}
