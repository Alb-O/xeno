use std::time::Duration;

use super::spec::NotificationsSpec;
use crate::core::LinkedDef;
use crate::notifications::def::{LinkedNotificationDef, NotificationPayload};
use crate::notifications::{AutoDismiss, Level};

pub fn link_notifications(spec: &NotificationsSpec) -> Vec<LinkedNotificationDef> {
	let mut defs = Vec::new();

	for meta in &spec.notifications {
		let level = match meta.level.as_str() {
			"info" => Level::Info,
			"warn" => Level::Warn,
			"error" => Level::Error,
			"debug" => Level::Debug,
			"success" => Level::Success,
			other => panic!("unknown notification level: '{}'", other),
		};

		let auto_dismiss = match meta.auto_dismiss.as_str() {
			"never" => AutoDismiss::Never,
			"after" => {
				let ms = meta.dismiss_ms.unwrap_or(4000);
				AutoDismiss::After(Duration::from_millis(ms))
			}
			other => panic!("unknown auto-dismiss: '{}'", other),
		};

		defs.push(LinkedDef {
			meta: crate::defs::link::linked_meta_from_spec(&meta.common),
			payload: NotificationPayload { level, auto_dismiss },
		});
	}

	defs
}
