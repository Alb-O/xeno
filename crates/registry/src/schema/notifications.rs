//! Notification specification schema.
//!
//! Defines named notification payload metadata and levels.


use serde::{Deserialize, Serialize};

use super::meta::MetaCommonSpec;

pub const VALID_LEVELS: &[&str] = &["info", "warn", "error", "debug", "success"];
pub const VALID_DISMISS: &[&str] = &["never", "after"];

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NotificationSpec {
	pub common: MetaCommonSpec,
	pub level: String,
	pub auto_dismiss: String,
	#[serde(default)]
	pub dismiss_ms: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NotificationsSpec {
	#[serde(default)]
	pub notifications: Vec<NotificationSpec>,
}
