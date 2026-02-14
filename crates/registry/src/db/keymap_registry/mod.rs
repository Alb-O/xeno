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

/// What a keybinding resolves to.
#[derive(Debug, Clone)]
pub enum BindingTarget {
	/// A resolved action from the registry.
	Action { id: ActionId },
	/// An invocation spec string (e.g., `command:write file.txt`, `editor:reload_config`).
	InvocationSpec { spec: Arc<str>, kind: xeno_invocation_spec::SpecKind },
}

/// Binding entry storing target info and the key sequence.
#[derive(Debug, Clone)]
pub struct BindingEntry {
	/// What this binding dispatches to.
	pub target: BindingTarget,
	/// Display name (action name or invocation spec).
	pub name: Arc<str>,
	/// Human-readable description for UI display.
	pub description: Arc<str>,
	/// Short description without key-sequence prefix (for which-key HUD).
	pub short_desc: Arc<str>,
	/// Key sequence that triggers this binding (for display).
	pub keys: Vec<Node>,
}

impl BindingEntry {
	/// Returns the action ID if this binding targets an action.
	pub fn action_id(&self) -> Option<ActionId> {
		match &self.target {
			BindingTarget::Action { id } => Some(*id),
			BindingTarget::InvocationSpec { .. } => None,
		}
	}
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

/// Classification of a keymap build problem.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeymapProblemKind {
	/// Key sequence string couldn't be parsed.
	InvalidKeySequence,
	/// Action target name couldn't be resolved in the registry.
	UnknownActionTarget,
	/// Invocation spec failed validation.
	InvalidInvocationSpec,
}

/// A problem encountered during keymap index construction.
#[derive(Debug, Clone)]
pub struct KeymapBuildProblem {
	pub mode: Option<BindingMode>,
	pub keys: Arc<str>,
	pub target: Arc<str>,
	pub kind: KeymapProblemKind,
	pub message: Arc<str>,
}

/// Registry of keybindings organized by mode.
pub struct KeymapIndex {
	/// Per-mode trie matchers for key sequences.
	matchers: HashMap<BindingMode, Matcher<BindingEntry>>,
	conflicts: Vec<KeymapConflict>,
	problems: Vec<KeymapBuildProblem>,
}

#[derive(Debug, Clone)]
pub struct KeymapConflict {
	pub mode: BindingMode,
	pub keys: Arc<str>,
	pub kept_target: String,
	pub dropped_target: String,
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
			problems: Vec::new(),
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

