use std::time::Duration;

use super::*;
use crate::kdl::types::NotificationsBlob;
use crate::notifications::def::LinkedNotificationDef;
use crate::notifications::{AutoDismiss, Level};

/// Links KDL notification metadata, producing `LinkedNotificationDef`s.
pub fn link_notifications(metadata: &NotificationsBlob) -> Vec<LinkedNotificationDef> {
	let mut defs = Vec::new();

	for meta in &metadata.notifications {
		let id = format!("xeno-registry::{}", meta.name);

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

		defs.push(LinkedNotificationDef {
			id,
			name: meta.name.clone(),
			description: meta.description.clone(),
			keys: Vec::new(),
			priority: 0,
			flags: 0,
			level,
			auto_dismiss,
			source: RegistrySource::Builtin,
		});
	}

	defs
}
