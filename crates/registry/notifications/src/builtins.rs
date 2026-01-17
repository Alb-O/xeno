//! Generic fallback notifications. Prefer domain-specific keys when available.

use crate::{AutoDismiss, Level, Notification, NotificationDef, RegistrySource};

static NOTIF_INFO: NotificationDef = NotificationDef::new(
	"info",
	Level::Info,
	AutoDismiss::DEFAULT,
	RegistrySource::Builtin,
);

static NOTIF_WARN: NotificationDef = NotificationDef::new(
	"warn",
	Level::Warn,
	AutoDismiss::DEFAULT,
	RegistrySource::Builtin,
);

static NOTIF_ERROR: NotificationDef = NotificationDef::new(
	"error",
	Level::Error,
	AutoDismiss::DEFAULT,
	RegistrySource::Builtin,
);

static NOTIF_SUCCESS: NotificationDef = NotificationDef::new(
	"success",
	Level::Success,
	AutoDismiss::DEFAULT,
	RegistrySource::Builtin,
);

static NOTIF_DEBUG: NotificationDef = NotificationDef::new(
	"debug",
	Level::Debug,
	AutoDismiss::DEFAULT,
	RegistrySource::Builtin,
);

#[allow(non_upper_case_globals, non_camel_case_types)]
pub mod keys {
	use super::*;

	/// Generic info notification. Prefer domain-specific keys.
	pub struct info;
	impl info {
		pub fn call(msg: impl Into<String>) -> Notification {
			Notification::new(&NOTIF_INFO, msg)
		}
	}

	/// Generic warning notification. Prefer domain-specific keys.
	pub struct warn;
	impl warn {
		pub fn call(msg: impl Into<String>) -> Notification {
			Notification::new(&NOTIF_WARN, msg)
		}
	}

	/// Generic error notification. Prefer domain-specific keys.
	pub struct error;
	impl error {
		pub fn call(msg: impl Into<String>) -> Notification {
			Notification::new(&NOTIF_ERROR, msg)
		}
	}

	/// Generic success notification. Prefer domain-specific keys.
	pub struct success;
	impl success {
		pub fn call(msg: impl Into<String>) -> Notification {
			Notification::new(&NOTIF_SUCCESS, msg)
		}
	}

	/// Generic debug notification. Prefer domain-specific keys.
	pub struct debug;
	impl debug {
		pub fn call(msg: impl Into<String>) -> Notification {
			Notification::new(&NOTIF_DEBUG, msg)
		}
	}
}

pub(crate) static NOTIFICATIONS: &[&NotificationDef] = &[
	&NOTIF_INFO,
	&NOTIF_WARN,
	&NOTIF_ERROR,
	&NOTIF_SUCCESS,
	&NOTIF_DEBUG,
];
