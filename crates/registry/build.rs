use std::collections::HashSet;
use std::io::Write;
use std::path::PathBuf;
use std::{env, fs};

use kdl::KdlDocument;
use serde::{Deserialize, Serialize};

const MAGIC: &[u8; 8] = b"XENOASST";
const SCHEMA_VERSION: u32 = 1;

const VALID_MODES: &[&str] = &["normal", "insert", "match", "space"];
const VALID_CAPS: &[&str] = &[
	"Text",
	"Cursor",
	"Selection",
	"Mode",
	"Messaging",
	"Edit",
	"Search",
	"Undo",
	"FileOps",
	"Overlay",
];

// ── Shared serialization types (mirrors kdl/types.rs) ─────────────────

#[derive(Debug, Serialize, Deserialize)]
struct ActionMetaRaw {
	name: String,
	description: String,
	short_desc: Option<String>,
	keys: Vec<String>,
	priority: i16,
	caps: Vec<String>,
	flags: u32,
	bindings: Vec<KeyBindingRaw>,
	group: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
struct KeyBindingRaw {
	mode: String,
	keys: String,
}

#[derive(Debug, Serialize, Deserialize)]
struct KeyPrefixRaw {
	mode: String,
	keys: String,
	description: String,
}

#[derive(Debug, Serialize, Deserialize)]
struct ActionsBlob {
	actions: Vec<ActionMetaRaw>,
	prefixes: Vec<KeyPrefixRaw>,
}

#[derive(Debug, Serialize, Deserialize)]
struct CommandMetaRaw {
	name: String,
	description: String,
	keys: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize)]
struct CommandsBlob {
	commands: Vec<CommandMetaRaw>,
}

#[derive(Debug, Serialize, Deserialize)]
struct MotionMetaRaw {
	name: String,
	description: String,
	keys: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize)]
struct MotionsBlob {
	motions: Vec<MotionMetaRaw>,
}

#[derive(Debug, Serialize, Deserialize)]
struct TextObjectMetaRaw {
	name: String,
	description: String,
	trigger: String,
	alt_triggers: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize)]
struct TextObjectsBlob {
	text_objects: Vec<TextObjectMetaRaw>,
}

