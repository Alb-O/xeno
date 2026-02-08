use serde::{Deserialize, Serialize};

pub use crate::defs::spec::MetaCommonSpec;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NotificationsSpec {
	pub notifications: Vec<NotificationSpec>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NotificationSpec {
	pub common: MetaCommonSpec,
	pub level: String,
	pub auto_dismiss: String,
	pub dismiss_ms: Option<u64>,
}
