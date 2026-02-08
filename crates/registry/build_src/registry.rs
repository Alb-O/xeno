use std::fs;
use std::path::PathBuf;

use kdl::KdlDocument;

use super::common::*;
use super::types::*;

pub fn build_commands_blob(data_dir: &PathBuf, out_dir: &PathBuf) {
	let path = data_dir.join("commands.kdl");
	println!("cargo:rerun-if-changed={}", path.display());

	let kdl = fs::read_to_string(&path).expect("failed to read commands.kdl");
	let doc: KdlDocument = kdl.parse().expect("failed to parse commands.kdl");

	let mut commands = Vec::new();
	for node in doc.nodes() {
		assert_eq!(
			node.name().value(),
			"command",
			"unexpected top-level node '{}' in commands.kdl",
			node.name().value()
		);
		let name = node_name_arg(node, "command");
		let context = format!("command '{name}'");
		let description = require_str(node, "description", &context);
		let keys = collect_keys(node);
		commands.push(CommandMetaRaw {
			name,
			description,
			keys,
		});
	}

	let pairs: Vec<(String, String)> = commands
		.iter()
		.map(|c| (c.name.clone(), String::new()))
		.collect();
	validate_unique(&pairs, "command");

	let blob = CommandsBlob { commands };
	let bin = postcard::to_stdvec(&blob).expect("failed to serialize commands blob");
	write_blob(&out_dir.join("commands.bin"), &bin);
}

pub fn build_motions_blob(data_dir: &PathBuf, out_dir: &PathBuf) {
	let path = data_dir.join("motions.kdl");
	println!("cargo:rerun-if-changed={}", path.display());

	let kdl = fs::read_to_string(&path).expect("failed to read motions.kdl");
	let doc: KdlDocument = kdl.parse().expect("failed to parse motions.kdl");

	let mut motions = Vec::new();
	for node in doc.nodes() {
		assert_eq!(
			node.name().value(),
			"motion",
			"unexpected top-level node '{}' in motions.kdl",
			node.name().value()
		);
		let name = node_name_arg(node, "motion");
		let context = format!("motion '{name}'");
		let description = require_str(node, "description", &context);
		let keys = collect_keys(node);
		motions.push(MotionMetaRaw {
			name,
			description,
			keys,
		});
	}

	let pairs: Vec<(String, String)> = motions
		.iter()
		.map(|m| (m.name.clone(), String::new()))
		.collect();
	validate_unique(&pairs, "motion");

	let blob = MotionsBlob { motions };
	let bin = postcard::to_stdvec(&blob).expect("failed to serialize motions blob");
	write_blob(&out_dir.join("motions.bin"), &bin);
}

pub fn build_textobj_blob(data_dir: &PathBuf, out_dir: &PathBuf) {
	let path = data_dir.join("text_objects.kdl");
	println!("cargo:rerun-if-changed={}", path.display());

	let kdl = fs::read_to_string(&path).expect("failed to read text_objects.kdl");
	let doc: KdlDocument = kdl.parse().expect("failed to parse text_objects.kdl");

	let mut text_objects = Vec::new();
	for node in doc.nodes() {
		assert_eq!(
			node.name().value(),
			"text_object",
			"unexpected top-level node '{}' in text_objects.kdl",
			node.name().value()
		);
		let name = node_name_arg(node, "text_object");
		let context = format!("text_object '{name}'");
		let description = require_str(node, "description", &context);
		let trigger = require_str(node, "trigger", &context);

		let alt_triggers = node
			.children()
			.and_then(|c| c.get("alt-triggers"))
			.map(|n| {
				n.entries()
					.iter()
					.filter(|e| e.name().is_none())
					.filter_map(|e| e.value().as_string().map(String::from))
					.collect()
			})
			.unwrap_or_default();

		text_objects.push(TextObjectMetaRaw {
			name,
			description,
			trigger,
			alt_triggers,
		});
	}

	let pairs: Vec<(String, String)> = text_objects
		.iter()
		.map(|t| (t.name.clone(), String::new()))
		.collect();
	validate_unique(&pairs, "text_object");

	let blob = TextObjectsBlob { text_objects };
	let bin = postcard::to_stdvec(&blob).expect("failed to serialize text_objects blob");
	write_blob(&out_dir.join("text_objects.bin"), &bin);
}

