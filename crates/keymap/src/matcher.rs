//! A trie-based pattern matcher for sequences of input keys (`Node`s).
//!
//! This module provides the [`Matcher`] type, which maps sequences of key inputs to associated values.
//! It internally uses a recursive [`Trie`] structure to efficiently match input patterns.
//!
//! Patterns can include:
//!
//! 1. **Exact keys** — matches a specific input key (e.g., `Key::Char('a')`, `Key::F(1)`).
//! 2. **Character groups** — matches keys falling into categories like `@digit`, `@upper`, or `@any`,
//!    optionally with modifiers (e.g., `ctrl-@any`, `shift-@upper`).
//!
//! The matching logic follows a prioritized order:
//!
//! 1. **Exact match** — if the next input node exactly matches a key in the current trie level.
//! 2. **Group match** — if the next input character matches a character group and modifiers align.
//! 3. **Wildcard group match** — if the group is `@any` with matching modifiers.
//!
//! This ensures more specific patterns take precedence over broader ones.
//!
//! ## Example Patterns
//!
//! | Pattern                  | Input          | Match Result |
//! | ------------------------ | -------------- | ------------ |
//! | ctrl-\@any shift-\@upper | ctrl-x shift-B | true         |
//! | ctrl-\@any shift-\@upper | ctrl-x shift-3 | false        |
//! | a enter                  | a enter        | true         |
//! | a enter                  | a esc          | false        |
//! | @digit                   | '3'            | true         |
//! | @digit                   | 'a'            | false        |
//!
//! Each complete match path in the trie may store an associated value (e.g., action, ID, etc.).
//!
//! See [`Matcher`] for the main interface and [`Trie`] for the underlying structure.
//!
//! ## Hierarchical Key Sequences
//!
//! The matcher supports hierarchical key sequences for which-key style UIs. When querying
//! continuations at a prefix, each child is classified as either:
//!
//! - **Leaf**: A terminal binding (no further keys possible)
//! - **Branch**: A sub-prefix with more bindings underneath (e.g., `ctrl-w f` → focus commands)
//!
//! This allows UIs to show "…" indicators for branches that can be drilled into.
use std::collections::HashMap;

use xeno_keymap_parser::node::{CharGroup, Key, Node};

/// Result of looking up a key sequence in the matcher.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MatchResult<'a, T> {
	/// Complete match - the sequence matches a binding exactly.
	Complete(&'a T),
	/// Partial match - the sequence is a prefix of one or more bindings.
	/// Contains whether this prefix also has a value (for "sticky" behavior, e.g. "g" alone).
	Partial {
		/// Optional intermediate value if this prefix is itself a complete binding.
		has_value: Option<&'a T>,
	},
	/// No match - the sequence doesn't match any binding.
	None,
}

/// Classification of a continuation node in the key sequence trie.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ContinuationKind {
	/// Leaf node - pressing this key completes a binding with no further options.
	Leaf,
	/// Branch node - pressing this key reveals more options (sub-prefix).
	Branch,
}

/// A continuation entry returned when querying available next keys at a prefix.
#[derive(Debug, Clone)]
pub struct ContinuationEntry<'a, T> {
	/// The key that can be pressed next.
	pub key: &'a Node,
	/// The value at this node, if any (action for leaf, sticky action for branch).
	pub value: Option<&'a T>,
	/// Whether this is a leaf (terminal) or branch (has children).
	pub kind: ContinuationKind,
}

/// A prefix tree node for storing key bindings.
#[derive(Debug)]
struct Trie<T> {
	/// Value stored at this node if the sequence ending here is a complete binding.
	value: Option<T>,
	/// Children keyed by exact input nodes (specific keys like 'a', F1, etc.).
	exact: HashMap<Node, Trie<T>>,
	/// Children for group patterns (@digit, @upper, @any, etc.) checked in order.
	groups: Vec<(Node, Trie<T>)>,
}

impl<T> Trie<T> {
	/// Creates a new empty Trie node.
	fn new() -> Self {
		Self {
			value: None,
			exact: HashMap::new(),
			groups: Vec::new(),
		}
	}
}

/// A pattern matcher that maps sequences of `Node`s to values.
///
/// Supports both exact matches and grouped matches (e.g. `CharGroup::Upper`).
#[derive(Debug)]
pub struct Matcher<T> {
	/// Root node of the trie structure.
	root: Trie<T>,
}

