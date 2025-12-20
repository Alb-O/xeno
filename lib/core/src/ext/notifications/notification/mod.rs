use ratatui::prelude::*;
use ratatui::widgets::BorderType;
use ratatui::widgets::block::Padding;

use crate::ext::notifications::types::{
	Anchor, Animation, AutoDismiss, Level, SizeConstraint, SlideDirection, Timing,
};

mod builder;
mod codegen;
mod layout;

#[cfg(test)]
mod tests;

pub use builder::NotificationBuilder;
pub use codegen::generate_code;
pub use layout::calculate_size;

#[derive(Debug, Clone)]
pub struct Notification {
	pub(crate) content: Text<'static>,
	pub(crate) title: Option<Line<'static>>,
	pub(crate) level: Option<Level>,
	pub(crate) anchor: Anchor,
	pub(crate) animation: Animation,
	pub(crate) slide_direction: SlideDirection,
	pub(crate) slide_in_timing: Timing,
	pub(crate) dwell_timing: Timing,
	pub(crate) slide_out_timing: Timing,
	pub(crate) auto_dismiss: AutoDismiss,
	pub(crate) max_width: Option<SizeConstraint>,
	pub(crate) max_height: Option<SizeConstraint>,
	pub(crate) padding: Padding,
	pub(crate) exterior_margin: u16,
	pub(crate) block_style: Option<Style>,
	pub(crate) border_style: Option<Style>,
	pub(crate) title_style: Option<Style>,
	pub(crate) border_type: Option<BorderType>,
	pub(crate) custom_entry_position: Option<Position>,
	pub(crate) custom_exit_position: Option<Position>,
	pub(crate) fade_effect: bool,
}

impl Default for Notification {
	fn default() -> Self {
		Self {
			content: Text::from(""),
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
			border_type: Some(BorderType::Padded),
			custom_entry_position: None,
			custom_exit_position: None,
			fade_effect: false,
		}
	}
}

impl Notification {
	pub fn builder(content: impl Into<Text<'static>>) -> NotificationBuilder {
		NotificationBuilder::new(content)
	}

	pub fn content(&self) -> &Text<'static> {
		&self.content
	}

	pub fn title(&self) -> Option<&Line<'static>> {
		self.title.as_ref()
	}

	pub fn level(&self) -> Option<Level> {
		self.level
	}

	pub fn anchor(&self) -> Anchor {
		self.anchor
	}

	pub fn animation(&self) -> Animation {
		self.animation
	}

	pub fn slide_direction(&self) -> SlideDirection {
		self.slide_direction
	}

	pub fn slide_in_timing(&self) -> Timing {
		self.slide_in_timing
	}

	pub fn dwell_timing(&self) -> Timing {
		self.dwell_timing
	}

	pub fn slide_out_timing(&self) -> Timing {
		self.slide_out_timing
	}

	pub fn auto_dismiss(&self) -> AutoDismiss {
		self.auto_dismiss
	}

	pub fn max_width(&self) -> Option<SizeConstraint> {
		self.max_width
	}

	pub fn max_height(&self) -> Option<SizeConstraint> {
		self.max_height
	}

	pub fn padding(&self) -> Padding {
		self.padding
	}

	pub fn exterior_margin(&self) -> u16 {
		self.exterior_margin
	}

	pub fn border_type(&self) -> Option<BorderType> {
		self.border_type
	}

	pub fn custom_entry_position(&self) -> Option<Position> {
		self.custom_entry_position
	}

	pub fn custom_exit_position(&self) -> Option<Position> {
		self.custom_exit_position
	}

	pub fn fade_effect(&self) -> bool {
		self.fade_effect
	}
}
