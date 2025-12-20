use std::time::Duration;

use ratatui::widgets::block::Padding;

use crate::ext::notifications::notification::Notification;
use crate::ext::notifications::types::{AutoDismiss, SizeConstraint, Timing};

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
			if millis.is_multiple_of(1000) {
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
