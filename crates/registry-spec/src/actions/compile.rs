//! KDL → [`ActionsSpec`] compiler.

use std::collections::HashSet;
use std::fs;

use kdl::KdlDocument;

use super::*;
use crate::compile::*;

pub fn build(ctx: &BuildCtx) {
	let path = ctx.asset("src/domains/actions/assets/actions.kdl");
	ctx.rerun_if_changed(&path);

	let kdl = fs::read_to_string(&path).expect("failed to read actions.kdl");
	let doc: KdlDocument = kdl.parse().expect("failed to parse actions.kdl");

	let (actions, prefixes) = collect_action_nodes(&doc);

	let mut seen = HashSet::new();
	for action in &actions {
		if !seen.insert(&action.common.name) {
			panic!("duplicate action name: '{}'", action.common.name);
		}
	}

	let spec = ActionsSpec { actions, prefixes };
	let bin = postcard::to_stdvec(&spec).expect("failed to serialize actions spec");
	ctx.write_blob("actions.bin", &bin);
}

fn collect_action_nodes(doc: &KdlDocument) -> (Vec<ActionSpec>, Vec<KeyPrefixSpec>) {
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

fn parse_action_node(node: &kdl::KdlNode, group_name: Option<&str>) -> ActionSpec {
	let name = node_name_arg(node, "action");
	let context = format!("action '{name}'");

	let description = require_str(node, "description", &context);

	let short_desc = node.get("short-desc").and_then(|v| v.as_string()).map(String::from);

	let priority = node.get("priority").and_then(|v| v.as_integer()).map(|v| v as i16).unwrap_or(0);

	let flags = node.get("flags").and_then(|v| v.as_integer()).map(|v| v as u32).unwrap_or(0);

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
					assert!(VALID_CAPS.contains(&s), "{context}: unknown capability '{s}'");
					caps.push(s.to_string());
				}
			}
		}

		if let Some(bindings_node) = children.get("bindings")
			&& let Some(bindings_children) = bindings_node.children()
		{
			for mode_node in bindings_children.nodes() {
				let mode = mode_node.name().value().to_string();
				assert!(VALID_MODES.contains(&mode.as_str()), "{context}: unknown binding mode '{mode}'");
				for entry in mode_node.entries() {
					if entry.name().is_none()
						&& let Some(keys) = entry.value().as_string()
					{
						bindings.push(KeyBindingSpec {
							mode: mode.clone(),
							keys: keys.to_string(),
						});
					}
				}
			}
		}
	}

	ActionSpec {
		common: MetaCommonSpec {
			name,
			description,
			short_desc,
			keys,
			priority,
			caps,
			flags,
		},
		bindings,
		group: group_name.map(String::from),
	}
}

fn parse_prefix_node(node: &kdl::KdlNode) -> KeyPrefixSpec {
	let mode = require_str(node, "mode", "prefix");
	assert!(VALID_MODES.contains(&mode.as_str()), "prefix: unknown mode '{mode}'");

	KeyPrefixSpec {
		keys: require_str(node, "keys", "prefix"),
		description: require_str(node, "description", "prefix"),
		mode,
	}
}
