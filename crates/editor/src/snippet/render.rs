use std::collections::BTreeMap;
use std::ops::Range;

use super::syntax::{FieldKind, Node, SnippetTemplate};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RenderedSnippet {
	pub text: String,
	pub tabstops: BTreeMap<u32, Vec<Range<usize>>>,
}

pub fn render(template: &SnippetTemplate) -> RenderedSnippet {
	let mut text = String::new();
	let mut tabstops: BTreeMap<u32, Vec<Range<usize>>> = BTreeMap::new();
	let mut out_chars = 0usize;

	render_nodes(&template.nodes, &mut text, &mut out_chars, &mut tabstops);

	RenderedSnippet { text, tabstops }
}

fn render_nodes(nodes: &[Node], out: &mut String, out_chars: &mut usize, tabstops: &mut BTreeMap<u32, Vec<Range<usize>>>) {
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
					render_nodes(children, out, out_chars, tabstops);
					tabstops.entry(field.index).or_default().push(start..*out_chars);
				}
			},
		}
	}
}

#[cfg(test)]
mod tests {
	use super::render;
	use crate::snippet::parse_snippet_template;

	#[test]
	fn renders_simple_tabstops() {
		let template = parse_snippet_template("foo $1 bar $0").unwrap();
		let rendered = render(&template);

		assert_eq!(rendered.text, "foo  bar ");
		assert_eq!(rendered.tabstops[&1], vec![4..4]);
		assert_eq!(rendered.tabstops[&0], vec![9..9]);
	}

	#[test]
	fn renders_placeholder_ranges() {
		let template = parse_snippet_template("let ${1:name} = ${2:val};").unwrap();
		let rendered = render(&template);

		assert_eq!(rendered.text, "let name = val;");
		assert_eq!(rendered.tabstops[&1], vec![4..8]);
		assert_eq!(rendered.tabstops[&2], vec![11..14]);
	}

	#[test]
	fn renders_nested_placeholder_ranges() {
		let template = parse_snippet_template("${1:foo ${2:bar}} baz").unwrap();
		let rendered = render(&template);

		assert_eq!(rendered.text, "foo bar baz");
		assert_eq!(rendered.tabstops[&1], vec![0..7]);
		assert_eq!(rendered.tabstops[&2], vec![4..7]);
	}

	#[test]
	fn renders_escaped_literals() {
		let template = parse_snippet_template(r"\$1 \} $$").unwrap();
		let rendered = render(&template);

		assert_eq!(rendered.text, "$1 } $");
		assert!(rendered.tabstops.is_empty());
	}
}
