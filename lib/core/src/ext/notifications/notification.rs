use std::time::Duration;

use ratatui::buffer::Buffer;
use ratatui::prelude::*;
use ratatui::widgets::block::Padding;
use ratatui::widgets::paragraph::Wrap;
use ratatui::widgets::{Block, BorderType, Borders, Paragraph};

use crate::ext::notifications::types::{
	Anchor, Animation, AutoDismiss, Level, NotificationError, SizeConstraint, SlideDirection,
	Timing,
};
use crate::ext::notifications::ui::{gutter_layout, padding_with_gutter};

/// Maximum allowed characters in notification content.
const MAX_CONTENT_CHARS: usize = 1000;

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

pub fn calculate_size(notification: &Notification, frame_area: Rect) -> (u16, u16) {
	let border_type = notification.border_type.unwrap_or(BorderType::Plain);
	let border_v_offset: u16 = 2;
	let border_h_offset: u16 = 2;

	let gutter = gutter_layout(notification.level);
	let effective_padding = padding_with_gutter(notification.padding, gutter);

	let h_padding = effective_padding.left + effective_padding.right;
	let v_padding = effective_padding.top + effective_padding.bottom;

	let min_width = (1 + h_padding + border_h_offset).max(3);
	let min_height = (1 + v_padding + border_v_offset).max(3);

	let max_width_constraint = notification
		.max_width
		.map(|c| match c {
			SizeConstraint::Absolute(w) => w.min(frame_area.width),
			SizeConstraint::Percentage(p) => {
				((frame_area.width as f32 * p.clamp(0.0, 1.0)).ceil() as u16).max(1)
			}
		})
		.unwrap_or(frame_area.width)
		.max(min_width);

	let max_height_constraint = notification
		.max_height
		.map(|c| match c {
			SizeConstraint::Absolute(h) => h.min(frame_area.height),
			SizeConstraint::Percentage(p) => {
				((frame_area.height as f32 * p.clamp(0.0, 1.0)).ceil() as u16).max(1)
			}
		})
		.unwrap_or(frame_area.height)
		.max(min_height);

	let content_max_line_width = notification
		.content
		.lines
		.iter()
		.map(|l: &Line| l.width())
		.max()
		.unwrap_or(0) as u16;

	let title_width = notification.title.as_ref().map_or(0, |t: &Line| t.width()) as u16;
	let title_padding = notification.padding.left + notification.padding.right;

	let width_for_body = (content_max_line_width + border_h_offset + h_padding).max(min_width);
	let width_for_title = (title_width + border_h_offset + title_padding).max(min_width);

	let intrinsic_width = width_for_body.max(width_for_title);
	let final_width = intrinsic_width.min(max_width_constraint);

	let mut temp_block = Block::default()
		.borders(Borders::ALL)
		.border_type(border_type)
		.padding(effective_padding);
	if let Some(title) = &notification.title {
		temp_block = temp_block.title(title.clone());
	}

	let buffer_height = max_height_constraint;
	let mut buffer = Buffer::empty(Rect::new(0, 0, final_width, buffer_height));

	let paragraph = Paragraph::new(notification.content.clone())
		.wrap(Wrap { trim: true })
		.block(temp_block.clone());
	paragraph.render(buffer.area, &mut buffer);

	let text_area = temp_block.inner(buffer.area);
	let used_text_height = measure_used_text_height(&buffer, text_area).max(1);

	let needed_height = used_text_height
		.saturating_add(border_v_offset)
		.saturating_add(v_padding);

	let final_height = needed_height.max(min_height).min(max_height_constraint);
	(final_width, final_height)
}

fn measure_used_text_height(buffer: &Buffer, text_area: Rect) -> u16 {
	if text_area.width == 0 || text_area.height == 0 {
		return 0;
	}

	let mut last_used_y: Option<u16> = None;
	for row in 0..text_area.height {
		let y = text_area.y.saturating_add(row);
		let mut row_has_glyph = false;

		for col in 0..text_area.width {
			let x = text_area.x.saturating_add(col);
			let sym = buffer[(x, y)].symbol();
			if !sym.is_empty() && sym != " " {
				row_has_glyph = true;
				break;
			}
		}

		if row_has_glyph {
			last_used_y = Some(y);
		}
	}

	match last_used_y {
		Some(y) => y.saturating_sub(text_area.y).saturating_add(1),
		None => 0,
	}
}

