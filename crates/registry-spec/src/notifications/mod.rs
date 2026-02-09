#[cfg(feature = "compile")]
pub mod compile;

use serde::{Deserialize, Serialize};

use crate::MetaCommonSpec;

pub const VALID_LEVELS: &[&str] = &["info", "warn", "error", "debug", "success"];
pub const VALID_DISMISS: &[&str] = &["never", "after"];

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NotificationSpec {
	pub common: MetaCommonSpec,
	pub level: String,
	pub auto_dismiss: String,
	pub dismiss_ms: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NotificationsSpec {
	pub notifications: Vec<NotificationSpec>,
}
