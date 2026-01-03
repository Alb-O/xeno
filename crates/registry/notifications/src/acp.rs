//! ACP (AI Completion Protocol) notification keys.

use linkme::distributed_slice;

use crate::{
	AutoDismiss, Level, NOTIFICATIONS, Notification, NotificationDef, NotificationKey,
	RegistrySource,
};

#[distributed_slice(NOTIFICATIONS)]
static NOTIF_ACP_STARTING: NotificationDef = NotificationDef::new(
	"acp_starting",
	Level::Info,
	AutoDismiss::DEFAULT,
	RegistrySource::Builtin,
);

#[distributed_slice(NOTIFICATIONS)]
static NOTIF_ACP_STOPPED: NotificationDef = NotificationDef::new(
	"acp_stopped",
	Level::Info,
	AutoDismiss::DEFAULT,
	RegistrySource::Builtin,
);

#[distributed_slice(NOTIFICATIONS)]
static NOTIF_ACP_CANCELLED: NotificationDef = NotificationDef::new(
	"acp_cancelled",
	Level::Info,
	AutoDismiss::DEFAULT,
	RegistrySource::Builtin,
);

#[distributed_slice(NOTIFICATIONS)]
static NOTIF_ACP_MODEL_SET: NotificationDef = NotificationDef::new(
	"acp_model_set",
	Level::Info,
	AutoDismiss::DEFAULT,
	RegistrySource::Builtin,
);

#[distributed_slice(NOTIFICATIONS)]
static NOTIF_ACP_MODEL_INFO: NotificationDef = NotificationDef::new(
	"acp_model_info",
	Level::Info,
	AutoDismiss::Never,
	RegistrySource::Builtin,
);

#[allow(non_upper_case_globals, non_camel_case_types)]
pub mod keys {
	use super::*;

	pub const acp_starting: NotificationKey =
		NotificationKey::new(&NOTIF_ACP_STARTING, "ACP agent starting...");
	pub const acp_stopped: NotificationKey =
		NotificationKey::new(&NOTIF_ACP_STOPPED, "ACP agent stopped");
	pub const acp_cancelled: NotificationKey =
		NotificationKey::new(&NOTIF_ACP_CANCELLED, "ACP request cancelled");

	/// "Setting model to: X".
	pub struct acp_model_set;
	impl acp_model_set {
		pub fn call(model: &str) -> Notification {
			Notification::new(&NOTIF_ACP_MODEL_SET, format!("Setting model to: {}", model))
		}
	}

	/// ACP model info display (no auto-dismiss).
	pub struct acp_model_info;
	impl acp_model_info {
		pub fn call(text: impl Into<String>) -> Notification {
			Notification::new(&NOTIF_ACP_MODEL_INFO, text)
		}
	}
}
