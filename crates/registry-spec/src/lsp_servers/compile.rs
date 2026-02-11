//! KDL → [`LspServersSpec`] compiler.

use std::collections::{BTreeMap, HashSet};
use std::fs;

use kdl::{KdlDocument, KdlNode, KdlValue};
use serde_json::Value as JsonValue;

use super::*;
use crate::compile::BuildCtx;

pub fn build(ctx: &BuildCtx) {
	let root = ctx.asset("src/domains/lsp_servers/assets");
	ctx.rerun_tree(&root);

	let path = root.join("lsp.kdl");
	let kdl = fs::read_to_string(&path).expect("failed to read lsp.kdl");
	let servers = parse_lsp_kdl(&kdl);

	let mut seen = HashSet::new();
	for server in &servers {
		if !seen.insert(&server.common.name) {
			panic!("duplicate lsp server name: '{}'", server.common.name);
		}
	}

	let spec = LspServersSpec { servers };
	let bin = postcard::to_stdvec(&spec).expect("failed to serialize lsp_servers spec");
	ctx.write_blob("lsp_servers.bin", &bin);
}

fn parse_lsp_kdl(input: &str) -> Vec<LspServerSpec> {
	let doc: KdlDocument = input.parse().expect("failed to parse lsp.kdl");
	let mut servers: Vec<_> = doc.nodes().iter().map(parse_lsp_server_node).collect();
	servers.sort_by(|a, b| a.common.name.cmp(&b.common.name));
	servers
}

fn parse_lsp_server_node(node: &KdlNode) -> LspServerSpec {
	let name = node.name().value().to_string();
	let command = node
		.entry(0)
		.and_then(|e| if e.name().is_none() { e.value().as_string().map(String::from) } else { None })
		.unwrap_or_else(|| name.clone());

	let children = node.children();
	let args = parse_string_args(children, "args");
	let environment = parse_lsp_environment(children);
	let config_json = children.and_then(|c| c.get("config")).map(|n| kdl_node_to_json(n).to_string());
	let source = children
		.and_then(|c| c.get("source"))
		.and_then(|n| n.entry(0))
		.and_then(|e| e.value().as_string())
		.map(String::from);
	let nix = children.and_then(|c| c.get("nix")).and_then(|n| {
		let entry = n.entry(0)?;
		if entry.value().as_bool() == Some(false) {
			None
		} else {
			entry.value().as_string().map(String::from)
		}
	});

	LspServerSpec {
		common: MetaCommonSpec {
			name,
			description: String::new(),
			short_desc: None,
			keys: Vec::new(),
			priority: 0,
			caps: Vec::new(),
			flags: 0,
		},
		command,
		args,
		environment,
		config_json,
		source,
		nix,
	}
}

fn parse_string_args(children: Option<&KdlDocument>, node_name: &str) -> Vec<String> {
	let Some(children) = children else {
		return Vec::new();
	};
	let Some(node) = children.get(node_name) else {
		return Vec::new();
	};
	node.entries()
		.iter()
		.filter(|e| e.name().is_none())
		.filter_map(|e| e.value().as_string())
		.map(String::from)
		.collect()
}

fn parse_lsp_environment(children: Option<&KdlDocument>) -> BTreeMap<String, String> {
	let mut env = BTreeMap::new();
	let Some(env_node) = children.and_then(|c| c.get("environment")) else {
		return env;
	};

	for entry in env_node.entries() {
		if let Some(name) = entry.name()
			&& let Some(value) = entry.value().as_string()
		{
			env.insert(name.value().to_string(), value.to_string());
		}
	}

	if let Some(env_children) = env_node.children() {
		for child in env_children.nodes() {
			if let Some(value) = child.entry(0).and_then(|e| e.value().as_string()) {
				env.insert(child.name().value().to_string(), value.to_string());
			}
		}
	}

	env
}

fn kdl_value_to_json(value: &KdlValue) -> JsonValue {
	if let Some(s) = value.as_string() {
		JsonValue::String(s.to_string())
	} else if let Some(i) = value.as_integer() {
		JsonValue::Number((i as i64).into())
	} else if let Some(b) = value.as_bool() {
		JsonValue::Bool(b)
	} else {
		JsonValue::Null
	}
}

fn kdl_doc_to_json(doc: &KdlDocument) -> JsonValue {
	let mut map = serde_json::Map::new();
	for node in doc.nodes() {
		let key = node.name().value().to_string();
		if node.children().is_some() {
			map.insert(key, kdl_node_to_json(node));
		} else if let Some(entry) = node.entry(0) {
			map.insert(key, kdl_value_to_json(entry.value()));
		} else {
			map.insert(key, JsonValue::Bool(true));
		}
	}
	JsonValue::Object(map)
}

fn kdl_node_to_json(node: &KdlNode) -> JsonValue {
	let Some(children) = node.children() else {
		if let Some(entry) = node.entry(0) {
			return kdl_value_to_json(entry.value());
		}
		return JsonValue::Object(serde_json::Map::new());
	};
	kdl_doc_to_json(children)
}
