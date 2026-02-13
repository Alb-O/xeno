use std::time::Duration;

use super::*;

#[test]
fn take_pending_render_items_maps_level_and_auto_dismiss() {
	let mut center = NotificationCenter::new();
	center.push(Notification::new(
		"test.notification",
		xeno_registry::notifications::Level::Warn,
		xeno_registry::notifications::AutoDismiss::After(Duration::from_secs(2)),
		"warning",
	));

	let items = center.take_pending_render_items();
	assert_eq!(items.len(), 1);
	assert_eq!(items[0].message, "warning");
	assert_eq!(items[0].level, NotificationRenderLevel::Warn);
	assert_eq!(items[0].auto_dismiss, NotificationRenderAutoDismiss::After(Duration::from_secs(2)));
}