pub fn build_options_blob(data_dir: &PathBuf, out_dir: &PathBuf) {
	let path = data_dir.join("options.kdl");
	println!("cargo:rerun-if-changed={}", path.display());

	let kdl = fs::read_to_string(&path).expect("failed to read options.kdl");
	let doc: KdlDocument = kdl.parse().expect("failed to parse options.kdl");

	let valid_types = ["bool", "int", "string"];
	let valid_scopes = ["buffer", "global"];

	let mut options = Vec::new();
	for node in doc.nodes() {
		assert_eq!(
			node.name().value(),
			"option",
			"unexpected top-level node '{}' in options.kdl",
			node.name().value()
		);
		let name = node_name_arg(node, "option");
		let context = format!("option '{name}'");

		let keys = collect_keys(node);
		let priority = node
			.get("priority")
			.and_then(|v| v.as_integer())
			.map(|v| v as i16)
			.unwrap_or(0);
		let flags = node
			.get("flags")
			.and_then(|v| v.as_integer())
			.map(|v| v as u32)
			.unwrap_or(0);

		let kdl_key = require_str(node, "kdl-key", &context);
		let value_type = require_str(node, "value-type", &context);
		assert!(
			valid_types.contains(&value_type.as_str()),
			"{context}: unknown value-type '{value_type}'"
		);
		let scope = require_str(node, "scope", &context);
		assert!(
			valid_scopes.contains(&scope.as_str()),
			"{context}: unknown scope '{scope}'"
		);
		let description = require_str(node, "description", &context);

		let default = require_str(node, "default", &context);

		let validator = node
			.get("validator")
			.and_then(|v| v.as_string())
			.map(String::from);

		options.push(OptionMetaRaw {
			name,
			keys,
			priority,
			flags,
			kdl_key,
			value_type,
			default,
			scope,
			description,
			validator,
		});
	}

	let pairs: Vec<(String, String)> = options
		.iter()
		.map(|o| (o.name.clone(), String::new()))
		.collect();
	validate_unique(&pairs, "option");

	let blob = OptionsBlob { options };
	let bin = postcard::to_stdvec(&blob).expect("failed to serialize options blob");
	write_blob(&out_dir.join("options.bin"), &bin);
}

pub fn build_gutters_blob(data_dir: &PathBuf, out_dir: &PathBuf) {
	let path = data_dir.join("gutters.kdl");
	println!("cargo:rerun-if-changed={}", path.display());

	let kdl = fs::read_to_string(&path).expect("failed to read gutters.kdl");
	let doc: KdlDocument = kdl.parse().expect("failed to parse gutters.kdl");

	let mut gutters = Vec::new();
	for node in doc.nodes() {
		assert_eq!(
			node.name().value(),
			"gutter",
			"unexpected top-level node '{}' in gutters.kdl",
			node.name().value()
		);
		let name = node_name_arg(node, "gutter");
		let context = format!("gutter '{name}'");
		let description = require_str(node, "description", &context);
		let priority = node
			.get("priority")
			.and_then(|v| v.as_integer())
			.map(|v| v as i16)
			.unwrap_or(0);

		// width: "dynamic" or integer
		let width = node
			.get("width")
			.map(|v| {
				if let Some(s) = v.as_string() {
					s.to_string()
				} else if let Some(i) = v.as_integer() {
					i.to_string()
				} else {
					panic!("{context}: width must be 'dynamic' or integer");
				}
			})
			.unwrap_or_else(|| "dynamic".to_string());

		let enabled = node
			.get("enabled")
			.and_then(|v| v.as_bool())
			.unwrap_or(true);

		gutters.push(GutterMetaRaw {
			name,
			description,
			priority,
			width,
			enabled,
		});
	}

	let pairs: Vec<(String, String)> = gutters
		.iter()
		.map(|g| (g.name.clone(), String::new()))
		.collect();
	validate_unique(&pairs, "gutter");

	let blob = GuttersBlob { gutters };
	let bin = postcard::to_stdvec(&blob).expect("failed to serialize gutters blob");
	write_blob(&out_dir.join("gutters.bin"), &bin);
}