pub fn generate_code(notification: &Notification) -> String {
	let defaults = Notification::default();
	let mut lines = Vec::new();

	let content_str = escape_string(&notification.content().to_string());
	lines.push(format!("Notification::builder(\"{}\")", content_str));

	if let Some(title) = notification.title() {
		let title_str = escape_string(&title.to_string());
		lines.push(format!("    .title(\"{}\")", title_str));
	}

	if notification.level() != defaults.level
		&& let Some(level) = notification.level()
	{
		lines.push(format!("    .level(Level::{:?})", level));
	}

	if notification.anchor() != defaults.anchor {
		lines.push(format!("    .anchor(Anchor::{:?})", notification.anchor()));
	}

	if notification.animation() != defaults.animation {
		lines.push(format!(
			"    .animation(Animation::{:?})",
			notification.animation()
		));
	}

	if notification.slide_direction() != defaults.slide_direction {
		lines.push(format!(
			"    .slide_direction(SlideDirection::{:?})",
			notification.slide_direction()
		));
	}

	let timing_changed = notification.slide_in_timing() != defaults.slide_in_timing
		|| notification.dwell_timing() != defaults.dwell_timing
		|| notification.slide_out_timing() != defaults.slide_out_timing;

	if timing_changed {
		let slide_in = format_timing(notification.slide_in_timing());
		let dwell = format_timing(notification.dwell_timing());
		let slide_out = format_timing(notification.slide_out_timing());
		lines.push(format!(
			"    .timing({}, {}, {})",
			slide_in, dwell, slide_out
		));
	}

	if notification.auto_dismiss() != defaults.auto_dismiss {
		lines.push(format!(
			"    .auto_dismiss({})",
			format_auto_dismiss(notification.auto_dismiss())
		));
	}

	let size_changed = notification.max_width() != defaults.max_width
		|| notification.max_height() != defaults.max_height;

	if size_changed
		&& let (Some(w), Some(h)) = (notification.max_width(), notification.max_height())
	{
		lines.push(format!(
			"    .max_size({}, {})",
			format_size_constraint(w),
			format_size_constraint(h)
		));
	}

	if notification.padding() != defaults.padding {
		lines.push(format!(
			"    .padding({})",
			format_padding(notification.padding())
		));
	}

	if notification.exterior_margin() != defaults.exterior_margin {
		lines.push(format!("    .margin({})", notification.exterior_margin()));
	}

	if notification.border_type() != defaults.border_type
		&& let Some(bt) = notification.border_type()
	{
		lines.push(format!("    .border_type(BorderType::{:?})", bt));
	}

	if let Some(pos) = notification.custom_entry_position() {
		lines.push(format!(
			"    .entry_position(Position::new({}, {}))",
			pos.x, pos.y
		));
	}

	if let Some(pos) = notification.custom_exit_position() {
		lines.push(format!(
			"    .exit_position(Position::new({}, {}))",
			pos.x, pos.y
		));
	}

	if notification.fade_effect() != defaults.fade_effect {
		lines.push(format!("    .fade({})", notification.fade_effect()));
	}

	lines.push("    .build()".to_string());
	lines.join("\n")
}

fn escape_string(s: &str) -> String {
	s.replace('\\', "\\\\")
		.replace('"', "\\\"")
		.replace('\n', "\\n")
		.replace('\r', "\\r")
		.replace('\t', "\\t")
}

fn format_timing(timing: Timing) -> String {
	match timing {
		Timing::Auto => "Timing::Auto".to_string(),
		Timing::Fixed(d) => format_duration_as_timing(d),
	}
}

fn format_duration_as_timing(d: Duration) -> String {
	let millis = d.as_millis();
	if millis.is_multiple_of(1000) {
		format!("Timing::Fixed(Duration::from_secs({}))", millis / 1000)
	} else {
		format!("Timing::Fixed(Duration::from_millis({}))", millis)
	}
}

fn format_auto_dismiss(ad: AutoDismiss) -> String {
	match ad {
		AutoDismiss::Never => "AutoDismiss::Never".to_string(),
		AutoDismiss::After(d) => {
			let millis = d.as_millis();
			if millis % 1000 == 0 {
				format!("AutoDismiss::After(Duration::from_secs({}))", millis / 1000)
			} else {
				format!("AutoDismiss::After(Duration::from_millis({}))", millis)
			}
		}
	}
}

