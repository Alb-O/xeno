//! KDL → [`ThemesSpec`] compiler.

use std::collections::HashMap;
use std::fs;

use kdl::KdlDocument;

use super::*;
use crate::compile::BuildCtx;

pub fn build(ctx: &BuildCtx) {
	let root = ctx.asset("src/domains/themes/assets");
	ctx.rerun_tree(&root);

	let mut themes = Vec::new();

	for entry in walkdir::WalkDir::new(&root) {
		let entry = entry.expect("failed to walk themes");
		let path = entry.path();
		if path.extension().is_some_and(|ext| ext == "kdl") {
			let kdl = fs::read_to_string(path).expect("failed to read theme kdl");
			let doc: KdlDocument = kdl.parse().unwrap_or_else(|e| panic!("failed to parse theme {}: {}", path.display(), e));

			let name = doc
				.get("name")
				.and_then(|n| n.entry(0))
				.and_then(|e| e.value().as_string())
				.unwrap_or_else(|| path.file_stem().unwrap().to_str().unwrap())
				.to_string();

			let context = format!("theme '{name}'");

			let variant = doc
				.get("variant")
				.and_then(|n| n.entry(0))
				.and_then(|e| e.value().as_string())
				.unwrap_or("dark")
				.to_string();

			let keys = doc
				.get("keys")
				.map(|n| n.entries().iter().filter_map(|e| e.value().as_string().map(String::from)).collect())
				.unwrap_or_default();

			let description = doc
				.get("description")
				.and_then(|n| n.entry(0))
				.and_then(|e| e.value().as_string())
				.unwrap_or("")
				.to_string();

			let short_desc = doc
				.get("short-desc")
				.and_then(|n| n.entry(0))
				.and_then(|e| e.value().as_string())
				.map(String::from);

			let priority = doc
				.get("priority")
				.and_then(|n| n.entry(0))
				.and_then(|e| e.value().as_integer())
				.map(|v| v as i16)
				.unwrap_or(0);

			let flags = doc
				.get("flags")
				.and_then(|n| n.entry(0))
				.and_then(|e| e.value().as_integer())
				.map(|v| v as u32)
				.unwrap_or(0);

			if doc.get("caps").is_some() {
				panic!("{context}: themes do not support 'caps'");
			}

			let palette = parse_kdl_map(doc.get("palette"));
			let ui = parse_kdl_map(doc.get("ui"));
			let mode = parse_kdl_map(doc.get("mode"));
			let semantic = parse_kdl_map(doc.get("semantic"));
			let popup = parse_kdl_map(doc.get("popup"));

			let mut syntax = HashMap::new();
			if let Some(node) = doc.get("syntax")
				&& let Some(children) = node.children()
			{
				parse_syntax_recursive(children, "", &mut syntax);
			}

			themes.push(ThemeSpec {
				common: MetaCommonSpec {
					name,
					description,
					short_desc,
					keys,
					priority,
					caps: vec![],
					flags,
				},
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

	let spec = ThemesSpec { themes };
	let bin = postcard::to_stdvec(&spec).expect("failed to serialize themes spec");
	ctx.write_blob("themes.bin", &bin);
}

fn parse_kdl_map(node: Option<&kdl::KdlNode>) -> HashMap<String, String> {
	let mut map = HashMap::new();
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

fn parse_syntax_recursive(children: &kdl::KdlDocument, prefix: &str, map: &mut HashMap<String, RawStyle>) {
	for node in children.nodes() {
		let name = node.name().value();
		let scope = if prefix.is_empty() { name.to_string() } else { format!("{prefix}.{name}") };

		let fg = node.get("fg").and_then(|v| v.as_string()).map(String::from);
		let bg = node.get("bg").and_then(|v| v.as_string()).map(String::from);
		let modifiers = node.get("mod").or_else(|| node.get("modifiers")).and_then(|v| v.as_string()).map(String::from);

		if fg.is_some() || bg.is_some() || modifiers.is_some() {
			map.insert(scope.clone(), RawStyle { fg, bg, modifiers });
		}

		if let Some(children) = node.children() {
			parse_syntax_recursive(children, &scope, map);
		}
	}
}