impl<T> Default for Matcher<T> {
	fn default() -> Self {
		Self::new()
	}
}

impl<T> FromIterator<(Vec<Node>, T)> for Matcher<T> {
	fn from_iter<I: IntoIterator<Item = (Vec<Node>, T)>>(iter: I) -> Self {
		let mut matcher = Matcher::new();
		for (pattern, value) in iter {
			matcher.add(pattern, value);
		}
		matcher
	}
}

impl<T> Matcher<T> {
	/// Creates a new, empty matcher.
	pub fn new() -> Self {
		Self { root: Trie::new() }
	}

	/// Adds a pattern and its associated value to the matcher.
	pub fn add(&mut self, pattern: Vec<Node>, value: T) {
		let mut node = &mut self.root;

		for input_node in pattern {
			node = match input_node.key {
				Key::Group(_) => {
					// Look for an existing group node
					if let Some(pos) = node.groups.iter().position(|(n, _)| n == &input_node) {
						&mut node.groups[pos].1
					} else {
						node.groups.push((input_node, Trie::new()));
						&mut node.groups.last_mut().unwrap().1
					}
				}
				_ => node.exact.entry(input_node).or_insert_with(Trie::new),
			};
		}

		node.value = Some(value);
	}

	/// Attempts to retrieve a value for the given input node sequence.
	pub fn get(&self, nodes: &[Node]) -> Option<&T> {
		search(&self.root, nodes, 0)
	}

	/// Looks up a key sequence, returning detailed match information.
	///
	/// Returns:
	/// - `Complete(value)` if the sequence exactly matches a binding
	/// - `Partial { has_value }` if the sequence is a prefix of bindings (with optional intermediate value)
	/// - `None` if the sequence doesn't match anything
	pub fn lookup(&self, nodes: &[Node]) -> MatchResult<'_, T> {
		lookup_with_info(&self.root, nodes, 0)
	}

	/// Check if any bindings exist that start with the given prefix.
	pub fn has_prefix(&self, nodes: &[Node]) -> bool {
		!matches!(self.lookup(nodes), MatchResult::None)
	}

	/// Returns all direct children (continuations) at a given prefix.
	///
	/// Used for which-key style displays showing available next keys.
	pub fn continuations_at(&self, prefix: &[Node]) -> Vec<(&Node, Option<&T>)> {
		let Some(trie) = navigate_to(&self.root, prefix, 0) else {
			return Vec::new();
		};
		let exact = trie.exact.iter().map(|(k, v)| (k, v.value.as_ref()));
		let groups = trie.groups.iter().map(|(k, v)| (k, v.value.as_ref()));
		exact.chain(groups).collect()
	}

	/// Returns continuations with classification (leaf vs branch).
	///
	/// Each continuation is classified as:
	/// - `Leaf`: Terminal binding with no further children
	/// - `Branch`: Sub-prefix with more bindings underneath
	///
	/// This enables which-key UIs to show "…" for branches that can be drilled into.
	pub fn continuations_with_kind(&self, prefix: &[Node]) -> Vec<ContinuationEntry<'_, T>> {
		let Some(trie) = navigate_to(&self.root, prefix, 0) else {
			return Vec::new();
		};

		let exact = trie.exact.iter().map(|(key, child)| {
			let has_children = !child.exact.is_empty() || !child.groups.is_empty();
			ContinuationEntry {
				key,
				value: child.value.as_ref(),
				kind: if has_children {
					ContinuationKind::Branch
				} else {
					ContinuationKind::Leaf
				},
			}
		});

		let groups = trie.groups.iter().map(|(key, child)| {
			let has_children = !child.exact.is_empty() || !child.groups.is_empty();
			ContinuationEntry {
				key,
				value: child.value.as_ref(),
				kind: if has_children {
					ContinuationKind::Branch
				} else {
					ContinuationKind::Leaf
				},
			}
		});

		exact.chain(groups).collect()
	}
}

