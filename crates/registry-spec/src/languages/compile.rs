//! KDL → [`LanguagesSpec`] compiler.

use std::collections::HashSet;
use std::fs;

use kdl::{KdlDocument, KdlNode};

use super::*;
use crate::compile::BuildCtx;

pub fn build(ctx: &BuildCtx) {
	let root = ctx.asset("src/domains/languages/assets");
	ctx.rerun_tree(&root);

	let path = root.join("languages.kdl");
	let kdl = fs::read_to_string(&path).expect("failed to read languages.kdl");
	let mut languages = parse_languages_kdl(&kdl);

	let queries_root = root.join("queries");
	for lang in &mut languages {
		let lang_dir = queries_root.join(&lang.common.name);
		if lang_dir.exists() {
			for entry in walkdir::WalkDir::new(&lang_dir) {
				let entry = entry.expect("failed to walk queries");
				let path = entry.path();
				if path.extension().is_some_and(|ext| ext == "scm") {
					let kind = path.file_stem().unwrap().to_str().unwrap().to_string();
					let text = fs::read_to_string(path).expect("failed to read query");
					lang.queries.push(LanguageQuerySpec { kind, text });
				}
			}
			lang.queries.sort_by(|a, b| a.kind.cmp(&b.kind));
		}
	}

	let mut seen = HashSet::new();
	for lang in &languages {
		if !seen.insert(&lang.common.name) {
			panic!("duplicate language name: '{}'", lang.common.name);
		}
	}

	let spec = LanguagesSpec { langs: languages };
	let bin = postcard::to_stdvec(&spec).expect("failed to serialize languages spec");
	ctx.write_blob("languages.bin", &bin);
}

fn parse_languages_kdl(input: &str) -> Vec<LanguageSpec> {
	let doc: KdlDocument = input.parse().expect("failed to parse languages.kdl");
	doc.nodes().iter().filter(|n| n.name().value() == "language").map(parse_language_node).collect()
}

fn parse_language_node(node: &KdlNode) -> LanguageSpec {
	let name = node.get("name").and_then(|v| v.as_string()).expect("language missing name").to_string();
	let scope = node.get("scope").and_then(|v| v.as_string()).map(String::from);
	let grammar = node.get("grammar").and_then(|v| v.as_string()).map(String::from);
	let injection_regex = node.get("injection-regex").and_then(|v| v.as_string()).map(String::from);
	let auto_format = node.get("auto-format").and_then(|v| v.as_bool()).unwrap_or(false);

	let children = node.children();
	let (extensions, filenames, globs) = parse_file_types(children);
	let shebangs = parse_string_args(children, "shebangs");

	let mut comment_tokens = Vec::new();
	if let Some(token) = node.get("comment-token").and_then(|v| v.as_string()) {
		comment_tokens.push(token.to_string());
	}
	comment_tokens.extend(parse_string_args(children, "comment-tokens"));

	let block_comment = parse_block_comment(node, children);
	let lsp_servers = parse_language_servers_from_lang(children);
	let roots = parse_string_args(children, "roots");

	LanguageSpec {
		common: MetaCommonSpec {
			name,
			description: String::new(),
			short_desc: None,
			keys: Vec::new(),
			priority: node.get("priority").and_then(|v| v.as_integer()).map(|v| v as i16).unwrap_or(0),
			caps: Vec::new(),
			flags: 0,
		},
		scope,
		grammar_name: grammar,
		injection_regex,
		auto_format,
		extensions,
		filenames,
		globs,
		shebangs,
		comment_tokens,
		block_comment,
		lsp_servers,
		roots,
		viewport_repair: None,
		queries: Vec::new(),
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

fn parse_file_types(children: Option<&KdlDocument>) -> (Vec<String>, Vec<String>, Vec<String>) {
	let mut extensions = Vec::new();
	let mut filenames = Vec::new();
	let mut globs = Vec::new();

	let Some(children) = children else {
		return (extensions, filenames, globs);
	};
	let Some(file_types_node) = children.get("file-types") else {
		return (extensions, filenames, globs);
	};

	for entry in file_types_node.entries() {
		if entry.name().is_none()
			&& let Some(s) = entry.value().as_string()
		{
			extensions.push(s.to_string());
		}
	}

	if let Some(ft_children) = file_types_node.children() {
		for child in ft_children.nodes() {
			if child.name().value() != "-" {
				continue;
			}
			if let Some(glob) = child.get("glob").and_then(|v| v.as_string()) {
				if glob.contains('*') || glob.contains('?') || glob.contains('[') {
					globs.push(glob.to_string());
				} else {
					filenames.push(glob.to_string());
				}
			} else if let Some(s) = child.entry(0).and_then(|e| e.value().as_string()) {
				extensions.push(s.to_string());
			}
		}
	}

	(extensions, filenames, globs)
}

fn parse_block_comment(node: &KdlNode, children: Option<&KdlDocument>) -> Option<(String, String)> {
	if let (Some(start), Some(end)) = (
		node.get("block-comment-start").and_then(|v| v.as_string()),
		node.get("block-comment-end").and_then(|v| v.as_string()),
	) {
		return Some((start.to_string(), end.to_string()));
	}

	let children = children?;
	let bc_node = children.get("block-comment-tokens")?;

	if let (Some(start), Some(end)) = (bc_node.get("start").and_then(|v| v.as_string()), bc_node.get("end").and_then(|v| v.as_string())) {
		return Some((start.to_string(), end.to_string()));
	}

	bc_node.children().and_then(|bc_children| {
		bc_children.nodes().iter().find_map(|child| {
			if child.name().value() != "-" {
				return None;
			}
			let start = child.get("start").and_then(|v| v.as_string())?;
			let end = child.get("end").and_then(|v| v.as_string())?;
			Some((start.to_string(), end.to_string()))
		})
	})
}

fn parse_language_servers_from_lang(children: Option<&KdlDocument>) -> Vec<String> {
	let Some(children) = children else {
		return Vec::new();
	};
	let Some(ls_node) = children.get("language-servers") else {
		return Vec::new();
	};

	let inline: Vec<String> = ls_node
		.entries()
		.iter()
		.filter(|e| e.name().is_none())
		.filter_map(|e| e.value().as_string())
		.map(String::from)
		.collect();

	if !inline.is_empty() {
		return inline;
	}

	let Some(ls_children) = ls_node.children() else {
		return Vec::new();
	};

	ls_children
		.nodes()
		.iter()
		.filter(|n| n.name().value() == "-")
		.filter_map(|n| n.get("name").and_then(|v| v.as_string()).map(String::from))
		.collect()
}
