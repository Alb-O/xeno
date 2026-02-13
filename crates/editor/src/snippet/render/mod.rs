use std::collections::BTreeMap;
use std::ops::Range;

use regex::RegexBuilder;

use super::syntax::{FieldKind, Node, SnippetTemplate, TransformSource, Var};

pub trait SnippetVarResolver {
	fn resolve_var(&self, name: &str) -> Option<String>;
}

struct NoVars;

impl SnippetVarResolver for NoVars {
	fn resolve_var(&self, _name: &str) -> Option<String> {
		None
	}
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RenderedSnippet {
	pub text: String,
	pub tabstops: BTreeMap<u32, Vec<Range<usize>>>,
	pub choices: BTreeMap<u32, Vec<String>>,
	pub transforms: Vec<RenderedTransform>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RenderedTransform {
	pub source: TransformSource,
	pub regex: String,
	pub replace: String,
	pub flags: String,
	pub range: Range<usize>,
}

pub fn render(template: &SnippetTemplate) -> RenderedSnippet {
	render_with_resolver(template, &NoVars)
}

pub fn render_with_resolver<R>(template: &SnippetTemplate, resolver: &R) -> RenderedSnippet
where
	R: SnippetVarResolver + ?Sized,
{
	let mut text = String::new();
	let mut tabstops: BTreeMap<u32, Vec<Range<usize>>> = BTreeMap::new();
	let mut choices: BTreeMap<u32, Vec<String>> = BTreeMap::new();
	let mut transforms: Vec<RenderedTransform> = Vec::new();
	let mut tabstop_sources: BTreeMap<u32, String> = BTreeMap::new();
	let mut out_chars = 0usize;
	collect_tabstop_sources(&template.nodes, resolver, &mut tabstop_sources);

	render_nodes(
		&template.nodes,
		&mut text,
		&mut out_chars,
		&mut tabstops,
		&mut choices,
		&mut transforms,
		&tabstop_sources,
		resolver,
	);

	RenderedSnippet {
		text,
		tabstops,
		choices,
		transforms,
	}
}

fn render_nodes<R>(
	nodes: &[Node],
	out: &mut String,
	out_chars: &mut usize,
	tabstops: &mut BTreeMap<u32, Vec<Range<usize>>>,
	choices: &mut BTreeMap<u32, Vec<String>>,
	transforms: &mut Vec<RenderedTransform>,
	tabstop_sources: &BTreeMap<u32, String>,
	resolver: &R,
) where
	R: SnippetVarResolver + ?Sized,
{
	for node in nodes {
		match node {
			Node::Text(text) => {
				out.push_str(text);
				*out_chars = out_chars.saturating_add(text.chars().count());
			}
			Node::Field(field) => match &field.kind {
				FieldKind::Tabstop => {
					tabstops.entry(field.index).or_default().push(*out_chars..*out_chars);
				}
				FieldKind::Placeholder(children) => {
					let start = *out_chars;
					render_nodes(children, out, out_chars, tabstops, choices, transforms, tabstop_sources, resolver);
					tabstops.entry(field.index).or_default().push(start..*out_chars);
				}
				FieldKind::Choice(options) => {
					let start = *out_chars;
					let selected = options.first().cloned().unwrap_or_default();
					*out_chars = out_chars.saturating_add(selected.chars().count());
					out.push_str(&selected);
					tabstops.entry(field.index).or_default().push(start..*out_chars);
					choices.entry(field.index).or_insert_with(|| options.clone());
				}
			},
			Node::Var(var) => render_var_node(var, out, out_chars, tabstops, choices, transforms, tabstop_sources, resolver),
			Node::Transform(transform) => {
				let source_value = match &transform.source {
					TransformSource::Tabstop(index) => tabstop_sources.get(index).cloned().unwrap_or_default(),
					TransformSource::Var(name) => resolver.resolve_var(name).unwrap_or_default(),
				};
				let output = apply_transform(&source_value, &transform.regex, &transform.replace, &transform.flags);
				let start = *out_chars;
				*out_chars = out_chars.saturating_add(output.chars().count());
				out.push_str(&output);
				transforms.push(RenderedTransform {
					source: transform.source.clone(),
					regex: transform.regex.clone(),
					replace: transform.replace.clone(),
					flags: transform.flags.clone(),
					range: start..*out_chars,
				});
			}
		}
	}
}

fn render_var_node<R>(
	var: &Var,
	out: &mut String,
	out_chars: &mut usize,
	tabstops: &mut BTreeMap<u32, Vec<Range<usize>>>,
	choices: &mut BTreeMap<u32, Vec<String>>,
	transforms: &mut Vec<RenderedTransform>,
	tabstop_sources: &BTreeMap<u32, String>,
	resolver: &R,
) where
	R: SnippetVarResolver + ?Sized,
{
	if let Some(value) = resolver.resolve_var(&var.name)
		&& !value.is_empty()
	{
		*out_chars = out_chars.saturating_add(value.chars().count());
		out.push_str(&value);
		return;
	}

	if let Some(default) = &var.default {
		render_nodes(default, out, out_chars, tabstops, choices, transforms, tabstop_sources, resolver);
	}
}

fn collect_tabstop_sources<R>(nodes: &[Node], resolver: &R, tabstop_sources: &mut BTreeMap<u32, String>)
where
	R: SnippetVarResolver + ?Sized,
{
	for node in nodes {
		let Node::Field(field) = node else {
			continue;
		};

		tabstop_sources.entry(field.index).or_insert_with(|| match &field.kind {
			FieldKind::Tabstop => String::new(),
			FieldKind::Placeholder(children) => render_plain_nodes(children, resolver),
			FieldKind::Choice(options) => options.first().cloned().unwrap_or_default(),
		});

		if let FieldKind::Placeholder(children) = &field.kind {
			collect_tabstop_sources(children, resolver, tabstop_sources);
		}
	}
}

fn render_plain_nodes<R>(nodes: &[Node], resolver: &R) -> String
where
	R: SnippetVarResolver + ?Sized,
{
	let mut out = String::new();
	for node in nodes {
		match node {
			Node::Text(text) => out.push_str(text),
			Node::Field(field) => match &field.kind {
				FieldKind::Tabstop => {}
				FieldKind::Placeholder(children) => out.push_str(&render_plain_nodes(children, resolver)),
				FieldKind::Choice(options) => out.push_str(options.first().map(String::as_str).unwrap_or_default()),
			},
			Node::Var(var) => {
				if let Some(value) = resolver.resolve_var(&var.name)
					&& !value.is_empty()
				{
					out.push_str(&value);
				} else if let Some(default) = &var.default {
					out.push_str(&render_plain_nodes(default, resolver));
				}
			}
			Node::Transform(_) => {}
		}
	}
	out
}

pub(crate) fn apply_transform(source: &str, regex: &str, replace: &str, flags: &str) -> String {
	let mut builder = RegexBuilder::new(regex);
	builder.case_insensitive(flags.contains('i'));
	builder.multi_line(flags.contains('m'));
	builder.dot_matches_new_line(flags.contains('s'));
	let Ok(compiled) = builder.build() else {
		return source.to_string();
	};
	let replacement = normalize_transform_replacement(replace);

	if flags.contains('g') {
		compiled.replace_all(source, replacement.as_str()).to_string()
	} else {
		compiled.replace(source, replacement.as_str()).to_string()
	}
}

fn normalize_transform_replacement(replace: &str) -> String {
	let chars: Vec<char> = replace.chars().collect();
	let mut out = String::new();
	let mut i = 0usize;

	while i < chars.len() {
		if chars[i] != '$' {
			out.push(chars[i]);
			i += 1;
			continue;
		}

		if i + 1 >= chars.len() {
			out.push('$');
			i += 1;
			continue;
		}

		if chars[i + 1].is_ascii_digit() {
			let mut j = i + 1;
			while j < chars.len() && chars[j].is_ascii_digit() {
				j += 1;
			}
			out.push_str("${");
			for ch in &chars[i + 1..j] {
				out.push(*ch);
			}
			out.push('}');
			i = j;
			continue;
		}

		out.push('$');
		i += 1;
	}

	out
}

#[cfg(test)]
mod tests;
