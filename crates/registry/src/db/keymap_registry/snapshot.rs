use std::collections::HashMap;
use std::sync::Arc;

use xeno_keymap_core::parser::Node;
use xeno_keymap_core::{ContinuationEntry, MatchResult, Matcher};

use super::diagnostics::{KeymapBuildProblem, KeymapConflict};
use crate::actions::BindingMode;
use crate::core::ActionId;
use crate::invocation::Invocation;

/// A resolved runtime target for a compiled key binding.
#[derive(Debug, Clone)]
pub enum CompiledBindingTarget {
	/// Resolved action target.
	Action {
		id: ActionId,
		count: usize,
		extend: bool,
		register: Option<char>,
	},
	/// Structured invocation target.
	Invocation { inv: Invocation },
}

/// Runtime binding entry stored in the keymap trie.
#[derive(Debug, Clone)]
pub struct CompiledBinding {
	target: CompiledBindingTarget,
	name: Arc<str>,
	description: Arc<str>,
	short_desc: Arc<str>,
	keys: Vec<Node>,
}

impl CompiledBinding {
	pub(crate) fn new(target: CompiledBindingTarget, name: Arc<str>, description: Arc<str>, short_desc: Arc<str>, keys: Vec<Node>) -> Self {
		Self {
			target,
			name,
			description,
			short_desc,
			keys,
		}
	}

	pub fn target(&self) -> &CompiledBindingTarget {
		&self.target
	}

	pub fn name(&self) -> &str {
		&self.name
	}

	pub fn description(&self) -> &str {
		&self.description
	}

	pub fn short_desc(&self) -> &str {
		&self.short_desc
	}

	pub fn keys(&self) -> &[Node] {
		&self.keys
	}

	pub fn action_id(&self) -> Option<ActionId> {
		match &self.target {
			CompiledBindingTarget::Action { id, .. } => Some(*id),
			CompiledBindingTarget::Invocation { .. } => None,
		}
	}
}

/// Lookup result for key sequence matching.
#[derive(Debug)]
pub enum LookupOutcome<'a> {
	/// Complete match.
	Match(&'a CompiledBinding),
	/// Prefix match awaiting more keys.
	Pending { sticky: Option<&'a CompiledBinding> },
	/// No match.
	None,
}

#[derive(Debug, Clone)]
struct PrefixEntry {
	mode: BindingMode,
	keys: Arc<str>,
	description: Arc<str>,
}

/// Immutable runtime keymap snapshot used by input dispatch.
pub struct KeymapSnapshot {
	matchers: HashMap<BindingMode, Matcher<CompiledBinding>>,
	prefixes: Vec<PrefixEntry>,
	conflicts: Vec<KeymapConflict>,
	problems: Vec<KeymapBuildProblem>,
}

impl Default for KeymapSnapshot {
	fn default() -> Self {
		Self::new()
	}
}

impl KeymapSnapshot {
	pub fn new() -> Self {
		Self {
			matchers: HashMap::new(),
			prefixes: Vec::new(),
			conflicts: Vec::new(),
			problems: Vec::new(),
		}
	}

	pub(crate) fn from_parts(
		matchers: HashMap<BindingMode, Matcher<CompiledBinding>>,
		prefixes: Vec<(BindingMode, Arc<str>, Arc<str>)>,
		conflicts: Vec<KeymapConflict>,
		problems: Vec<KeymapBuildProblem>,
	) -> Self {
		Self {
			matchers,
			prefixes: prefixes
				.into_iter()
				.map(|(mode, keys, description)| PrefixEntry { mode, keys, description })
				.collect(),
			conflicts,
			problems,
		}
	}

	pub fn lookup(&self, mode: BindingMode, keys: &[Node]) -> LookupOutcome<'_> {
		let Some(matcher) = self.matchers.get(&mode) else {
			return LookupOutcome::None;
		};

		match matcher.lookup(keys) {
			MatchResult::Complete(entry) => LookupOutcome::Match(entry),
			MatchResult::Partial { has_value } => LookupOutcome::Pending { sticky: has_value },
			MatchResult::None => LookupOutcome::None,
		}
	}

	pub fn prefix_description(&self, mode: BindingMode, keys: &str) -> Option<&str> {
		self.prefixes.iter().find(|p| p.mode == mode && &*p.keys == keys).map(|p| &*p.description)
	}

	pub fn continuations_at(&self, mode: BindingMode, prefix: &[Node]) -> Vec<(&Node, Option<&CompiledBinding>)> {
		let Some(matcher) = self.matchers.get(&mode) else {
			return Vec::new();
		};
		matcher.continuations_at(prefix)
	}

	pub fn continuations_with_kind(&self, mode: BindingMode, prefix: &[Node]) -> Vec<ContinuationEntry<'_, CompiledBinding>> {
		let Some(matcher) = self.matchers.get(&mode) else {
			return Vec::new();
		};
		matcher.continuations_with_kind(prefix)
	}

	pub fn conflicts(&self) -> &[KeymapConflict] {
		&self.conflicts
	}

	pub fn problems(&self) -> &[KeymapBuildProblem] {
		&self.problems
	}
}
