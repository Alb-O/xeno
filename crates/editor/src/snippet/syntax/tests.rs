use super::*;

#[test]
fn parses_nested_placeholder_structure() {
	let template = parse_snippet_template("${1:foo ${2:bar}} baz").unwrap();
	assert_eq!(template.nodes.len(), 2);

	let Node::Field(outer) = &template.nodes[0] else {
		panic!("expected first node to be a field");
	};
	assert_eq!(outer.index, 1);

	let FieldKind::Placeholder(children) = &outer.kind else {
		panic!("expected placeholder children");
	};
	assert_eq!(children.len(), 2);
	assert_eq!(children[0], Node::Text("foo ".to_string()));

	let Node::Field(inner) = &children[1] else {
		panic!("expected nested field");
	};
	assert_eq!(inner.index, 2);
	let FieldKind::Placeholder(inner_children) = &inner.kind else {
		panic!("expected nested placeholder children");
	};
	assert_eq!(inner_children, &vec![Node::Text("bar".to_string())]);

	assert_eq!(template.nodes[1], Node::Text(" baz".to_string()));
}

#[test]
fn parses_braced_tabstop() {
	let template = parse_snippet_template("before ${7} after").unwrap();
	assert_eq!(template.nodes.len(), 3);

	let Node::Field(field) = &template.nodes[1] else {
		panic!("expected middle node field");
	};
	assert_eq!(field.index, 7);
	assert_eq!(field.kind, FieldKind::Tabstop);
}

#[test]
fn parses_choice_placeholder() {
	let template = parse_snippet_template("${1|a,b,c|}").unwrap();
	assert_eq!(template.nodes.len(), 1);
	let Node::Field(field) = &template.nodes[0] else {
		panic!("expected field node");
	};
	assert_eq!(field.index, 1);
	assert_eq!(field.kind, FieldKind::Choice(vec!["a".to_string(), "b".to_string(), "c".to_string()]));
}

#[test]
fn parses_choice_placeholder_with_escapes() {
	let template = parse_snippet_template(r"${1|a\,b,c\|d,e\\f|}").unwrap();
	let Node::Field(field) = &template.nodes[0] else {
		panic!("expected field node");
	};
	assert_eq!(field.kind, FieldKind::Choice(vec!["a,b".to_string(), "c|d".to_string(), "e\\f".to_string()]));
}

#[test]
fn parses_variable_unbraced() {
	let template = parse_snippet_template("$TM_FILENAME").unwrap();
	assert_eq!(
		template.nodes,
		vec![Node::Var(Var {
			name: "TM_FILENAME".to_string(),
			default: None,
		})]
	);
}

#[test]
fn parses_variable_braced() {
	let template = parse_snippet_template("${TM_FILENAME}").unwrap();
	assert_eq!(
		template.nodes,
		vec![Node::Var(Var {
			name: "TM_FILENAME".to_string(),
			default: None,
		})]
	);
}

#[test]
fn parses_variable_with_default_nodes() {
	let template = parse_snippet_template("${TM_FILENAME:${1:default}}").expect("variable with default should parse");
	let Node::Var(var) = &template.nodes[0] else {
		panic!("expected variable node");
	};
	assert_eq!(var.name, "TM_FILENAME");
	let default = var.default.as_ref().expect("default nodes expected");
	assert_eq!(default.len(), 1);
	assert!(matches!(default[0], Node::Field(_)));
}

#[test]
fn malformed_braced_transform_falls_back_to_literal_text() {
	let template = parse_snippet_template("${1/(.*)/$1").expect("malformed transform should not fail parsing");
	assert_eq!(template.nodes, vec![Node::Text("${1/(.*)/$1".to_string())]);
}

#[test]
fn parses_tabstop_transform() {
	let template = parse_snippet_template("${1/(.*)/$1_bar/gi}").expect("tabstop transform should parse");
	assert_eq!(template.nodes.len(), 1);
	let Node::Transform(transform) = &template.nodes[0] else {
		panic!("expected transform node");
	};
	assert_eq!(transform.source, TransformSource::Tabstop(1));
	assert_eq!(transform.regex, "(.*)");
	assert_eq!(transform.replace, "$1_bar");
	assert_eq!(transform.flags, "gi");
}

#[test]
fn parses_variable_transform() {
	let template = parse_snippet_template("${TM_FILENAME/(.*)/$1_x/}").expect("variable transform should parse");
	let Node::Transform(transform) = &template.nodes[0] else {
		panic!("expected transform node");
	};
	assert_eq!(transform.source, TransformSource::Var("TM_FILENAME".to_string()));
	assert_eq!(transform.regex, "(.*)");
	assert_eq!(transform.replace, "$1_x");
	assert_eq!(transform.flags, "");
}
