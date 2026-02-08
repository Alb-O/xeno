use super::{AutoDismiss, Level};
use crate::core::RegistryMeta;

/// Symbolized notification entry.
pub struct NotificationEntry {
	pub meta: RegistryMeta,
	pub level: Level,
	pub auto_dismiss: AutoDismiss,
}

crate::impl_registry_entry!(NotificationEntry);
