//! Unified keymap registry using trie-based matching.
//!
//! This module provides [`KeymapRegistry`], which uses the `xeno-keymap` crate's
//! trie-based matcher for efficient key sequence lookup supporting:
//!
//! - Key sequences (`g g`, `d d`)
//! - Partial match detection for which-key style UIs
//! - Key groups (`@digit`, `@alpha`)

use std::collections::HashMap;
use std::sync::OnceLock;

use tracing::warn;
use xeno_keymap_core::parser::{Node, parse_seq};
pub use xeno_keymap_core::{ContinuationEntry, ContinuationKind};
use xeno_keymap_core::{MatchResult, Matcher};
use xeno_registry_core::ActionId;

use crate::actions::{BindingMode, KEYBINDINGS};

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
///
/// Each mode has its own [`Matcher`] for efficient trie-based lookup
/// supporting key sequences and partial matches.
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
		use crate::index::{find_action, resolve_action_id};

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

	/// Returns all bindings for a mode (for which-key display).
	pub fn bindings_for_mode(&self, _mode: BindingMode) -> Vec<&BindingEntry> {
		// TODO: Implement iteration over matcher entries
		Vec::new()
	}

	/// Returns available continuations at a given key prefix.
	///
	/// Used for which-key style HUD showing next available keys.
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
	///
	/// Each continuation is classified as:
	/// - `Leaf`: Terminal binding with no further children
	/// - `Branch`: Sub-prefix with more bindings underneath
	///
	/// This enables which-key UIs to show "…" for branches that can be drilled into.
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
///
/// The registry is lazily initialized from the `KEYBINDINGS` list.
pub fn get_keymap_registry() -> &'static KeymapRegistry {
	KEYMAP_REGISTRY.get_or_init(KeymapRegistry::from_slice)
}

#[cfg(test)]
mod tests {
	use xeno_keymap_core::parser::parse_seq;

	use super::*;

	fn test_entry(name: &'static str) -> BindingEntry {
		BindingEntry {
			action_id: ActionId(1),
			action_name: name,
			description: "",
			short_desc: "",
			keys: vec![],
		}
	}

	#[test]
	fn single_key_lookup() {
		let mut registry = KeymapRegistry::new();
		registry.add(
			BindingMode::Normal,
			parse_seq("h").unwrap(),
			test_entry("move_left"),
		);

		match registry.lookup(BindingMode::Normal, &parse_seq("h").unwrap()) {
			LookupResult::Match(entry) => assert_eq!(entry.action_name, "move_left"),
			other => panic!("Expected Match, got {other:?}"),
		}
	}

	#[test]
	fn sequence_lookup() {
		let mut registry = KeymapRegistry::new();
		registry.add(
			BindingMode::Normal,
			parse_seq("g g").unwrap(),
			test_entry("document_start"),
		);

		// "g" alone is pending
		match registry.lookup(BindingMode::Normal, &parse_seq("g").unwrap()) {
			LookupResult::Pending { sticky: None } => {}
			other => panic!("Expected Pending without sticky, got {other:?}"),
		}

		// "g g" completes
		match registry.lookup(BindingMode::Normal, &parse_seq("g g").unwrap()) {
			LookupResult::Match(entry) => assert_eq!(entry.action_name, "document_start"),
			other => panic!("Expected Match, got {other:?}"),
		}
	}

	#[test]
	fn sticky_action() {
		let mut registry = KeymapRegistry::new();
		// "g" has a sticky action (can execute immediately or wait for more keys)
		registry.add(
			BindingMode::Normal,
			parse_seq("g").unwrap(),
			test_entry("sticky_prefix"),
		);
		// "g g" is also bound
		registry.add(
			BindingMode::Normal,
			parse_seq("g g").unwrap(),
			test_entry("document_start"),
		);

		// "g" is pending with sticky action available
		match registry.lookup(BindingMode::Normal, &parse_seq("g").unwrap()) {
			LookupResult::Pending {
				sticky: Some(entry),
			} => assert_eq!(entry.action_name, "sticky_prefix"),
			other => panic!("Expected Pending with sticky, got {other:?}"),
		}
	}

	#[test]
	fn no_match() {
		let registry = KeymapRegistry::new();
		match registry.lookup(BindingMode::Normal, &parse_seq("x").unwrap()) {
			LookupResult::None => {}
			other => panic!("Expected None, got {other:?}"),
		}
	}
}
