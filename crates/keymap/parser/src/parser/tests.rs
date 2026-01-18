use serde::Deserialize;

use super::{ParseError, parse};
use crate::parser::{CharGroup, Key, Modifier, Node};

#[test]
fn test_parse() {
	let err = |message: &str, position: usize| {
		Err::<Node, ParseError>(ParseError {
			message: message.to_string(),
			position,
		})
	};

	for (input, result) in [
		("alt-f", Ok(Node::new(Modifier::Alt as u8, Key::Char('f')))),
		("space", Ok(Node::new(0, Key::Space))),
		("delta", err("expect end of input, found: e", 1)),
		(
			"shift-a",
			Ok(Node::new(Modifier::Shift as u8, Key::Char('a'))),
		),
		("shift-a-delete", err("expect end of input, found: -", 7)),
		("al", err("expect end of input, found: l", 1)),
	] {
		let output = parse(input);
		assert_eq!(result, output);
	}
}

#[test]
fn test_parse_seq() {
	for (s, v) in [
		("ctrl-b", Ok(vec![parse("ctrl-b").unwrap()])),
		(
			"ctrl-b l",
			Ok(vec![parse("ctrl-b").unwrap(), parse("l").unwrap()]),
		),
		("ctrl-b -l", Err(parse("-l").unwrap_err())), // Invalid: dangling separator
	] {
		assert_eq!(super::parse_seq(s), v);
	}
}

#[test]
fn test_parse_fn_key() {
	// Valid function key numbers: f0 - f12
	(0..=12).for_each(|n| {
		let input = format!("f{n}");
		let result = parse(&input);
		assert_eq!(Key::F(n), result.unwrap().key);
	});

	// Invalid: above f12
	for n in [13, 15] {
		let input = format!("f{n}");
		let result = parse(&input);
		assert!(result.is_err());
	}
}

#[test]
fn test_parse_enum() {
	// Check named keys
	for (s, key) in [("up", Key::Up), ("esc", Key::Esc), ("del", Key::Delete)] {
		let result = parse(s);
		assert_eq!(result.unwrap().key, key);
	}
}

#[test]
fn test_parse_char_groups() {
	for (input, expected_key) in [
		("@digit", Key::Group(CharGroup::Digit)),
		("@lower", Key::Group(CharGroup::Lower)),
		("@upper", Key::Group(CharGroup::Upper)),
		("@alpha", Key::Group(CharGroup::Alpha)),
		("@alnum", Key::Group(CharGroup::Alnum)),
		("@any", Key::Group(CharGroup::Any)),
	] {
		let result = parse(input);
		assert_eq!(result.unwrap().key, expected_key);
	}

	// Test invalid group names
	let result = parse("@invalid");
	assert!(result.is_err());
	assert!(
		result
			.unwrap_err()
			.message
			.contains("unknown char group: '@invalid'")
	);

	// Test incomplete group syntax
	let result = parse("@x");
	assert!(result.is_err());
	assert!(
		result
			.unwrap_err()
			.message
			.contains("unknown char group: '@x'")
	);
}

#[test]
fn test_format() {
	for (node, expected) in [
		(Node::new(0, Key::F(3)), "f3"),
		(Node::new(0, Key::Delete), "delete"),
		(Node::new(0, Key::Space), "space"),
		(Node::new(0, Key::Char('g')), "g"),
		(Node::new(0, Key::Char('#')), "#"),
		(Node::new(0, Key::Group(CharGroup::Digit)), "@digit"),
		(Node::new(0, Key::Group(CharGroup::Lower)), "@lower"),
		(Node::new(Modifier::Alt as u8, Key::Char('f')), "alt-f"),
		(
			Node::new(Modifier::Alt as u8, Key::Group(CharGroup::Alpha)),
			"alt-@alpha",
		),
		(
			Node::new(Modifier::Shift as u8 | Modifier::Cmd as u8, Key::Char('f')),
			"cmd-shift-f",
		),
	] {
		assert_eq!(expected, format!("{node}"));
	}
}

#[test]
fn test_deserialize() {
	use std::collections::HashMap;

	#[derive(Deserialize, Debug)]
	struct Test {
		keys: HashMap<Node, String>,
	}

	let result: Test = toml::from_str(
		r#"
[keys]
alt-d = "a"
cmd-shift-del = "b"
shift-cmd-del = "b" # this is the same as previous one
delete = "d"
"@digit" = "number"
"alt-@lower" = "alt-lowercase"
    "#,
	)
	.unwrap();

	for n in [
		Node::new(Modifier::Alt as u8, Key::Char('d')),
		Node::new(Modifier::Cmd as u8 | Modifier::Shift as u8, Key::Delete),
		Node::new(0, Key::Delete),
		Node::new(0, Key::Group(CharGroup::Digit)),
		Node::new(Modifier::Alt as u8, Key::Group(CharGroup::Lower)),
	] {
		let (key, _) = result.keys.get_key_value(&n).unwrap();
		assert_eq!(key, &n);
	}
}

#[test]
fn test_parse_str() {
	[
		(Node::new(0, Key::F(3)), "f3"),
		(Node::new(0, Key::Delete), "delete"),
		(Node::new(0, Key::Space), "space"),
	]
	.iter()
	.for_each(|(expected, input)| {
		let node = input.parse::<Node>().unwrap();
		assert_eq!(expected, &node);
	});
}