/// Navigates to the trie node at the given prefix, or None if prefix doesn't exist.
fn navigate_to<'a, T>(node: &'a Trie<T>, nodes: &[Node], pos: usize) -> Option<&'a Trie<T>> {
	if pos == nodes.len() {
		return Some(node);
	}

	let input_node = &nodes[pos];

	if let Some(child) = node.exact.get(input_node) {
		return navigate_to(child, nodes, pos + 1);
	}

	if let Key::Char(ch) = input_node.key {
		for (n, child) in &node.groups {
			if let Key::Group(group) = n.key
				&& n.modifiers == input_node.modifiers
				&& group.matches(ch)
			{
				return navigate_to(child, nodes, pos + 1);
			}
		}
	}

	for (n, child) in &node.groups {
		if matches!(n.key, Key::Group(CharGroup::Any)) {
			return navigate_to(child, nodes, pos + 1);
		}
	}

	None
}

/// Recursively searches the Trie for a matching value.
///
/// Priority order:
/// 1. Exact match
/// 2. Group match with same modifiers
/// 3. Any-char group match with same modifiers
fn search<'a, T>(node: &'a Trie<T>, nodes: &[Node], pos: usize) -> Option<&'a T> {
	if pos == nodes.len() {
		return node.value.as_ref();
	}

	let input_node = &nodes[pos];

	if let Some(result) = node
		.exact
		.get(input_node)
		.and_then(|child| search(child, nodes, pos + 1))
	{
		return Some(result);
	}

	if let Key::Char(ch) = input_node.key
		&& let Some(result) = node.groups.iter().find_map(|(n, child)| match n.key {
			Key::Group(group) if n.modifiers == input_node.modifiers && group.matches(ch) => {
				search(child, nodes, pos + 1)
			}
			_ => None,
		}) {
		return Some(result);
	}

	node.groups.iter().find_map(|(n, child)| {
		if matches!(n.key, Key::Group(CharGroup::Any)) {
			search(child, nodes, pos + 1)
		} else {
			None
		}
	})
}

/// Looks up a key sequence with detailed match information.
fn lookup_with_info<'a, T>(node: &'a Trie<T>, nodes: &[Node], pos: usize) -> MatchResult<'a, T> {
	if pos == nodes.len() {
		// We've consumed all input nodes, check what we have at this position
		let has_children = !node.exact.is_empty() || !node.groups.is_empty();

		return if has_children {
			// More keys possible - partial match
			MatchResult::Partial {
				has_value: node.value.as_ref(),
			}
		} else if let Some(val) = node.value.as_ref() {
			// No more keys possible, but we have a value - complete match
			MatchResult::Complete(val)
		} else {
			// No value and no children - shouldn't happen in well-formed trie
			MatchResult::None
		};
	}

	let input_node = &nodes[pos];

	if let Some(child) = node.exact.get(input_node) {
		let result = lookup_with_info(child, nodes, pos + 1);
		if !matches!(result, MatchResult::None) {
			return result;
		}
	}

	if let Key::Char(ch) = input_node.key {
		for (n, child) in &node.groups {
			if let Key::Group(group) = n.key
				&& n.modifiers == input_node.modifiers
				&& group.matches(ch)
			{
				let result = lookup_with_info(child, nodes, pos + 1);
				if !matches!(result, MatchResult::None) {
					return result;
				}
			}
		}
	}

	for (n, child) in &node.groups {
		if matches!(n.key, Key::Group(CharGroup::Any)) {
			let result = lookup_with_info(child, nodes, pos + 1);
			if !matches!(result, MatchResult::None) {
				return result;
			}
		}
	}

	MatchResult::None
}

#[cfg(test)]
mod tests {
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
	fn test_exact_nodes() {
		matches(&[
			("a", "a", true),
			("ctrl-c", "ctrl-c", true),
			("f12", "f12", true),
			("f10", "f11", false),
			("enter", "enter", true),
		]);
	}

	#[test]
	fn test_groups() {
		matches(&[
			("@upper", "A", true),
			("@digit", "1", true),
			("ctrl-@any", "ctrl-x", true),
			("@any", "b", true),
			("a", "a", true), // Exact match has highest priority
		]);
	}

	#[test]
	fn test_sequences() {
		matches(&[
			("a enter", "a enter", true),
			("ctrl-@any shift-@upper", "ctrl-x shift-B", true),
		]);
	}

