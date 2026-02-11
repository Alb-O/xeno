use xeno_keymap_parser::parse_seq;

use super::*;

fn matches(inputs: &[(&'static str, &'static str, bool)]) {
	let items = inputs
		.iter()
		.enumerate()
		.map(|(i, (keys, _, _))| (parse_seq(keys).unwrap(), i))
		.collect::<Vec<_>>();

	let matcher = Matcher::from_iter(items);
	inputs.iter().enumerate().for_each(|(i, (_, v, pass))| {
		let key = parse_seq(v).unwrap();
		let result = matcher.get(&key);

		if *pass {
			assert_eq!(result, Some(i).as_ref(), "{key:?}");
		} else {
			assert_eq!(result, None);
		}
	});
}

#[test]
fn exact_nodes() {
	matches(&[
		("a", "a", true),
		("ctrl-c", "ctrl-c", true),
		("f12", "f12", true),
		("f10", "f11", false),
		("enter", "enter", true),
	]);
}

#[test]
fn groups() {
	matches(&[
		("@upper", "A", true),
		("@digit", "1", true),
		("ctrl-@any", "ctrl-x", true),
		("@any", "b", true),
		("a", "a", true), // Exact match has highest priority
	]);
}

#[test]
fn sequences() {
	matches(&[("a enter", "a enter", true), ("ctrl-@any shift-@upper", "ctrl-x shift-B", true)]);
}

#[test]
fn lookup_complete() {
	let mut matcher = Matcher::new();
	matcher.add(parse_seq("a").unwrap(), 1);
	matcher.add(parse_seq("b").unwrap(), 2);

	assert_eq!(matcher.lookup(&parse_seq("a").unwrap()), MatchResult::Complete(&1));
	assert_eq!(matcher.lookup(&parse_seq("b").unwrap()), MatchResult::Complete(&2));
}

#[test]
fn lookup_partial() {
	let mut matcher = Matcher::new();
	matcher.add(parse_seq("g g").unwrap(), 1);
	matcher.add(parse_seq("g j").unwrap(), 2);

	// "g" alone is a partial match
	match matcher.lookup(&parse_seq("g").unwrap()) {
		MatchResult::Partial { has_value: None } => {}
		other => panic!("Expected Partial without value, got {other:?}"),
	}

	// "g g" is complete
	assert_eq!(matcher.lookup(&parse_seq("g g").unwrap()), MatchResult::Complete(&1));
}

#[test]
fn lookup_partial_with_value() {
	let mut matcher = Matcher::new();
	matcher.add(parse_seq("g").unwrap(), 1); // "g" alone does something
	matcher.add(parse_seq("g g").unwrap(), 2); // "g g" does something else

	// "g" is partial but also has a value (sticky mode behavior)
	match matcher.lookup(&parse_seq("g").unwrap()) {
		MatchResult::Partial { has_value: Some(&1) } => {}
		other => panic!("Expected Partial with value 1, got {other:?}"),
	}
}

#[test]
fn lookup_none() {
	let mut matcher = Matcher::new();
	matcher.add(parse_seq("a").unwrap(), 1);

	assert_eq!(matcher.lookup(&parse_seq("x").unwrap()), MatchResult::None);
	assert_eq!(matcher.lookup(&parse_seq("a b").unwrap()), MatchResult::None);
}

#[test]
fn continuations_with_kind_leaf() {
	let mut matcher = Matcher::new();
	matcher.add(parse_seq("g h").unwrap(), 1);
	matcher.add(parse_seq("g j").unwrap(), 2);

	let conts = matcher.continuations_with_kind(&parse_seq("g").unwrap());
	assert_eq!(conts.len(), 2);

	for cont in &conts {
		assert_eq!(cont.kind, ContinuationKind::Leaf, "Expected leaf for terminal binding");
		assert!(cont.value.is_some(), "Leaf should have a value");
	}
}

#[test]
fn continuations_with_kind_branch() {
	let mut matcher = Matcher::new();
	matcher.add(parse_seq("ctrl-w f h").unwrap(), 1);
	matcher.add(parse_seq("ctrl-w f j").unwrap(), 2);
	matcher.add(parse_seq("ctrl-w s").unwrap(), 3);

	let conts = matcher.continuations_with_kind(&parse_seq("ctrl-w").unwrap());
	assert_eq!(conts.len(), 2);

	let f_cont = conts.iter().find(|c| format!("{}", c.key) == "f").unwrap();
	let s_cont = conts.iter().find(|c| format!("{}", c.key) == "s").unwrap();

	assert_eq!(f_cont.kind, ContinuationKind::Branch, "'f' should be a branch (has children)");
	assert!(f_cont.value.is_none(), "Branch without sticky action");

	assert_eq!(s_cont.kind, ContinuationKind::Leaf, "'s' should be a leaf (no children)");
	assert!(s_cont.value.is_some(), "Leaf should have a value");
}

#[test]
fn continuations_with_kind_branch_with_sticky() {
	let mut matcher = Matcher::new();
	matcher.add(parse_seq("g").unwrap(), 1);
	matcher.add(parse_seq("g g").unwrap(), 2);

	let conts = matcher.continuations_with_kind(&[]);
	let g_cont = conts.iter().find(|c| format!("{}", c.key) == "g").unwrap();

	assert_eq!(g_cont.kind, ContinuationKind::Branch, "'g' is a branch (has 'g g' child)");
	assert_eq!(g_cont.value, Some(&1), "'g' has sticky value");
}
