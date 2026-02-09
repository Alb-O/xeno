//! Unified keymap registry using trie-matching.

use std::collections::HashMap;
use std::sync::Arc;

use arc_swap::ArcSwap;
use tracing::warn;
use xeno_keymap_core::parser::{Node, parse_seq};
pub use xeno_keymap_core::{ContinuationEntry, ContinuationKind};
use xeno_keymap_core::{MatchResult, Matcher};

use crate::actions::{ActionEntry, BindingMode};
use crate::core::index::Snapshot;
use crate::core::{ActionId, DenseId, RegistryEntry};

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
pub struct KeymapIndex {
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

impl Default for KeymapIndex {
	fn default() -> Self {
		Self::new()
	}
}

impl KeymapIndex {
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

	/// Build registry index from action snapshot.
	pub fn build(actions: &Snapshot<ActionEntry, ActionId>) -> Self {
		let mut registry = Self::new();
		let mut bindings = Vec::new();

		// Collect all bindings from all actions in the snapshot
		for (idx, action_entry) in actions.table.iter().enumerate() {
			let action_id = ActionId::from_u32(idx as u32);
			for binding in action_entry.bindings.iter() {
				bindings.push((action_id, binding.clone()));
			}
		}

		// Sort bindings for deterministic matching
		bindings.sort_by(|a, b| {
			a.1.mode
				.cmp(&b.1.mode)
				.then_with(|| a.1.keys.cmp(&b.1.keys))
				.then_with(|| a.1.priority.cmp(&b.1.priority))
				.then_with(|| a.1.action.cmp(&b.1.action))
		});

		let mut seen: HashMap<(BindingMode, Arc<str>), (String, i16)> = HashMap::new();
		let mut parse_failures: Vec<(Arc<str>, Arc<str>)> = Vec::new();

		for (id, def) in bindings {
			let action_entry = &actions.table[id.as_u32() as usize];
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
				action_id: id,
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

/// A reactive keymap registry that recomputes its index when the underlying actions snapshot changes.
pub struct KeymapRegistry {
	cache: ArcSwap<KeymapCache>,
}

struct KeymapCache {
	snap: Arc<Snapshot<ActionEntry, ActionId>>,
	index: Arc<KeymapIndex>,
}

impl KeymapRegistry {
	/// Creates a new keymap registry initialized from the given snapshot.
	pub fn new(snap: Arc<Snapshot<ActionEntry, ActionId>>) -> Self {
		let index = Arc::new(KeymapIndex::build(&snap));
		Self {
			cache: ArcSwap::from_pointee(KeymapCache { snap, index }),
		}
	}

	/// Returns the keymap index for the given actions snapshot, recomputing it if necessary.
	pub fn for_snapshot(&self, snap: Arc<Snapshot<ActionEntry, ActionId>>) -> Arc<KeymapIndex> {
		let current = self.cache.load();
		if Arc::ptr_eq(&current.snap, &snap) {
			return Arc::clone(&current.index);
		}

		// Recompute
		let index = Arc::new(KeymapIndex::build(&snap));
		self.cache.store(Arc::new(KeymapCache {
			snap: Arc::clone(&snap),
			index: Arc::clone(&index),
		}));
		index
	}
}

/// Returns the current keymap index for the current actions snapshot.
pub fn get_keymap_registry() -> Arc<KeymapIndex> {
	let db = crate::db::get_db();
	db.keymap.for_snapshot(crate::db::ACTIONS.snapshot())
}