	#[test]
	fn test_lookup_complete() {
		let mut matcher = Matcher::new();
		matcher.add(parse_seq("a").unwrap(), 1);
		matcher.add(parse_seq("b").unwrap(), 2);

		assert_eq!(
			matcher.lookup(&parse_seq("a").unwrap()),
			MatchResult::Complete(&1)
		);
		assert_eq!(
			matcher.lookup(&parse_seq("b").unwrap()),
			MatchResult::Complete(&2)
		);
	}

	#[test]
	fn test_lookup_partial() {
		let mut matcher = Matcher::new();
		matcher.add(parse_seq("g g").unwrap(), 1);
		matcher.add(parse_seq("g j").unwrap(), 2);

		// "g" alone is a partial match
		match matcher.lookup(&parse_seq("g").unwrap()) {
			MatchResult::Partial { has_value: None } => {}
			other => panic!("Expected Partial without value, got {other:?}"),
		}

		// "g g" is complete
		assert_eq!(
			matcher.lookup(&parse_seq("g g").unwrap()),
			MatchResult::Complete(&1)
		);
	}

	#[test]
	fn test_lookup_partial_with_value() {
		let mut matcher = Matcher::new();
		matcher.add(parse_seq("g").unwrap(), 1); // "g" alone does something
		matcher.add(parse_seq("g g").unwrap(), 2); // "g g" does something else

		// "g" is partial but also has a value (sticky mode behavior)
		match matcher.lookup(&parse_seq("g").unwrap()) {
			MatchResult::Partial {
				has_value: Some(&1),
			} => {}
			other => panic!("Expected Partial with value 1, got {other:?}"),
		}
	}

	#[test]
	fn test_lookup_none() {
		let mut matcher = Matcher::new();
		matcher.add(parse_seq("a").unwrap(), 1);

		assert_eq!(matcher.lookup(&parse_seq("x").unwrap()), MatchResult::None);
		assert_eq!(
			matcher.lookup(&parse_seq("a b").unwrap()),
			MatchResult::None
		);
	}

	#[test]
	fn test_continuations_with_kind_leaf() {
		let mut matcher = Matcher::new();
		matcher.add(parse_seq("g h").unwrap(), 1);
		matcher.add(parse_seq("g j").unwrap(), 2);

		let conts = matcher.continuations_with_kind(&parse_seq("g").unwrap());
		assert_eq!(conts.len(), 2);

		for cont in &conts {
			assert_eq!(
				cont.kind,
				ContinuationKind::Leaf,
				"Expected leaf for terminal binding"
			);
			assert!(cont.value.is_some(), "Leaf should have a value");
		}
	}

	#[test]
	fn test_continuations_with_kind_branch() {
		let mut matcher = Matcher::new();
		matcher.add(parse_seq("ctrl-w f h").unwrap(), 1);
		matcher.add(parse_seq("ctrl-w f j").unwrap(), 2);
		matcher.add(parse_seq("ctrl-w s").unwrap(), 3);

		let conts = matcher.continuations_with_kind(&parse_seq("ctrl-w").unwrap());
		assert_eq!(conts.len(), 2);

		let f_cont = conts.iter().find(|c| format!("{}", c.key) == "f").unwrap();
		let s_cont = conts.iter().find(|c| format!("{}", c.key) == "s").unwrap();

		assert_eq!(
			f_cont.kind,
			ContinuationKind::Branch,
			"'f' should be a branch (has children)"
		);
		assert!(f_cont.value.is_none(), "Branch without sticky action");

		assert_eq!(
			s_cont.kind,
			ContinuationKind::Leaf,
			"'s' should be a leaf (no children)"
		);
		assert!(s_cont.value.is_some(), "Leaf should have a value");
	}

	#[test]
	fn test_continuations_with_kind_branch_with_sticky() {
		let mut matcher = Matcher::new();
		matcher.add(parse_seq("g").unwrap(), 1);
		matcher.add(parse_seq("g g").unwrap(), 2);

		let conts = matcher.continuations_with_kind(&[]);
		let g_cont = conts.iter().find(|c| format!("{}", c.key) == "g").unwrap();

		assert_eq!(
			g_cont.kind,
			ContinuationKind::Branch,
			"'g' is a branch (has 'g g' child)"
		);
		assert_eq!(g_cont.value, Some(&1), "'g' has sticky value");
	}
}