pub fn build_statusline_blob(data_dir: &PathBuf, out_dir: &PathBuf) {
	let path = data_dir.join("statusline.kdl");
	println!("cargo:rerun-if-changed={}", path.display());

	let kdl = fs::read_to_string(&path).expect("failed to read statusline.kdl");
	let doc: KdlDocument = kdl.parse().expect("failed to parse statusline.kdl");

	let valid_positions = ["left", "right", "center"];

	let mut segments = Vec::new();
	for node in doc.nodes() {
		assert_eq!(
			node.name().value(),
			"segment",
			"unexpected top-level node '{}' in statusline.kdl",
			node.name().value()
		);
		let name = node_name_arg(node, "segment");
		let context = format!("segment '{name}'");
		let description = require_str(node, "description", &context);
		let position = require_str(node, "position", &context);
		assert!(
			valid_positions.contains(&position.as_str()),
			"{context}: unknown position '{position}'"
		);
		let priority = node
			.get("priority")
			.and_then(|v| v.as_integer())
			.map(|v| v as i16)
			.unwrap_or(0);

		segments.push(StatuslineMetaRaw {
			name,
			description,
			position,
			priority,
		});
	}

	let pairs: Vec<(String, String)> = segments
		.iter()
		.map(|s| (s.name.clone(), String::new()))
		.collect();
	validate_unique(&pairs, "segment");

	let blob = StatuslineBlob { segments };
	let bin = postcard::to_stdvec(&blob).expect("failed to serialize statusline blob");
	write_blob(&out_dir.join("statusline.bin"), &bin);
}

pub fn build_hooks_blob(data_dir: &PathBuf, out_dir: &PathBuf) {
	let path = data_dir.join("hooks.kdl");
	println!("cargo:rerun-if-changed={}", path.display());

	let kdl = fs::read_to_string(&path).expect("failed to read hooks.kdl");
	let doc: KdlDocument = kdl.parse().expect("failed to parse hooks.kdl");

	let mut hooks = Vec::new();
	for node in doc.nodes() {
		assert_eq!(
			node.name().value(),
			"hook",
			"unexpected top-level node '{}' in hooks.kdl",
			node.name().value()
		);
		let name = node_name_arg(node, "hook");
		let context = format!("hook '{name}'");
		let event = require_str(node, "event", &context);
		let description = require_str(node, "description", &context);
		let priority = node
			.get("priority")
			.and_then(|v| v.as_integer())
			.map(|v| v as i16)
			.unwrap_or(0);

		hooks.push(HookMetaRaw {
			name,
			event,
			priority,
			description,
		});
	}

	let pairs: Vec<(String, String)> = hooks
		.iter()
		.map(|h| (h.name.clone(), String::new()))
		.collect();
	validate_unique(&pairs, "hook");

	let blob = HooksBlob { hooks };
	let bin = postcard::to_stdvec(&blob).expect("failed to serialize hooks blob");
	write_blob(&out_dir.join("hooks.bin"), &bin);
}

pub fn build_notifications_blob(data_dir: &PathBuf, out_dir: &PathBuf) {
	let path = data_dir.join("notifications.kdl");
	println!("cargo:rerun-if-changed={}", path.display());

	let kdl = fs::read_to_string(&path).expect("failed to read notifications.kdl");
	let doc: KdlDocument = kdl.parse().expect("failed to parse notifications.kdl");

	let valid_levels = ["info", "warn", "error", "debug", "success"];
	let valid_dismiss = ["never", "after"];

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

		let level = require_str(node, "level", &context);
		assert!(
			valid_levels.contains(&level.as_str()),
			"{context}: unknown level '{level}'"
		);

		let auto_dismiss = require_str(node, "auto-dismiss", &context);
		assert!(
			valid_dismiss.contains(&auto_dismiss.as_str()),
			"{context}: unknown auto-dismiss '{auto_dismiss}'"
		);

		let dismiss_ms = node
			.get("dismiss-ms")
			.and_then(|v| v.as_integer())
			.map(|v| v as u64);
		let description = require_str(node, "description", &context);

		notifications.push(NotificationMetaRaw {
			name,
			level,
			auto_dismiss,
			dismiss_ms,
			description,
		});
	}

	let pairs: Vec<(String, String)> = notifications
		.iter()
		.map(|n| (n.name.clone(), String::new()))
		.collect();
	validate_unique(&pairs, "notification");

	let blob = NotificationsBlob { notifications };
	let bin = postcard::to_stdvec(&blob).expect("failed to serialize notifications blob");
	write_blob(&out_dir.join("notifications.bin"), &bin);
}
