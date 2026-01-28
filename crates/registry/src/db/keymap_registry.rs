//! Unified keymap registry using trie-based matching.

use std::collections::HashMap;
use std::sync::OnceLock;

use tracing::warn;
use xeno_keymap_core::parser::{Node, parse_seq};
pub use xeno_keymap_core::{ContinuationEntry, ContinuationKind};
use xeno_keymap_core::{MatchResult, Matcher};

use crate::actions::{BindingMode, KEYBINDINGS};
use crate::core::ActionId;

/// Binding entry storing action info and the key sequence.
#[derive(Debug, Clone)]
pub struct BindingEntry {
	/// Resolved action ID for dispatch.
	pub action_id: ActionId,
	/// Action name (for display/debugging).
	pub action_name: &'static str,
	/// Human-readable description for UI display.
	pub description: &'static str,
	/// Short description without key-sequence prefix (for which-key HUD).
	pub short_desc: &'static str,
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

	/// Initialize from the KEYBINDINGS list.
	pub fn from_slice() -> Self {
		use crate::db::index::{find_action, resolve_action_id};

		let mut registry = Self::new();

		for def in KEYBINDINGS.iter() {
			let Some(action_id) = resolve_action_id(def.action) else {
				warn!(
					action = def.action,
					mode = ?def.mode,
					"Unknown action in keybinding"
				);
				continue;
			};

			let Ok(keys) = parse_seq(def.keys) else {
				warn!(
					keys = def.keys,
					action = def.action,
					"Failed to parse key sequence"
				);
				continue;
			};

			let (description, short_desc) = find_action(def.action)
				.map(|a| (a.description(), a.short_desc))
				.unwrap_or(("", ""));

			let entry = BindingEntry {
				action_id,
				action_name: def.action,
				description,
				short_desc,
				keys: keys.clone(),
			};

			registry.add(def.mode, keys, entry);
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
}

/// Global keymap registry singleton.
static KEYMAP_REGISTRY: OnceLock<KeymapRegistry> = OnceLock::new();

/// Returns the global keymap registry.
pub fn get_keymap_registry() -> &'static KeymapRegistry {
	KEYMAP_REGISTRY.get_or_init(KeymapRegistry::from_slice)
}
