//! Unified keymap registry using trie-matching.

use std::collections::HashMap;
use std::sync::Arc;

use arc_swap::ArcSwap;
use tracing::warn;
use xeno_keymap_core::parser::{Node, parse_seq};
pub use xeno_keymap_core::{ContinuationEntry, ContinuationKind};
use xeno_keymap_core::{MatchResult, Matcher};

use crate::actions::{ActionEntry, BindingMode};
use crate::config::UnresolvedKeys;
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
		Self::build_with_overrides(actions, None)
	}

	/// Build registry index from action snapshot with optional key overrides.
	pub fn build_with_overrides(actions: &Snapshot<ActionEntry, ActionId>, overrides: Option<&UnresolvedKeys>) -> Self {
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

			if let Some((kept_action, kept_priority)) = seen.get(&(def.mode, Arc::clone(&def.keys))).cloned() {
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
				push_parse_failure(&mut parse_failures, &def.keys, &def.action);
				continue;
			};

			seen.insert((def.mode, Arc::clone(&def.keys)), (action_id_str.clone(), def.priority));

			let entry = BindingEntry {
				action_id: id,
				action_name: actions.interner.resolve(action_entry.name()).to_string(),
				description: actions.interner.resolve(action_entry.description()).to_string(),
				short_desc: actions.interner.resolve(action_entry.short_desc).to_string(),
				keys: keys.clone(),
			};

			registry.add(def.mode, keys, entry);
		}

		if let Some(overrides) = overrides {
			let mut override_entries: Vec<(BindingMode, Arc<str>, Arc<str>)> = Vec::new();
			for (mode_name, key_map) in &overrides.modes {
				let Some(mode) = parse_binding_mode(mode_name) else {
					continue;
				};
				for (key_seq, action_name) in key_map {
					override_entries.push((mode, Arc::from(key_seq.as_str()), Arc::from(action_name.as_str())));
				}
			}

			override_entries.sort_by(|a, b| a.0.cmp(&b.0).then_with(|| a.1.cmp(&b.1)).then_with(|| a.2.cmp(&b.2)));

			for (mode, key_seq, action_name) in override_entries {
				let resolved_name = action_name.strip_prefix("action:").unwrap_or(&action_name);
				let Some(sym) = actions.interner.get(resolved_name) else {
					push_parse_failure(&mut parse_failures, &key_seq, &action_name);
					continue;
				};

				let Some(action_id) = actions
					.by_id
					.get(&sym)
					.or_else(|| actions.by_name.get(&sym))
					.or_else(|| actions.by_key.get(&sym))
					.copied()
				else {
					push_parse_failure(&mut parse_failures, &key_seq, &action_name);
					continue;
				};

				let Ok(keys) = parse_seq(&key_seq) else {
					push_parse_failure(&mut parse_failures, &key_seq, &action_name);
					continue;
				};

				let action_entry = &actions.table[action_id.as_u32() as usize];
				let action_name_str = actions.interner.resolve(action_entry.name()).to_string();
				let action_id_str = actions.interner.resolve(action_entry.id()).to_string();

				if let Some((kept_action, kept_priority)) = seen.get(&(mode, Arc::clone(&key_seq))).cloned() {
					registry.conflicts.push(KeymapConflict {
						mode,
						keys: Arc::clone(&key_seq),
						kept_action: action_id_str.clone(),
						dropped_action: kept_action,
						kept_priority: i16::MIN,
						dropped_priority: kept_priority,
					});
				}

				seen.insert((mode, Arc::clone(&key_seq)), (action_id_str, i16::MIN));

				let entry = BindingEntry {
					action_id,
					action_name: action_name_str,
					description: actions.interner.resolve(action_entry.description()).to_string(),
					short_desc: actions.interner.resolve(action_entry.short_desc).to_string(),
					keys: keys.clone(),
				};

				registry.add(mode, keys, entry);
			}
		}

		if !registry.conflicts.is_empty() {
			let samples: Vec<_> = registry.conflicts.iter().take(5).collect();
			tracing::debug!(count = registry.conflicts.len(), ?samples, "Keymap conflicts detected");
		}

		if !parse_failures.is_empty() {
			warn!(count = parse_failures.len(), ?parse_failures, "Failed to parse keybinding sequences");
		}

		registry
	}

	/// Returns available continuations at a given key prefix.
	pub fn continuations_at(&self, mode: BindingMode, prefix: &[Node]) -> Vec<(&Node, Option<&BindingEntry>)> {
		let Some(matcher) = self.matchers.get(&mode) else {
			return Vec::new();
		};
		matcher.continuations_at(prefix)
	}

	/// Returns continuations with classification (leaf vs branch).
	pub fn continuations_with_kind(&self, mode: BindingMode, prefix: &[Node]) -> Vec<ContinuationEntry<'_, BindingEntry>> {
		let Some(matcher) = self.matchers.get(&mode) else {
			return Vec::new();
		};
		matcher.continuations_with_kind(prefix)
	}

	pub fn conflicts(&self) -> &[KeymapConflict] {
		&self.conflicts
	}
}

