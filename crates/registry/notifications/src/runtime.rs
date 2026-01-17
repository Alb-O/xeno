//! Runtime notification keys (result handlers, input state).

use crate::{AutoDismiss, Level, Notification, NotificationDef, NotificationKey, RegistrySource};

static NOTIF_VIEWPORT_UNAVAILABLE: NotificationDef = NotificationDef::new(
	"viewport_unavailable",
	Level::Error,
	AutoDismiss::DEFAULT,
	RegistrySource::Builtin,
);

static NOTIF_SCREEN_MOTION_UNAVAILABLE: NotificationDef = NotificationDef::new(
	"screen_motion_unavailable",
	Level::Error,
	AutoDismiss::DEFAULT,
	RegistrySource::Builtin,
);

static NOTIF_PENDING_PROMPT: NotificationDef = NotificationDef::new(
	"pending_prompt",
	Level::Info,
	AutoDismiss::DEFAULT,
	RegistrySource::Builtin,
);

static NOTIF_COUNT_DISPLAY: NotificationDef = NotificationDef::new(
	"count_display",
	Level::Info,
	AutoDismiss::DEFAULT,
	RegistrySource::Builtin,
);

static NOTIF_UNHANDLED_RESULT: NotificationDef = NotificationDef::new(
	"unhandled_result",
	Level::Debug,
	AutoDismiss::DEFAULT,
	RegistrySource::Builtin,
);

#[allow(non_upper_case_globals, non_camel_case_types)]
pub mod keys {
	use super::*;

	pub const viewport_unavailable: NotificationKey = NotificationKey::new(
		&NOTIF_VIEWPORT_UNAVAILABLE,
		"Viewport info unavailable for screen motion",
	);
	pub const viewport_height_unavailable: NotificationKey = NotificationKey::new(
		&NOTIF_VIEWPORT_UNAVAILABLE,
		"Viewport height unavailable for screen motion",
	);
	pub const screen_motion_unavailable: NotificationKey = NotificationKey::new(
		&NOTIF_SCREEN_MOTION_UNAVAILABLE,
		"Screen motion target is unavailable",
	);

	/// Pending input prompt.
	pub struct pending_prompt;
	impl pending_prompt {
		pub fn call(prompt: &str) -> Notification {
			Notification::new(&NOTIF_PENDING_PROMPT, prompt.to_string())
		}
	}

	/// Numeric count display.
	pub struct count_display;
	impl count_display {
		pub fn call(count: usize) -> Notification {
			Notification::new(&NOTIF_COUNT_DISPLAY, count.to_string())
		}
	}

	/// Unhandled action result (debug).
	pub struct unhandled_result;
	impl unhandled_result {
		pub fn call(discriminant: impl core::fmt::Debug) -> Notification {
			Notification::new(
				&NOTIF_UNHANDLED_RESULT,
				format!("Unhandled action result: {:?}", discriminant),
			)
		}
	}
}

pub(crate) static NOTIFICATIONS: &[&NotificationDef] = &[
	&NOTIF_VIEWPORT_UNAVAILABLE,
	&NOTIF_SCREEN_MOTION_UNAVAILABLE,
	&NOTIF_PENDING_PROMPT,
	&NOTIF_COUNT_DISPLAY,
	&NOTIF_UNHANDLED_RESULT,
];
