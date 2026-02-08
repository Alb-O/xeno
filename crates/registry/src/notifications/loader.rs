use super::spec::NotificationsSpec;

pub fn load_notifications_spec() -> NotificationsSpec {
	const BYTES: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/notifications.bin"));
	crate::defs::loader::load_blob(BYTES, "notifications")
}