fn parse_binding_mode(mode: &str) -> Option<BindingMode> {
	match mode.trim().to_ascii_lowercase().as_str() {
		"normal" | "n" => Some(BindingMode::Normal),
		"insert" | "i" => Some(BindingMode::Insert),
		"match" | "m" => Some(BindingMode::Match),
		"space" | "spc" => Some(BindingMode::Space),
		_ => None,
	}
}

fn push_parse_failure(samples: &mut Vec<(Arc<str>, Arc<str>)>, keys: &Arc<str>, action: &Arc<str>) {
	if samples.len() < 5 {
		samples.push((Arc::clone(keys), Arc::clone(action)));
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

#[cfg(test)]
mod tests {
	use std::collections::HashMap;

	use super::*;

	fn mode_name(mode: BindingMode) -> &'static str {
		match mode {
			BindingMode::Normal => "normal",
			BindingMode::Insert => "insert",
			BindingMode::Match => "match",
			BindingMode::Space => "space",
		}
	}

	fn sample_binding(actions: &Snapshot<ActionEntry, ActionId>) -> Option<(BindingMode, String, ActionId, ActionId, String)> {
		for (idx, action_entry) in actions.table.iter().enumerate() {
			let source_id = ActionId::from_u32(idx as u32);
			for binding in action_entry.bindings.iter() {
				if parse_seq(&binding.keys).is_err() {
					continue;
				}

				let Some((target_idx, target_entry)) = actions.table.iter().enumerate().find(|(target_idx, _)| *target_idx != idx) else {
					continue;
				};

				let target_id = ActionId::from_u32(target_idx as u32);
				let target_name = actions.interner.resolve(target_entry.name()).to_string();
				return Some((binding.mode, binding.keys.to_string(), source_id, target_id, target_name));
			}
		}
		None
	}

	fn lookup_action_id(index: &KeymapIndex, mode: BindingMode, key_seq: &str) -> ActionId {
		let keys = parse_seq(key_seq).expect("binding key sequence should parse");
		match index.lookup(mode, &keys) {
			LookupResult::Match(entry) => entry.action_id,
			_ => panic!("expected a complete keybinding match"),
		}
	}

	#[test]
	fn override_wins_over_base_binding() {
		let actions = crate::db::ACTIONS.snapshot();
		let (mode, key_seq, _base_id, target_id, target_name) = sample_binding(&actions).expect("registry should contain at least one binding");

		let mut mode_overrides = HashMap::new();
		mode_overrides.insert(key_seq.clone(), target_name);
		let mut modes = HashMap::new();
		modes.insert(mode_name(mode).to_string(), mode_overrides);
		let overrides = UnresolvedKeys { modes };

		let index = KeymapIndex::build_with_overrides(&actions, Some(&overrides));
		let resolved = lookup_action_id(&index, mode, &key_seq);
		assert_eq!(resolved, target_id);
	}

	#[test]
	fn invalid_override_action_keeps_base_binding() {
		let actions = crate::db::ACTIONS.snapshot();
		let (mode, key_seq, base_id, _target_id, _target_name) = sample_binding(&actions).expect("registry should contain at least one binding");

		let mut mode_overrides = HashMap::new();
		mode_overrides.insert(key_seq.clone(), "action:does-not-exist".to_string());
		let mut modes = HashMap::new();
		modes.insert(mode_name(mode).to_string(), mode_overrides);
		let overrides = UnresolvedKeys { modes };

		let index = KeymapIndex::build_with_overrides(&actions, Some(&overrides));
		let resolved = lookup_action_id(&index, mode, &key_seq);
		assert_eq!(resolved, base_id);
	}
}
