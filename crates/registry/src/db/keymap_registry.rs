//! Unified keymap registry using trie-based matching.

use std::collections::HashMap;
use std::sync::Arc;

use tracing::warn;
use xeno_keymap_core::parser::{Node, parse_seq};
pub use xeno_keymap_core::{ContinuationEntry, ContinuationKind};
use xeno_keymap_core::{MatchResult, Matcher};

use crate::actions::{ActionEntry, BindingMode, KeyBindingDef};
use crate::core::{ActionId, RegistryEntry, RegistryIndex};

/// Binding entry storing action info and the key sequence.
#[derive(Debug, Clone)]
pub struct BindingEntry {
	/// Resolved action ID for dispatch.
	pub action_id: ActionId,
	/// Action name (for display/debugging).
	pub action_name: String,
	/// Human-readable description for UI display.
	pub description: String,
	/// Short description without key-sequence prefix (for which-key HUD).
	pub short_desc: String,
	/// Key sequence that triggers this binding (for display).
	pub keys: Vec<Node>,
}

/// Result of looking up a key sequence.
#[derive(Debug)]
pub enum LookupResult<'a> {
	/// Complete match - execute this action.
	Match(&'a BindingEntry),
	/// Partial match - wait for more keys. May have sticky action.
	Pending {
		/// If Some, this action can be executed (sticky mode behavior).
		sticky: Option<&'a BindingEntry>,
	},
	/// No match found.
	None,
}

/// Registry of keybindings organized by mode.
pub struct KeymapRegistry {
	/// Per-mode trie matchers for key sequences.
	matchers: HashMap<BindingMode, Matcher<BindingEntry>>,
	conflicts: Vec<KeymapConflict>,
}

#[derive(Debug, Clone)]
pub struct KeymapConflict {
	pub mode: BindingMode,
	pub keys: Arc<str>,
	pub kept_action: String,
	pub dropped_action: String,
	pub kept_priority: i16,
	pub dropped_priority: i16,
}

impl Default for KeymapRegistry {
	fn default() -> Self {
		Self::new()
	}
}

impl KeymapRegistry {
	/// Creates a new empty registry.
	pub fn new() -> Self {
		Self {
			matchers: HashMap::new(),
			conflicts: Vec::new(),
		}
	}

	/// Adds a binding to the registry.
	pub fn add(&mut self, mode: BindingMode, keys: Vec<Node>, entry: BindingEntry) {
		self.matchers.entry(mode).or_default().add(keys, entry);
	}

	/// Looks up a key sequence in the given mode.
	pub fn lookup(&self, mode: BindingMode, keys: &[Node]) -> LookupResult<'_> {
		let Some(matcher) = self.matchers.get(&mode) else {
			return LookupResult::None;
		};

		match matcher.lookup(keys) {
			MatchResult::Complete(entry) => LookupResult::Match(entry),
			MatchResult::Partial { has_value } => LookupResult::Pending { sticky: has_value },
			MatchResult::None => LookupResult::None,
		}
	}

	/// Build registry from action index + builtin keybindings.
	pub fn build(
		actions: &RegistryIndex<ActionEntry, ActionId>,
		bindings: &[KeyBindingDef],
	) -> Self {
		let mut registry = Self::new();
		let mut sorted: Vec<KeyBindingDef> = bindings.to_vec();
		sorted.sort_by(|a, b| {
			a.mode
				.cmp(&b.mode)
				.then_with(|| a.keys.cmp(&b.keys))
				.then_with(|| a.priority.cmp(&b.priority))
				.then_with(|| a.action.cmp(&b.action))
		});

		let mut seen: HashMap<(BindingMode, Arc<str>), (String, i16)> = HashMap::new();
		let mut unknown_actions: Vec<(Arc<str>, BindingMode)> = Vec::new();
		let mut parse_failures: Vec<(Arc<str>, Arc<str>)> = Vec::new();

		for def in sorted {
			let Some(action_entry) = actions.get(&def.action) else {
				if unknown_actions.len() < 5 {
					unknown_actions.push((Arc::clone(&def.action), def.mode));
				}
				continue;
			};

			let Some(&action_id) = actions.by_key.get(&action_entry.id()) else {
				if unknown_actions.len() < 5 {
					unknown_actions.push((Arc::clone(&def.action), def.mode));
				}
				continue;
			};
			let action_id_str = actions.interner.resolve(action_entry.id()).to_string();

			if let Some((kept_action, kept_priority)) =
				seen.get(&(def.mode, Arc::clone(&def.keys))).cloned()
			{
				registry.conflicts.push(KeymapConflict {
					mode: def.mode,
					keys: Arc::clone(&def.keys),
					kept_action,
					dropped_action: action_id_str,
					kept_priority,
					dropped_priority: def.priority,
				});
				continue;
			}

			let Ok(keys) = parse_seq(&def.keys) else {
				if parse_failures.len() < 5 {
					parse_failures.push((Arc::clone(&def.keys), Arc::clone(&def.action)));
				}
				continue;
			};

			seen.insert(
				(def.mode, Arc::clone(&def.keys)),
				(action_id_str.clone(), def.priority),
			);

			let entry = BindingEntry {
				action_id,
				action_name: actions.interner.resolve(action_entry.name()).to_string(),
				description: actions
					.interner
					.resolve(action_entry.description())
					.to_string(),
				short_desc: actions
					.interner
					.resolve(action_entry.short_desc)
					.to_string(),
				keys: keys.clone(),
			};

			registry.add(def.mode, keys, entry);
		}

		if !registry.conflicts.is_empty() {
			let samples: Vec<_> = registry.conflicts.iter().take(5).collect();
			tracing::debug!(
				count = registry.conflicts.len(),
				?samples,
				"Keymap conflicts detected"
			);
		}

		if !unknown_actions.is_empty() {
			warn!(
				count = unknown_actions.len(),
				?unknown_actions,
				"Unknown actions referenced by keybindings"
			);
		}

		if !parse_failures.is_empty() {
			warn!(
				count = parse_failures.len(),
				?parse_failures,
				"Failed to parse keybinding sequences"
			);
		}

		registry
	}

	/// Returns available continuations at a given key prefix.
	pub fn continuations_at(
		&self,
		mode: BindingMode,
		prefix: &[Node],
	) -> Vec<(&Node, Option<&BindingEntry>)> {
		let Some(matcher) = self.matchers.get(&mode) else {
			return Vec::new();
		};
		matcher.continuations_at(prefix)
	}

	/// Returns continuations with classification (leaf vs branch).
	pub fn continuations_with_kind(
		&self,
		mode: BindingMode,
		prefix: &[Node],
	) -> Vec<ContinuationEntry<'_, BindingEntry>> {
		let Some(matcher) = self.matchers.get(&mode) else {
			return Vec::new();
		};
		matcher.continuations_with_kind(prefix)
	}

	pub fn conflicts(&self) -> &[KeymapConflict] {
		&self.conflicts
	}
}

/// Returns the global keymap registry.
pub fn get_keymap_registry() -> &'static KeymapRegistry {
	&crate::db::get_db().keymap
}
