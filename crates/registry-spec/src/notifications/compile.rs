//! KDL → [`NotificationsSpec`] compiler.

use std::fs;

use kdl::KdlDocument;

use super::*;
use crate::compile::*;

pub fn build(ctx: &BuildCtx) {
	let path = ctx.asset("src/domains/notifications/assets/notifications.kdl");
	ctx.rerun_if_changed(&path);

	let kdl = fs::read_to_string(&path).expect("failed to read notifications.kdl");
	let doc: KdlDocument = kdl.parse().expect("failed to parse notifications.kdl");

	let mut notifications = Vec::new();
	for node in doc.nodes() {
		assert_eq!(
			node.name().value(),
			"notification",
			"unexpected top-level node '{}' in notifications.kdl",
			node.name().value()
		);
		let name = node_name_arg(node, "notification");
		let context = format!("notification '{name}'");
		let keys = collect_keys(node);
		let short_desc = node.get("short-desc").and_then(|v| v.as_string()).map(String::from);
		let priority = node.get("priority").and_then(|v| v.as_integer()).map(|v| v as i16).unwrap_or(0);
		let flags = node.get("flags").and_then(|v| v.as_integer()).map(|v| v as u32).unwrap_or(0);
		if let Some(children) = node.children()
			&& children.get("caps").is_some()
		{
			panic!("{context}: notifications do not support 'caps'");
		}

		let level = require_str(node, "level", &context);
		assert!(VALID_LEVELS.contains(&level.as_str()), "{context}: unknown level '{level}'");

		let auto_dismiss = require_str(node, "auto-dismiss", &context);
		assert!(
			VALID_DISMISS.contains(&auto_dismiss.as_str()),
			"{context}: unknown auto-dismiss '{auto_dismiss}'"
		);

		let dismiss_ms = node.get("dismiss-ms").and_then(|v| v.as_integer()).map(|v| v as u64);
		let description = require_str(node, "description", &context);

		notifications.push(NotificationSpec {
			common: MetaCommonSpec {
				name,
				description,
				short_desc,
				keys,
				priority,
				caps: vec![],
				flags,
			},
			level,
			auto_dismiss,
			dismiss_ms,
		});
	}

	let pairs: Vec<(String, String)> = notifications.iter().map(|n| (n.common.name.clone(), String::new())).collect();
	validate_unique(&pairs, "notification");

	let spec = NotificationsSpec { notifications };
	let bin = postcard::to_stdvec(&spec).expect("failed to serialize notifications spec");
	ctx.write_blob("notifications.bin", &bin);
}