fn format_size_constraint(sc: SizeConstraint) -> String {
	match sc {
		SizeConstraint::Absolute(n) => format!("SizeConstraint::Absolute({})", n),
		SizeConstraint::Percentage(p) => format!("SizeConstraint::Percentage({})", p),
	}
}

fn format_padding(p: Padding) -> String {
	if p.top == p.bottom && p.left == p.right && p.top == p.left {
		format!("Padding::uniform({})", p.top)
	} else if p.top == p.bottom && p.left == p.right {
		format!("Padding::symmetric({}, {})", p.top, p.left)
	} else if p.top == 0 && p.bottom == 0 {
		format!("Padding::horizontal({})", p.left)
	} else if p.left == 0 && p.right == 0 {
		format!("Padding::vertical({})", p.top)
	} else {
		format!(
			"Padding::new({}, {}, {}, {})",
			p.top, p.right, p.bottom, p.left
		)
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::ext::notifications::types::{
		Anchor, Animation, AutoDismiss, Level, SizeConstraint, SlideDirection,
	};

	#[test]
	fn test_default_notification_has_sensible_defaults() {
		let notification = Notification::default();
		assert_eq!(notification.anchor, Anchor::BottomRight);
		assert_eq!(notification.animation, Animation::Slide);
		assert_eq!(notification.slide_direction, SlideDirection::Default);
		assert_eq!(
			notification.auto_dismiss,
			AutoDismiss::After(Duration::from_secs(4))
		);
		assert_eq!(notification.level, Some(Level::Info));
		assert_eq!(notification.title, None);
		assert!(!notification.fade_effect);
		assert_eq!(notification.exterior_margin, 0);
		assert_eq!(
			notification.max_width,
			Some(SizeConstraint::Percentage(0.4))
		);
		assert_eq!(
			notification.max_height,
			Some(SizeConstraint::Percentage(0.4))
		);
		assert_eq!(notification.border_type, Some(BorderType::Padded));
		assert_eq!(notification.padding, Padding::horizontal(1));
		assert_eq!(notification.content.to_string(), "");
	}

	#[test]
	fn test_builder_sets_title_correctly() {
		let notification = NotificationBuilder::new("Content")
			.title("My Title")
			.build()
			.unwrap();

		assert!(notification.title.is_some());
		assert_eq!(notification.title.unwrap().to_string(), "My Title");
	}

	#[test]
	fn test_builder_sets_level() {
		let notification = NotificationBuilder::new("Error message")
			.level(Level::Error)
			.build()
			.unwrap();

		assert_eq!(notification.level, Some(Level::Error));
	}

	#[test]
	fn test_builder_sets_anchor() {
		let notification = NotificationBuilder::new("Test")
			.anchor(Anchor::TopLeft)
			.build()
			.unwrap();

		assert_eq!(notification.anchor, Anchor::TopLeft);
	}

	#[test]
	fn test_builder_sets_animation() {
		let notification = NotificationBuilder::new("Test")
			.animation(Animation::Fade)
			.build()
			.unwrap();

		assert_eq!(notification.animation, Animation::Fade);
	}

	#[test]
	fn test_builder_sets_slide_direction() {
		let notification = NotificationBuilder::new("Test")
			.slide_direction(SlideDirection::FromTop)
			.build()
			.unwrap();

		assert_eq!(notification.slide_direction, SlideDirection::FromTop);
	}

	#[test]
	fn test_builder_sets_timings() {
		let slide_in = Timing::Fixed(Duration::from_millis(200));
		let dwell = Timing::Fixed(Duration::from_millis(3000));
		let slide_out = Timing::Fixed(Duration::from_millis(150));

		let notification = NotificationBuilder::new("Test")
			.timing(slide_in, dwell, slide_out)
			.build()
			.unwrap();

		assert_eq!(notification.slide_in_timing, slide_in);
		assert_eq!(notification.dwell_timing, dwell);
		assert_eq!(notification.slide_out_timing, slide_out);
	}

	#[test]
	fn test_builder_sets_auto_dismiss() {
		let notification = NotificationBuilder::new("Test")
			.auto_dismiss(AutoDismiss::Never)
			.build()
			.unwrap();

		assert_eq!(notification.auto_dismiss, AutoDismiss::Never);
	}

	#[test]
	fn test_builder_sets_max_size() {
		let notification = NotificationBuilder::new("Test")
			.max_size(
				SizeConstraint::Absolute(50),
				SizeConstraint::Percentage(0.3),
			)
			.build()
			.unwrap();

		assert_eq!(notification.max_width, Some(SizeConstraint::Absolute(50)));
		assert_eq!(
			notification.max_height,
			Some(SizeConstraint::Percentage(0.3))
		);
	}

	#[test]
	fn test_builder_sets_padding() {
		let padding = Padding::new(1, 2, 3, 4);

		let notification = NotificationBuilder::new("Test")
			.padding(padding)
			.build()
			.unwrap();

		assert_eq!(notification.padding, padding);
	}

	#[test]
	fn test_builder_sets_margin() {
		let notification = NotificationBuilder::new("Test").margin(5).build().unwrap();

		assert_eq!(notification.exterior_margin, 5);
	}

	#[test]
	fn test_builder_sets_block_style() {
		let style = Style::default().fg(Color::Red);

		let notification = NotificationBuilder::new("Test")
			.style(style)
			.build()
			.unwrap();

		assert_eq!(notification.block_style, Some(style));
	}

	#[test]
	fn test_builder_sets_border_style() {
		let style = Style::default().fg(Color::Blue);

		let notification = NotificationBuilder::new("Test")
			.border_style(style)
			.build()
			.unwrap();

		assert_eq!(notification.border_style, Some(style));
	}

	#[test]
	fn test_builder_sets_title_style() {
		let style = Style::default().fg(Color::Green);

		let notification = NotificationBuilder::new("Test")
			.title_style(style)
			.build()
			.unwrap();

		assert_eq!(notification.title_style, Some(style));
	}

	#[test]
	fn test_builder_sets_border_type() {
		let notification = NotificationBuilder::new("Test")
			.border_type(BorderType::Double)
			.build()
			.unwrap();

		assert_eq!(notification.border_type, Some(BorderType::Double));
	}

	#[test]
	fn test_builder_sets_custom_entry_position() {
		let pos = Position::new(10, 20);

		let notification = NotificationBuilder::new("Test")
			.entry_position(pos)
			.build()
			.unwrap();

		assert_eq!(notification.custom_entry_position, Some(pos));
	}

	#[test]
	fn test_builder_sets_custom_exit_position() {
		let pos = Position::new(5, 15);

		let notification = NotificationBuilder::new("Test")
			.exit_position(pos)
			.build()
			.unwrap();

		assert_eq!(notification.custom_exit_position, Some(pos));
	}

	#[test]
	fn test_builder_sets_fade_effect() {
		let notification = NotificationBuilder::new("Test").fade(true).build().unwrap();

		assert!(notification.fade_effect);
	}

	#[test]
	fn test_builder_builds_with_all_options() {
		let padding = Padding::uniform(2);
		let style = Style::default().fg(Color::Yellow);
		let border_style = Style::default().fg(Color::Cyan);
		let title_style = Style::default().fg(Color::Magenta);
		let entry_pos = Position::new(0, 0);
		let exit_pos = Position::new(100, 100);

		let notification = NotificationBuilder::new("Full config test")
			.title("Test Title")
			.level(Level::Warn)
			.anchor(Anchor::TopCenter)
			.animation(Animation::ExpandCollapse)
			.slide_direction(SlideDirection::FromBottom)
			.timing(
				Timing::Fixed(Duration::from_millis(100)),
				Timing::Fixed(Duration::from_millis(2000)),
				Timing::Fixed(Duration::from_millis(100)),
			)
			.auto_dismiss(AutoDismiss::After(Duration::from_secs(5)))
			.max_size(
				SizeConstraint::Percentage(0.5),
				SizeConstraint::Absolute(10),
			)
			.padding(padding)
			.margin(3)
			.style(style)
			.border_style(border_style)
			.title_style(title_style)
			.border_type(BorderType::Thick)
			.entry_position(entry_pos)
			.exit_position(exit_pos)
			.fade(true)
			.build()
			.unwrap();

		assert_eq!(notification.content.to_string(), "Full config test");
		assert_eq!(notification.title.unwrap().to_string(), "Test Title");
		assert_eq!(notification.level, Some(Level::Warn));
		assert_eq!(notification.anchor, Anchor::TopCenter);
		assert_eq!(notification.animation, Animation::ExpandCollapse);
		assert_eq!(notification.slide_direction, SlideDirection::FromBottom);
		assert_eq!(
			notification.auto_dismiss,
			AutoDismiss::After(Duration::from_secs(5))
		);
		assert_eq!(
			notification.max_width,
			Some(SizeConstraint::Percentage(0.5))
		);
		assert_eq!(notification.max_height, Some(SizeConstraint::Absolute(10)));
		assert_eq!(notification.padding, padding);
		assert_eq!(notification.exterior_margin, 3);
		assert_eq!(notification.block_style, Some(style));
		assert_eq!(notification.border_style, Some(border_style));
		assert_eq!(notification.title_style, Some(title_style));
		assert_eq!(notification.border_type, Some(BorderType::Thick));
		assert_eq!(notification.custom_entry_position, Some(entry_pos));
		assert_eq!(notification.custom_exit_position, Some(exit_pos));
		assert!(notification.fade_effect);
	}

	#[test]
	fn test_content_validation_accepts_valid_content() {
		let content = "a".repeat(999);
		let result = NotificationBuilder::new(Text::from(content)).build();
		assert!(result.is_ok());
	}

	#[test]
	fn test_content_validation_rejects_oversized_content() {
		let content = "a".repeat(1001);
		let result = NotificationBuilder::new(Text::from(content)).build();
		assert!(result.is_err());
	}

	#[test]
	fn test_content_validation_at_boundary() {
		let content = "a".repeat(1000);
		let result = NotificationBuilder::new(Text::from(content)).build();
		assert!(result.is_ok());
	}

	#[test]
	fn test_notification_implements_debug() {
		let notification = NotificationBuilder::new("Test")
			.title("Debug Test")
			.build()
			.unwrap();
		let debug_str = format!("{:?}", notification);
		assert!(debug_str.contains("Notification"));
	}

	#[test]
	fn test_notification_implements_clone() {
		let notification = NotificationBuilder::new("Original")
			.title("Clone Test")
			.level(Level::Info)
			.build()
			.unwrap();
		let cloned = notification.clone();
		assert_eq!(notification.content.to_string(), cloned.content.to_string());
		assert_eq!(
			notification.title.as_ref().map(|l| l.to_string()),
			cloned.title.as_ref().map(|l| l.to_string())
		);
		assert_eq!(notification.level, cloned.level);
	}

	#[test]
	fn test_multiline_content() {
		let content = "Line 1\nLine 2\nLine 3";
		let notification = NotificationBuilder::new(content).build().unwrap();
		assert_eq!(notification.content.to_string(), content);
	}

	#[test]
	fn test_empty_content() {
		let notification = NotificationBuilder::new("").build().unwrap();
		assert_eq!(notification.content.to_string(), "");
	}

	#[test]
	fn calculate_size_includes_gutter_width_when_icon_present() {
		let frame_area = Rect::new(0, 0, 120, 40);
		let mut without_icon = Notification::default();
		without_icon.level = None;
		without_icon.content = Text::raw("1234567890");
		without_icon.max_width = Some(SizeConstraint::Absolute(120));
		let mut with_icon = without_icon.clone();
		with_icon.level = Some(Level::Info);
		let (w0, _) = calculate_size(&without_icon, frame_area);
		let (w1, _) = calculate_size(&with_icon, frame_area);
		assert!(w1 > w0, "expected icon gutter to increase width");
	}

	#[test]
	fn calculate_size_wraps_content_with_gutter() {
		let frame_area = Rect::new(0, 0, 120, 40);
		let mut n = Notification::default();
		n.level = Some(Level::Info);
		n.content = Text::raw("abcde fghij klmno");
		n.max_width = Some(SizeConstraint::Absolute(12));
		n.max_height = Some(SizeConstraint::Absolute(40));
		let (_, h) = calculate_size(&n, frame_area);
		assert!(h > 3, "expected wrapping to increase height");
	}

	#[test]
	fn percentage_constraints_round_up() {
		let frame_area = Rect::new(0, 0, 80, 8);
		let mut n = Notification::default();
		n.level = Some(Level::Info);
		n.content = Text::raw("Buffer has unsaved changes (use :write)");
		let (_, h) = calculate_size(&n, frame_area);
		assert!(h >= 4, "expected room for >1 content line");
	}
}
