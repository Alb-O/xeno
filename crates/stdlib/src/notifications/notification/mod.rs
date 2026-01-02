//! Notification data model.
//!
//! This module defines the abstract `Notification` struct that represents
//! notification content and configuration. It uses abstract types from
//! `evildoer_base` and `evildoer_registry` to avoid UI library dependencies.
//!
//! Actual rendering is handled by `evildoer_api`.

use evildoer_base::{BorderKind, Padding, Position, Style};
use evildoer_registry::notifications::{
	Anchor, Animation, AutoDismiss, Level, NotificationError, SizeConstraint, SlideDirection,
	Timing,
};

mod builder;

pub use builder::NotificationBuilder;

/// Maximum allowed characters in notification content.
pub const MAX_CONTENT_CHARS: usize = 1000;

/// A notification to be displayed to the user.
///
/// This is the abstract data model for notifications. It contains all the
/// configuration needed to display a notification, but no UI-specific types.
/// The rendering layer converts this to UI-specific state.
#[derive(Debug, Clone)]
pub struct Notification {
	/// The main text content of the notification.
	pub content: String,
	/// Optional title displayed at the top.
	pub title: Option<String>,
	/// Severity level (info, warn, error, etc.).
	pub level: Option<Level>,
	/// Screen position where the notification appears.
	pub anchor: Anchor,
	/// Animation style for entry/exit.
	pub animation: Animation,
	/// Direction for slide animations.
	pub slide_direction: SlideDirection,
	/// Duration of the entry animation.
	pub slide_in_timing: Timing,
	/// Duration of the dwell phase.
	pub dwell_timing: Timing,
	/// Duration of the exit animation.
	pub slide_out_timing: Timing,
	/// When to automatically dismiss.
	pub auto_dismiss: AutoDismiss,
	/// Maximum width constraint.
	pub max_width: Option<SizeConstraint>,
	/// Maximum height constraint.
	pub max_height: Option<SizeConstraint>,
	/// Internal padding around content.
	pub padding: Padding,
	/// External margin around the notification box.
	pub exterior_margin: u16,
	/// Style for the notification background.
	pub block_style: Option<Style>,
	/// Style for the border.
	pub border_style: Option<Style>,
	/// Style for the title text.
	pub title_style: Option<Style>,
	/// Border type/style.
	pub border_kind: BorderKind,
	/// Custom entry animation start position.
	pub custom_entry_position: Option<Position>,
	/// Custom exit animation end position.
	pub custom_exit_position: Option<Position>,
	/// Whether to apply fade effect during animations.
	pub fade_effect: bool,
}

impl Default for Notification {
	fn default() -> Self {
		Self {
			content: String::new(),
			title: None,
			level: Some(Level::Info),
			anchor: Anchor::default(),
			animation: Animation::default(),
			slide_direction: SlideDirection::default(),
			slide_in_timing: Timing::default(),
			dwell_timing: Timing::default(),
			slide_out_timing: Timing::default(),
			auto_dismiss: AutoDismiss::default(),
			max_width: Some(SizeConstraint::Percentage(0.4)),
			max_height: Some(SizeConstraint::Percentage(0.4)),
			padding: Padding::horizontal(1),
			exterior_margin: 0,
			block_style: None,
			border_style: None,
			title_style: None,
			border_kind: BorderKind::Padded,
			custom_entry_position: None,
			custom_exit_position: None,
			fade_effect: false,
		}
	}
}

impl Notification {
	/// Creates a new notification builder with the given content.
	pub fn builder(content: impl Into<String>) -> NotificationBuilder {
		NotificationBuilder::new(content)
	}

	/// Validates the notification configuration.
	pub fn validate(&self) -> Result<(), NotificationError> {
		let char_count = self.content.chars().count();
		if char_count > MAX_CONTENT_CHARS {
			return Err(NotificationError::ContentTooLarge(
				char_count,
				MAX_CONTENT_CHARS,
			));
		}
		Ok(())
	}
}