#[derive(Debug, Serialize, Deserialize)]
struct OptionMetaRaw {
	name: String,
	keys: Vec<String>,
	priority: i16,
	flags: u32,
	kdl_key: String,
	value_type: String,
	default: String,
	scope: String,
	description: String,
	validator: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
struct OptionsBlob {
	options: Vec<OptionMetaRaw>,
}

#[derive(Debug, Serialize, Deserialize)]
struct GutterMetaRaw {
	name: String,
	description: String,
	priority: i16,
	width: String,
	enabled: bool,
}

#[derive(Debug, Serialize, Deserialize)]
struct GuttersBlob {
	gutters: Vec<GutterMetaRaw>,
}

#[derive(Debug, Serialize, Deserialize)]
struct StatuslineMetaRaw {
	name: String,
	description: String,
	position: String,
	priority: i16,
}

#[derive(Debug, Serialize, Deserialize)]
struct StatuslineBlob {
	segments: Vec<StatuslineMetaRaw>,
}

#[derive(Debug, Serialize, Deserialize)]
struct HookMetaRaw {
	name: String,
	event: String,
	priority: i16,
	description: String,
}

#[derive(Debug, Serialize, Deserialize)]
struct HooksBlob {
	hooks: Vec<HookMetaRaw>,
}

#[derive(Debug, Serialize, Deserialize)]
struct NotificationMetaRaw {
	name: String,
	level: String,
	auto_dismiss: String,
	dismiss_ms: Option<u64>,
	description: String,
}

#[derive(Debug, Serialize, Deserialize)]
struct NotificationsBlob {
	notifications: Vec<NotificationMetaRaw>,
}

#[derive(Debug, Serialize, Deserialize)]
struct ThemeMetaRaw {
	name: String,
	keys: Vec<String>,
	description: String,
	priority: i16,
	variant: String,
	palette: std::collections::HashMap<String, String>,
	ui: std::collections::HashMap<String, String>,
	mode: std::collections::HashMap<String, String>,
	semantic: std::collections::HashMap<String, String>,
	popup: std::collections::HashMap<String, String>,
	syntax: std::collections::HashMap<String, RawStyle>,
}

#[derive(Debug, Serialize, Deserialize)]
struct RawStyle {
	fg: Option<String>,
	bg: Option<String>,
	modifiers: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
struct ThemesBlob {
	themes: Vec<ThemeMetaRaw>,
}

// ── Blob I/O ──────────────────────────────────────────────────────────

fn write_blob(path: &PathBuf, data: &[u8]) {
	let mut file = fs::File::create(path).expect("failed to create blob");
	file.write_all(MAGIC).expect("failed to write magic");
	file.write_all(&SCHEMA_VERSION.to_le_bytes())
		.expect("failed to write version");
	file.write_all(data).expect("failed to write data");
}

// ── Helpers ───────────────────────────────────────────────────────────

/// Extracts the first positional string argument from a KDL node.
fn node_name_arg(node: &kdl::KdlNode, domain: &str) -> String {
	node.entry(0)
		.and_then(|e| {
			if e.name().is_none() {
				e.value().as_string().map(String::from)
			} else {
				None
			}
		})
		.unwrap_or_else(|| panic!("{domain} node missing name argument"))
}

/// Extracts a required string attribute.
fn require_str(node: &kdl::KdlNode, attr: &str, context: &str) -> String {
	node.get(attr)
		.and_then(|v| v.as_string())
		.unwrap_or_else(|| panic!("{context} missing '{attr}' attribute"))
		.to_string()
}

/// Extracts positional string arguments from a child node.
fn collect_keys(node: &kdl::KdlNode) -> Vec<String> {
	let Some(children) = node.children() else {
		return Vec::new();
	};
	let Some(keys_node) = children.get("keys") else {
		return Vec::new();
	};
	keys_node
		.entries()
		.iter()
		.filter(|e| e.name().is_none())
		.filter_map(|e| e.value().as_string().map(String::from))
		.collect()
}

/// Validates no duplicate names in a list.
fn validate_unique(items: &[(String, String)], domain: &str) {
	let mut seen = HashSet::new();
	for (name, _) in items {
		if !seen.insert(name.as_str()) {
			panic!("duplicate {domain} name: '{name}'");
		}
	}
}

// ── Actions ───────────────────────────────────────────────────────────

fn parse_action_node(node: &kdl::KdlNode, group_name: Option<&str>) -> ActionMetaRaw {
	let name = node_name_arg(node, "action");
	let context = format!("action '{name}'");

	let description = require_str(node, "description", &context);

	let short_desc = node
		.get("short-desc")
		.and_then(|v| v.as_string())
		.map(String::from);

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

	let children = node.children();

	let mut keys = Vec::new();
	let mut caps = Vec::new();
	let mut bindings = Vec::new();

	if let Some(children) = children {
		if let Some(keys_node) = children.get("keys") {
			for entry in keys_node.entries() {
				if entry.name().is_none()
					&& let Some(s) = entry.value().as_string()
				{
					keys.push(s.to_string());
				}
			}
		}

		if let Some(caps_node) = children.get("caps") {
			for entry in caps_node.entries() {
				if entry.name().is_none()
					&& let Some(s) = entry.value().as_string()
				{
					assert!(
						VALID_CAPS.contains(&s),
						"{context}: unknown capability '{s}'"
					);
					caps.push(s.to_string());
				}
			}
		}

		if let Some(bindings_node) = children.get("bindings")
			&& let Some(bindings_children) = bindings_node.children()
		{
			for mode_node in bindings_children.nodes() {
				let mode = mode_node.name().value().to_string();
				assert!(
					VALID_MODES.contains(&mode.as_str()),
					"{context}: unknown binding mode '{mode}'"
				);
				for entry in mode_node.entries() {
					if entry.name().is_none()
						&& let Some(keys) = entry.value().as_string()
					{
						bindings.push(KeyBindingRaw {
							mode: mode.clone(),
							keys: keys.to_string(),
						});
					}
				}
			}
		}
	}

	ActionMetaRaw {
		name,
		description,
		short_desc,
		keys,
		priority,
		caps,
		flags,
		bindings,
		group: group_name.map(String::from),
	}
}

fn parse_prefix_node(node: &kdl::KdlNode) -> KeyPrefixRaw {
	let mode = require_str(node, "mode", "prefix");
	assert!(
		VALID_MODES.contains(&mode.as_str()),
		"prefix: unknown mode '{mode}'"
	);

	KeyPrefixRaw {
		keys: require_str(node, "keys", "prefix"),
		description: require_str(node, "description", "prefix"),
		mode,
	}
}

fn collect_action_nodes(doc: &KdlDocument) -> (Vec<ActionMetaRaw>, Vec<KeyPrefixRaw>) {
	let mut actions = Vec::new();
	let mut prefixes = Vec::new();

	for node in doc.nodes() {
		let name = node.name().value();
		match name {
			"action" => {
				actions.push(parse_action_node(node, None));
			}
			"prefix" => {
				prefixes.push(parse_prefix_node(node));
			}
			_ => {
				if let Some(children) = node.children() {
					for child in children.nodes() {
						let child_name = child.name().value();
						match child_name {
							"action" => {
								actions.push(parse_action_node(child, Some(name)));
							}
							"prefix" => {
								prefixes.push(parse_prefix_node(child));
							}
							_ => panic!("unexpected node '{child_name}' inside group '{name}'"),
						}
					}
				}
			}
		}
	}

	(actions, prefixes)
}

fn build_actions_blob(data_dir: &PathBuf, out_dir: &PathBuf) {
	let path = data_dir.join("actions.kdl");
	println!("cargo:rerun-if-changed={}", path.display());

	let kdl = fs::read_to_string(&path).expect("failed to read actions.kdl");
	let doc: KdlDocument = kdl.parse().expect("failed to parse actions.kdl");

	let (actions, prefixes) = collect_action_nodes(&doc);

	let mut seen = HashSet::new();
	for action in &actions {
		if !seen.insert(&action.name) {
			panic!("duplicate action name: '{}'", action.name);
		}
	}

	let blob = ActionsBlob { actions, prefixes };
	let bin = postcard::to_stdvec(&blob).expect("failed to serialize actions blob");
	write_blob(&out_dir.join("actions.bin"), &bin);
}

// ── Commands ──────────────────────────────────────────────────────────

fn build_commands_blob(data_dir: &PathBuf, out_dir: &PathBuf) {
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

// ── Motions ───────────────────────────────────────────────────────────

fn build_motions_blob(data_dir: &PathBuf, out_dir: &PathBuf) {
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

// ── Text Objects ──────────────────────────────────────────────────────

fn build_textobj_blob(data_dir: &PathBuf, out_dir: &PathBuf) {
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

// ── Options ───────────────────────────────────────────────────────────

fn build_options_blob(data_dir: &PathBuf, out_dir: &PathBuf) {
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

// ── Gutters ───────────────────────────────────────────────────────────

fn build_gutters_blob(data_dir: &PathBuf, out_dir: &PathBuf) {
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

// ── Statusline ────────────────────────────────────────────────────────

fn build_statusline_blob(data_dir: &PathBuf, out_dir: &PathBuf) {
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

// ── Hooks ─────────────────────────────────────────────────────────────

fn build_hooks_blob(data_dir: &PathBuf, out_dir: &PathBuf) {
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

// ── Notifications ──────────────────────────────────────────────────────

fn build_notifications_blob(data_dir: &PathBuf, out_dir: &PathBuf) {
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

// ── Themes ───────────────────────────────────────────────────────────

fn build_themes_blob(data_dir: &PathBuf, out_dir: &PathBuf) {
	let mut themes = Vec::new();

	let entries = fs::read_dir(data_dir).expect("failed to read themes directory");
	for entry in entries {
		let entry = entry.expect("failed to read theme entry");
		let path = entry.path();
		if path.extension().is_some_and(|ext| ext == "kdl") {
			println!("cargo:rerun-if-changed={}", path.display());
			let kdl = fs::read_to_string(&path).expect("failed to read theme kdl");
			let doc: KdlDocument = kdl
				.parse()
				.unwrap_or_else(|e| panic!("failed to parse theme {}: {}", path.display(), e));

			let name = doc
				.get_arg("name")
				.and_then(|v| v.as_string())
				.unwrap_or_else(|| path.file_stem().unwrap().to_str().unwrap())
				.to_string();

			let variant = doc
				.get_arg("variant")
				.and_then(|v| v.as_string())
				.unwrap_or("dark")
				.to_string();

			let keys = doc
				.get("keys")
				.map(|n| {
					n.entries()
						.iter()
						.filter_map(|e| e.value().as_string().map(String::from))
						.collect()
				})
				.unwrap_or_default();

			let description = doc
				.get_arg("description")
				.and_then(|v| v.as_string())
				.unwrap_or("")
				.to_string();

			let priority = doc
				.get_arg("priority")
				.and_then(|v| v.as_integer())
				.map(|v| v as i16)
				.unwrap_or(0);

			let palette = parse_kdl_map(doc.get("palette"));
			let ui = parse_kdl_map(doc.get("ui"));
			let mode = parse_kdl_map(doc.get("mode"));
			let semantic = parse_kdl_map(doc.get("semantic"));
			let popup = parse_kdl_map(doc.get("popup"));

			let mut syntax = std::collections::HashMap::new();
			if let Some(node) = doc.get("syntax")
				&& let Some(children) = node.children() {
					parse_syntax_recursive(children, "", &mut syntax);
				}

			themes.push(ThemeMetaRaw {
				name,
				keys,
				description,
				priority,
				variant,
				palette,
				ui,
				mode,
				semantic,
				popup,
				syntax,
			});
		}
	}

	let blob = ThemesBlob { themes };
	let bin = postcard::to_stdvec(&blob).expect("failed to serialize themes blob");
	write_blob(&out_dir.join("themes.bin"), &bin);
}

fn parse_kdl_map(node: Option<&kdl::KdlNode>) -> std::collections::HashMap<String, String> {
	let mut map = std::collections::HashMap::new();
	if let Some(node) = node
		&& let Some(children) = node.children()
	{
		for child in children.nodes() {
			if let Some(entry) = child.entry(0)
				&& let Some(val) = entry.value().as_string()
			{
				map.insert(child.name().value().to_string(), val.to_string());
			}
		}
	}
	map
}

fn parse_syntax_recursive(
	children: &kdl::KdlDocument,
	prefix: &str,
	map: &mut std::collections::HashMap<String, RawStyle>,
) {
	for node in children.nodes() {
		let name = node.name().value();
		let scope = if prefix.is_empty() {
			name.to_string()
		} else {
			format!("{prefix}.{name}")
		};

		let fg = node.get("fg").and_then(|v| v.as_string()).map(String::from);
		let bg = node.get("bg").and_then(|v| v.as_string()).map(String::from);
		let modifiers = node
			.get("mod")
			.or_else(|| node.get("modifiers"))
			.and_then(|v| v.as_string())
			.map(String::from);

		if fg.is_some() || bg.is_some() || modifiers.is_some() {
			map.insert(scope.clone(), RawStyle { fg, bg, modifiers });
		}

		if let Some(children) = node.children() {
			parse_syntax_recursive(children, &scope, map);
		}
	}
}

// ── Main ──────────────────────────────────────────────────────────────

fn main() {
	let manifest_dir = env::var("CARGO_MANIFEST_DIR").unwrap();
	let out_dir = PathBuf::from(env::var("OUT_DIR").unwrap());

	let data_dir = PathBuf::from(&manifest_dir)
		.parent()
		.unwrap()
		.join("runtime/data/assets/registry");

	build_actions_blob(&data_dir, &out_dir);
	build_commands_blob(&data_dir, &out_dir);
	build_motions_blob(&data_dir, &out_dir);
	build_textobj_blob(&data_dir, &out_dir);
	build_options_blob(&data_dir, &out_dir);
	build_gutters_blob(&data_dir, &out_dir);
	build_statusline_blob(&data_dir, &out_dir);
	build_hooks_blob(&data_dir, &out_dir);
	build_notifications_blob(&data_dir, &out_dir);

	let themes_dir = PathBuf::from(&manifest_dir)
		.parent()
		.unwrap()
		.join("runtime/data/assets/themes");
	println!("cargo:rerun-if-changed={}", themes_dir.display());
	build_themes_blob(&themes_dir, &out_dir);
}
