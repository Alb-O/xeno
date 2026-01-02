use std::time::Duration;

use crate::{Animation, AutoDismiss, Level, NotificationTypeDef, RegistrySource, Timing};

#[::linkme::distributed_slice(crate::NOTIFICATION_TYPES)]
pub static INFO: NotificationTypeDef = NotificationTypeDef {
	id: "info",
	name: "info",
	level: Level::Info,
	icon: None,
	semantic: "info",
	auto_dismiss: AutoDismiss::After(Duration::from_secs(4)),
	animation: Animation::Fade,
	timing: (
		Timing::Fixed(Duration::from_millis(200)),
		Timing::Auto,
		Timing::Fixed(Duration::from_millis(200)),
	),
	priority: 0,
	source: RegistrySource::Crate(env!("CARGO_PKG_NAME")),
};

#[::linkme::distributed_slice(crate::NOTIFICATION_TYPES)]
pub static WARN: NotificationTypeDef = NotificationTypeDef {
	id: "warn",
	name: "warn",
	level: Level::Warn,
	icon: None,
	semantic: "warning",
	auto_dismiss: AutoDismiss::After(Duration::from_secs(6)),
	animation: Animation::Fade,
	timing: (
		Timing::Fixed(Duration::from_millis(200)),
		Timing::Auto,
		Timing::Fixed(Duration::from_millis(200)),
	),
	priority: 0,
	source: RegistrySource::Crate(env!("CARGO_PKG_NAME")),
};

#[::linkme::distributed_slice(crate::NOTIFICATION_TYPES)]
pub static ERROR: NotificationTypeDef = NotificationTypeDef {
	id: "error",
	name: "error",
	level: Level::Error,
	icon: None,
	semantic: "error",
	auto_dismiss: AutoDismiss::After(Duration::from_secs(8)),
	animation: Animation::Fade,
	timing: (
		Timing::Fixed(Duration::from_millis(200)),
		Timing::Auto,
		Timing::Fixed(Duration::from_millis(200)),
	),
	priority: 0,
	source: RegistrySource::Crate(env!("CARGO_PKG_NAME")),
};

#[::linkme::distributed_slice(crate::NOTIFICATION_TYPES)]
pub static SUCCESS: NotificationTypeDef = NotificationTypeDef {
	id: "success",
	name: "success",
	level: Level::Info,
	icon: None,
	semantic: "success",
	auto_dismiss: AutoDismiss::After(Duration::from_secs(3)),
	animation: Animation::Fade,
	timing: (
		Timing::Fixed(Duration::from_millis(200)),
		Timing::Auto,
		Timing::Fixed(Duration::from_millis(200)),
	),
	priority: 0,
	source: RegistrySource::Crate(env!("CARGO_PKG_NAME")),
};

#[::linkme::distributed_slice(crate::NOTIFICATION_TYPES)]
pub static DEBUG: NotificationTypeDef = NotificationTypeDef {
	id: "debug",
	name: "debug",
	level: Level::Debug,
	icon: None,
	semantic: "dim",
	auto_dismiss: AutoDismiss::After(Duration::from_secs(2)),
	animation: Animation::Fade,
	timing: (
		Timing::Fixed(Duration::from_millis(200)),
		Timing::Auto,
		Timing::Fixed(Duration::from_millis(200)),
	),
	priority: 0,
	source: RegistrySource::Crate(env!("CARGO_PKG_NAME")),
};
