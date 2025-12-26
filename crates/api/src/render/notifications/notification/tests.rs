//! Tests for notification rendering module.

use std::time::Duration;

use tome_base::{BorderKind, Padding};
use tome_manifest::notifications::{
	Anchor, Animation, AutoDismiss, Level, SizeConstraint, SlideDirection, Timing,
};
use tome_stdlib::notifications::{Notification, NotificationBuilder};

#[test]
fn test_default_notification_has_sensible_defaults() {
	let notification = Notification::default();
	assert_eq!(notification.anchor, Anchor::BottomRight);
	assert_eq!(notification.animation, Animation::Fade);
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
	assert_eq!(notification.border_kind, BorderKind::Padded);
	assert_eq!(notification.padding, Padding::horizontal(1));
	assert_eq!(notification.content, "");
}

#[test]
fn test_builder_sets_title_correctly() {
	let notification = NotificationBuilder::new("Content")
		.title("My Title")
		.build()
		.unwrap();

	assert!(notification.title.is_some());
	assert_eq!(notification.title.unwrap(), "My Title");
}

#[test]
fn test_builder_sets_content_correctly() {
	let notification = NotificationBuilder::new("Hello World").build().unwrap();

	assert_eq!(notification.content, "Hello World");
}

#[test]
fn test_builder_sets_level_correctly() {
	let notification = NotificationBuilder::new("content")
		.level(Level::Error)
		.build()
		.unwrap();

	assert_eq!(notification.level, Some(Level::Error));
}

#[test]
fn test_builder_sets_anchor_correctly() {
	let notification = NotificationBuilder::new("content")
		.anchor(Anchor::TopLeft)
		.build()
		.unwrap();

	assert_eq!(notification.anchor, Anchor::TopLeft);
}

#[test]
fn test_builder_sets_animation_correctly() {
	let notification = NotificationBuilder::new("content")
		.animation(Animation::Slide)
		.build()
		.unwrap();

	assert_eq!(notification.animation, Animation::Slide);
}

#[test]
fn test_builder_sets_slide_direction_correctly() {
	let notification = NotificationBuilder::new("content")
		.slide_direction(SlideDirection::FromTop)
		.build()
		.unwrap();

	assert_eq!(notification.slide_direction, SlideDirection::FromTop);
}

#[test]
fn test_builder_sets_timing_correctly() {
	let notification = NotificationBuilder::new("content")
		.timing(
			Timing::Fixed(Duration::from_millis(100)),
			Timing::Fixed(Duration::from_millis(200)),
			Timing::Fixed(Duration::from_millis(300)),
		)
		.build()
		.unwrap();

	assert_eq!(
		notification.slide_in_timing,
		Timing::Fixed(Duration::from_millis(100))
	);
	assert_eq!(
		notification.dwell_timing,
		Timing::Fixed(Duration::from_millis(200))
	);
	assert_eq!(
		notification.slide_out_timing,
		Timing::Fixed(Duration::from_millis(300))
	);
}

#[test]
fn test_builder_sets_auto_dismiss_correctly() {
	let notification = NotificationBuilder::new("content")
		.auto_dismiss(AutoDismiss::Never)
		.build()
		.unwrap();

	assert_eq!(notification.auto_dismiss, AutoDismiss::Never);
}

#[test]
fn test_builder_sets_max_size_correctly() {
	let notification = NotificationBuilder::new("content")
		.max_size(SizeConstraint::Absolute(100), SizeConstraint::Absolute(50))
		.build()
		.unwrap();

	assert_eq!(notification.max_width, Some(SizeConstraint::Absolute(100)));
	assert_eq!(notification.max_height, Some(SizeConstraint::Absolute(50)));
}

#[test]
fn test_builder_sets_padding_correctly() {
	let notification = NotificationBuilder::new("content")
		.padding(Padding::uniform(5))
		.build()
		.unwrap();

	assert_eq!(notification.padding, Padding::uniform(5));
}

#[test]
fn test_builder_sets_margin_correctly() {
	let notification = NotificationBuilder::new("content")
		.margin(10)
		.build()
		.unwrap();

	assert_eq!(notification.exterior_margin, 10);
}

#[test]
fn test_builder_sets_border_kind_correctly() {
	let notification = NotificationBuilder::new("content")
		.border_kind(BorderKind::Rounded)
		.build()
		.unwrap();

	assert_eq!(notification.border_kind, BorderKind::Rounded);
}

#[test]
fn test_builder_sets_fade_correctly() {
	let notification = NotificationBuilder::new("content")
		.fade(true)
		.build()
		.unwrap();

	assert!(notification.fade_effect);
}

#[test]
fn test_builder_from_registry_applies_type_defaults() {
	// Note: This test relies on the "info" notification type being registered
	let notification = NotificationBuilder::from_registry("info", "content")
		.build()
		.unwrap();

	// Should have Info level from the registry
	assert_eq!(notification.level, Some(Level::Info));
}

#[test]
fn test_notification_validate_success() {
	let notification = Notification {
		content: "Short content".to_string(),
		..Default::default()
	};
	assert!(notification.validate().is_ok());
}

#[test]
fn test_notification_validate_too_long() {
	let long_content = "x".repeat(2000);
	let notification = Notification {
		content: long_content,
		..Default::default()
	};
	assert!(notification.validate().is_err());
}

#[test]
fn test_notification_clone() {
	let notification = NotificationBuilder::new("test")
		.title("Title")
		.level(Level::Warn)
		.anchor(Anchor::TopCenter)
		.build()
		.unwrap();

	let cloned = notification.clone();

	assert_eq!(notification.content, cloned.content);
	assert_eq!(notification.title, cloned.title);
	assert_eq!(notification.level, cloned.level);
	assert_eq!(notification.anchor, cloned.anchor);
}