		for (id, def) in bindings {
			let action_entry = &actions.table[id.as_u32() as usize];
			let action_id_str = actions.interner.resolve(action_entry.id()).to_string();

			if let Some((kept_target, kept_priority)) = seen.get(&(def.mode, Arc::clone(&def.keys))).cloned() {
				registry.conflicts.push(KeymapConflict {
					mode: def.mode,
					keys: Arc::clone(&def.keys),
					kept_target,
					dropped_target: action_id_str,
					kept_priority,
					dropped_priority: def.priority,
				});
				continue;
			}

			let Ok(keys) = parse_seq(&def.keys) else {
				registry.push_problem(
					Some(def.mode),
					&def.keys,
					&def.action,
					KeymapProblemKind::InvalidKeySequence,
					"invalid key sequence",
				);
				continue;
			};

			seen.insert((def.mode, Arc::clone(&def.keys)), (action_id_str.clone(), def.priority));

			let entry = BindingEntry {
				target: BindingTarget::Action { id },
				name: Arc::from(actions.interner.resolve(action_entry.name())),
				description: Arc::from(actions.interner.resolve(action_entry.description())),
				short_desc: Arc::from(actions.interner.resolve(action_entry.short_desc)),
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

			for (mode, key_seq, target_spec) in override_entries {
				let Ok(keys) = parse_seq(&key_seq) else {
					registry.push_problem(
						Some(mode),
						&key_seq,
						&target_spec,
						KeymapProblemKind::InvalidKeySequence,
						"invalid key sequence",
					);
					continue;
				};

				// Non-action targets go into the trie as InvocationSpec
				if target_spec.starts_with("command:") || target_spec.starts_with("editor:") || target_spec.starts_with("nu:") {
					let parsed = match xeno_invocation_spec::parse_spec(&target_spec) {
						Ok(p) => p,
						Err(e) => {
							registry.push_problem(
								Some(mode),
								&key_seq,
								&target_spec,
								KeymapProblemKind::InvalidInvocationSpec,
								&format!("invalid invocation spec: {e}"),
							);
							continue;
						}
					};

					if let Some((prev_target, prev_priority)) = seen.get(&(mode, Arc::clone(&key_seq))).cloned() {
						registry.conflicts.push(KeymapConflict {
							mode,
							keys: Arc::clone(&key_seq),
							kept_target: target_spec.to_string(),
							dropped_target: prev_target,
							kept_priority: i16::MIN,
							dropped_priority: prev_priority,
						});
					}
					seen.insert((mode, Arc::clone(&key_seq)), (target_spec.to_string(), i16::MIN));

					let spec_arc: Arc<str> = Arc::from(target_spec.as_ref());
					let entry = BindingEntry {
						target: BindingTarget::InvocationSpec {
							spec: Arc::clone(&spec_arc),
							kind: parsed.kind,
						},
						name: Arc::clone(&spec_arc),
						description: Arc::clone(&spec_arc),
						short_desc: Arc::from(parsed.name.as_str()),
						keys: keys.clone(),
					};
					registry.add(mode, keys, entry);
					continue;
				}

				let resolved_name = target_spec.strip_prefix("action:").unwrap_or(&target_spec);
				let Some(sym) = actions.interner.get(resolved_name) else {
					registry.push_problem(
						Some(mode),
						&key_seq,
						&target_spec,
						KeymapProblemKind::UnknownActionTarget,
						"unknown action target",
					);
					continue;
				};

				let Some(action_id) = actions
					.by_id
					.get(&sym)
					.or_else(|| actions.by_name.get(&sym))
					.or_else(|| actions.by_key.get(&sym))
					.copied()
				else {
					registry.push_problem(
						Some(mode),
						&key_seq,
						&target_spec,
						KeymapProblemKind::UnknownActionTarget,
						"unknown action target",
					);
					continue;
				};

				let action_entry = &actions.table[action_id.as_u32() as usize];
				let action_name_str = actions.interner.resolve(action_entry.name()).to_string();
				let action_id_str = actions.interner.resolve(action_entry.id()).to_string();

				if let Some((prev_target, prev_priority)) = seen.get(&(mode, Arc::clone(&key_seq))).cloned() {
					registry.conflicts.push(KeymapConflict {
						mode,
						keys: Arc::clone(&key_seq),
						kept_target: action_id_str.clone(),
						dropped_target: prev_target,
						kept_priority: i16::MIN,
						dropped_priority: prev_priority,
					});
				}

				seen.insert((mode, Arc::clone(&key_seq)), (action_id_str, i16::MIN));

				let entry = BindingEntry {
					target: BindingTarget::Action { id: action_id },
					name: Arc::from(action_name_str.as_str()),
					description: Arc::from(actions.interner.resolve(action_entry.description())),
					short_desc: Arc::from(actions.interner.resolve(action_entry.short_desc)),
					keys: keys.clone(),
				};

				registry.add(mode, keys, entry);
			}
		}

		if !registry.conflicts.is_empty() {
			let samples: Vec<_> = registry.conflicts.iter().take(5).collect();
			tracing::debug!(count = registry.conflicts.len(), ?samples, "Keymap conflicts detected");
		}

		if !registry.problems.is_empty() {
			let samples: Vec<_> = registry.problems.iter().take(5).collect();
			warn!(count = registry.problems.len(), ?samples, "Keymap build problems");
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

	pub fn problems(&self) -> &[KeymapBuildProblem] {
		&self.problems
	}

	fn push_problem(&mut self, mode: Option<BindingMode>, keys: &Arc<str>, target: &Arc<str>, kind: KeymapProblemKind, message: &str) {
		if self.problems.len() < 50 {
			self.problems.push(KeymapBuildProblem {
				mode,
				keys: Arc::clone(keys),
				target: Arc::clone(target),
				kind,
				message: Arc::from(message),
			});
		}
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
mod tests;
