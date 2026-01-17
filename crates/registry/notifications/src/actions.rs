//! Action notification keys.

use crate::{AutoDismiss, Level, Notification, NotificationDef, RegistrySource};

static NOTIF_UNKNOWN_ACTION: NotificationDef = NotificationDef::new(
	"unknown_action",
	Level::Error,
	AutoDismiss::DEFAULT,
	RegistrySource::Builtin,
);

static NOTIF_ACTION_ERROR: NotificationDef = NotificationDef::new(
	"action_error",
	Level::Error,
	AutoDismiss::DEFAULT,
	RegistrySource::Builtin,
);

#[allow(non_upper_case_globals, non_camel_case_types)]
pub mod keys {
	use super::*;

	/// "Unknown action: X".
	pub struct unknown_action;
	impl unknown_action {
		pub fn call(name: &str) -> Notification {
			Notification::new(&NOTIF_UNKNOWN_ACTION, format!("Unknown action: {}", name))
		}
	}

	/// Action execution error.
	pub struct action_error;
	impl action_error {
		pub fn call(err: impl core::fmt::Display) -> Notification {
			Notification::new(&NOTIF_ACTION_ERROR, err.to_string())
		}
	}
}

pub(crate) static NOTIFICATIONS: &[&NotificationDef] =
	&[&NOTIF_UNKNOWN_ACTION, &NOTIF_ACTION_ERROR];
