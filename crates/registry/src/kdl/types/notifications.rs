use serde::{Deserialize, Serialize};

/// Raw notification metadata extracted from KDL.
#[derive(Debug, Serialize, Deserialize)]
pub struct NotificationMetaRaw {
	/// Common metadata.
	pub common: super::common::MetaCommonRaw,
	/// Severity level: "info", "warn", "error", "debug", "success".
	pub level: String,
	/// Auto-dismiss behavior: "never", "after".
	pub auto_dismiss: String,
	/// Dismiss duration in milliseconds (if auto_dismiss is "after").
	pub dismiss_ms: Option<u64>,
}

/// Top-level blob containing all notification metadata.
#[derive(Debug, Serialize, Deserialize)]
pub struct NotificationsBlob {
	/// All notification definitions.
	pub notifications: Vec<NotificationMetaRaw>,
}
