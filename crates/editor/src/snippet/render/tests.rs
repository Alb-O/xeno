use std::collections::HashMap;

use super::{SnippetVarResolver, render, render_with_resolver};
use crate::snippet::parse_snippet_template;

struct MapResolver {
	vars: HashMap<String, String>,
}

impl SnippetVarResolver for MapResolver {
	fn resolve_var(&self, name: &str) -> Option<String> {
		self.vars.get(name).cloned()
	}
}

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

#[test]
fn renders_choice_placeholder_with_default_and_choices() {
	let template = parse_snippet_template("${1|a,b,c|}").unwrap();
	let rendered = render(&template);

	assert_eq!(rendered.text, "a");
	assert_eq!(rendered.tabstops[&1], vec![0..1]);
	assert_eq!(rendered.choices[&1], vec!["a".to_string(), "b".to_string(), "c".to_string()]);
}

#[test]
fn renders_variable_default_when_unset() {
	let template = parse_snippet_template("${FOO:bar}").unwrap();
	let resolver = MapResolver { vars: HashMap::new() };
	let rendered = render_with_resolver(&template, &resolver);
	assert_eq!(rendered.text, "bar");
}

#[test]
fn renders_variable_value_when_set() {
	let template = parse_snippet_template("${FOO:bar}").unwrap();
	let resolver = MapResolver {
		vars: HashMap::from([("FOO".to_string(), "x".to_string())]),
	};
	let rendered = render_with_resolver(&template, &resolver);
	assert_eq!(rendered.text, "x");
}

#[test]
fn renders_variable_default_when_resolved_empty() {
	let template = parse_snippet_template("${FOO:bar}").unwrap();
	let resolver = MapResolver {
		vars: HashMap::from([("FOO".to_string(), String::new())]),
	};
	let rendered = render_with_resolver(&template, &resolver);
	assert_eq!(rendered.text, "bar");
}

#[test]
fn renders_variable_transform_with_resolver_value() {
	let template = parse_snippet_template("${FOO/(.*)/$1x/}").unwrap();
	let resolver = MapResolver {
		vars: HashMap::from([("FOO".to_string(), "a".to_string())]),
	};
	let rendered = render_with_resolver(&template, &resolver);
	assert_eq!(rendered.text, "ax");
	assert_eq!(rendered.transforms.len(), 1);
}

#[test]
fn renders_tabstop_transform_using_initial_placeholder_value() {
	let template = parse_snippet_template("${1:foo} ${1/(.*)/$1_bar/}").unwrap();
	let rendered = render(&template);
	assert_eq!(rendered.text, "foo foo_bar");
	assert_eq!(rendered.transforms.len(), 1);
	assert_eq!(rendered.transforms[0].range, 4..11);
}
