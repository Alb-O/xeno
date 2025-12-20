use std::time::Duration;

use ratatui::prelude::*;
use ratatui::widgets::BorderType;
use ratatui::widgets::block::Padding;

use crate::ext::notifications::notification::{Notification, NotificationBuilder, calculate_size};
use crate::ext::notifications::types::{
	Anchor, Animation, AutoDismiss, Level, SizeConstraint, SlideDirection, Timing,
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
