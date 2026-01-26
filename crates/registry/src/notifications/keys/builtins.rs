//! Generic fallback notifications. Prefer domain-specific keys when available.

pub mod keys {
	notif!(info(msg: impl Into<String>), Info, msg);
	notif!(warn(msg: impl Into<String>), Warn, msg);
	notif!(error(msg: impl Into<String>), Error, msg);
	notif!(success(msg: impl Into<String>), Success, msg);
	notif!(debug(msg: impl Into<String>), Debug, msg);
}
