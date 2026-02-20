//! NUON → [`NotificationsSpec`] compiler.

use std::collections::HashSet;

use crate::build_support::compile::*;
use crate::schema::notifications::{NotificationsSpec, VALID_DISMISS, VALID_LEVELS};

pub fn build(ctx: &BuildCtx) {
	let path = ctx.asset("src/domains/notifications/assets/notifications.nuon");
	ctx.rerun_if_changed(&path);

	let spec: NotificationsSpec = read_nuon_spec(&path);

	let mut seen = HashSet::new();
	for notif in &spec.notifications {
		let name = &notif.common.name;
		if !seen.insert(name) {
			panic!("duplicate notification name: '{name}'");
		}
		assert!(
			VALID_LEVELS.contains(&notif.level.as_str()),
			"notification '{name}': unknown level '{}'",
			notif.level
		);
		assert!(
			VALID_DISMISS.contains(&notif.auto_dismiss.as_str()),
			"notification '{name}': unknown auto_dismiss '{}'",
			notif.auto_dismiss
		);
	}

	let bin = postcard::to_stdvec(&spec).expect("failed to serialize notifications spec");
	ctx.write_blob("notifications.bin", &bin);
}
