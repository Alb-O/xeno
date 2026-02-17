//! Unified keymap registry using trie-matching.
//!
//! Builds per-mode `Matcher<BindingEntry>` tries from action registry
//! snapshots and optional user key overrides. The build uses a two-phase
//! "final map then build matcher" approach:
//!
//! 1. Collect base slots from the action snapshot.
//! 2. Apply override layers (bind/unbind) on top.
//! 3. Build trie matchers from the resolved final map.
//!
//! This design supports unbinding (`None` in overrides removes a base
//! binding) and produces deterministic, conflict-aware results.

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
use crate::invocation::Invocation;
use crate::keymaps::KeymapPreset;

/// What a keybinding resolves to.
#[derive(Debug, Clone)]
pub enum BindingTarget {
	/// A resolved action from the registry (fast path).
	Action {
		id: ActionId,
		/// Binding-level count (multiplied with prefix count at dispatch time).
		count: usize,
		/// Binding-level extend flag (OR'd with prefix extend at dispatch time).
		extend: bool,
		/// Binding-level register (prefix register takes priority if set).
		register: Option<char>,
	},
	/// A structured invocation dispatched via `Editor::run_invocation`.
	Invocation { inv: Invocation },
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
			BindingTarget::Action { id, .. } => Some(*id),
			BindingTarget::Invocation { .. } => None,
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
	/// Named prefix descriptions for which-key HUD.
	prefixes: Vec<PrefixEntry>,
	conflicts: Vec<KeymapConflict>,
	problems: Vec<KeymapBuildProblem>,
}

/// A stored prefix description for which-key display.
#[derive(Debug, Clone)]
struct PrefixEntry {
	mode: BindingMode,
	keys: Arc<str>,
	description: Arc<str>,
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

/// Intermediate slot used during the two-phase build.
struct Slot {
	entry: BindingEntry,
	parsed_keys: Vec<Node>,
	target_desc: String,
	priority: i16,
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
			prefixes: Vec::new(),
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

	/// Build registry index from action snapshot using the default preset.
	pub fn build(actions: &Snapshot<ActionEntry, ActionId>) -> Self {
		Self::build_with_overrides(actions, None)
	}

	/// Build registry index from action snapshot with optional key overrides.
	///
	/// Uses the default preset as the base layer.
	pub fn build_with_overrides(actions: &Snapshot<ActionEntry, ActionId>, overrides: Option<&UnresolvedKeys>) -> Self {
		let preset = crate::keymaps::preset(crate::keymaps::DEFAULT_PRESET);
		Self::build_with_preset(actions, preset.as_deref(), overrides)
	}

	/// Build registry index from a preset, action snapshot, and optional key overrides.
	///
	/// Three-phase build:
	/// 1. Populate base slots from the preset's bindings (or fall back to action bindings).
	/// 2. Apply user key overrides: `Some(inv)` replaces/adds, `None` unbinds.
	/// 3. Build trie matchers from the resolved final map.
	pub fn build_with_preset(actions: &Snapshot<ActionEntry, ActionId>, preset: Option<&KeymapPreset>, overrides: Option<&UnresolvedKeys>) -> Self {
		let mut problems = Vec::new();
		let mut conflicts = Vec::new();
		let mut slots: HashMap<(BindingMode, Arc<str>), Slot> = HashMap::new();

		// Phase 1: collect base bindings.
		if let Some(preset) = preset {
			apply_preset_layer(actions, preset, &mut slots, &mut problems, &mut conflicts);
		} else {
			apply_base_layer(actions, &mut slots, &mut problems, &mut conflicts);
		}

		// Phase 1b: add runtime action bindings not covered by the preset.
		apply_runtime_action_bindings(actions, &mut slots, &mut problems, &mut conflicts);

		// Phase 2: apply overrides.
		if let Some(overrides) = overrides {
			apply_override_layer(actions, overrides, &mut slots, &mut problems, &mut conflicts);
		}

		// Phase 3: build matchers from final slots.
		let matchers = build_matchers(slots);

		// Collect prefixes from preset.
		let prefixes = preset.map_or_else(Vec::new, |p| {
			p.prefixes
				.iter()
				.filter_map(|p| {
					let mode = parse_binding_mode(&p.mode)?;
					Some(PrefixEntry {
						mode,
						keys: Arc::clone(&p.keys),
						description: Arc::clone(&p.description),
					})
				})
				.collect()
		});

		if !conflicts.is_empty() {
			let samples: Vec<_> = conflicts.iter().take(5).collect();
			tracing::debug!(count = conflicts.len(), ?samples, "Keymap conflicts detected");
		}

		if !problems.is_empty() {
			let samples: Vec<_> = problems.iter().take(5).collect();
			warn!(count = problems.len(), ?samples, "Keymap build problems");
		}

		KeymapIndex {
			matchers,
			prefixes,
			conflicts,
			problems,
		}
	}

	/// Returns the description for a named prefix key sequence.
	pub fn prefix_description(&self, mode: BindingMode, keys: &str) -> Option<&str> {
		self.prefixes.iter().find(|p| p.mode == mode && &*p.keys == keys).map(|p| &*p.description)
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
}

/// Phase 1 (preset path): populate slots from a keymap preset's bindings.
///
/// Each preset binding carries an invocation spec string (e.g., `"action:move_left"`)
/// resolved against the action snapshot at build time.
fn apply_preset_layer(
	actions: &Snapshot<ActionEntry, ActionId>,
	preset: &KeymapPreset,
	slots: &mut HashMap<(BindingMode, Arc<str>), Slot>,
	problems: &mut Vec<KeymapBuildProblem>,
	conflicts: &mut Vec<KeymapConflict>,
) {
	for binding in &preset.bindings {
		let Some(mode) = parse_binding_mode(&binding.mode) else {
			continue;
		};

		let Ok(parsed_spec) = xeno_invocation_spec::parse_spec(&binding.target) else {
			push_problem(
				problems,
				Some(mode),
				&binding.keys,
				&Arc::from(binding.target.as_str()),
				KeymapProblemKind::InvalidKeySequence,
				"invalid target spec in preset",
			);
			continue;
		};

		let inv = match parsed_spec.kind {
			xeno_invocation_spec::SpecKind::Action => Invocation::action(&parsed_spec.name),
			xeno_invocation_spec::SpecKind::Command => Invocation::command(&parsed_spec.name, parsed_spec.args),
			xeno_invocation_spec::SpecKind::Editor => Invocation::editor_command(&parsed_spec.name, parsed_spec.args),
			xeno_invocation_spec::SpecKind::Nu => Invocation::nu(&parsed_spec.name, parsed_spec.args),
		};

		let target_desc: Arc<str> = Arc::from(binding.target.as_str());
		let Ok(parsed_keys) = parse_seq(&binding.keys) else {
			push_problem(
				problems,
				Some(mode),
				&binding.keys,
				&target_desc,
				KeymapProblemKind::InvalidKeySequence,
				"invalid key sequence in preset",
			);
			continue;
		};

		let entry = match resolve_override_entry(actions, &inv, &parsed_keys, mode, &binding.keys, &target_desc, problems) {
			Some(e) => e,
			None => continue,
		};

		// Use canonical action ID as target_desc for action bindings (consistent with base layer).
		let resolved_desc = match &entry.target {
			BindingTarget::Action { id, .. } => {
				let ae = &actions.table[id.as_u32() as usize];
				actions.interner.resolve(ae.id()).to_string()
			}
			_ => binding.target.clone(),
		};

		let slot_key = (mode, Arc::clone(&binding.keys));
		if let Some(existing) = slots.get(&slot_key) {
			conflicts.push(KeymapConflict {
				mode,
				keys: Arc::clone(&binding.keys),
				kept_target: existing.target_desc.clone(),
				dropped_target: resolved_desc,
				kept_priority: existing.priority,
				dropped_priority: 100,
			});
			continue;
		}

		slots.insert(
			slot_key,
			Slot {
				entry,
				parsed_keys,
				target_desc: resolved_desc,
				priority: 100,
			},
		);
	}
}

/// Adds action bindings from the snapshot that aren't already covered by a
/// preset (or base layer). Handles runtime-registered actions with bindings
/// from plugins that aren't part of any preset file.
fn apply_runtime_action_bindings(
	actions: &Snapshot<ActionEntry, ActionId>,
	slots: &mut HashMap<(BindingMode, Arc<str>), Slot>,
	problems: &mut Vec<KeymapBuildProblem>,
	conflicts: &mut Vec<KeymapConflict>,
) {
	for (idx, action_entry) in actions.table.iter().enumerate() {
		let action_id = ActionId::from_u32(idx as u32);
		let source = action_entry.source();

		// Only consider runtime-registered actions (not from compiled specs).
		if !matches!(source, crate::core::RegistrySource::Runtime) {
			continue;
		}

		for binding in action_entry.bindings.iter() {
			let action_id_str = actions.interner.resolve(action_entry.id()).to_string();
			let slot_key = (binding.mode, Arc::clone(&binding.keys));

			let target_desc: Arc<str> = Arc::from(action_id_str.as_str());
			let Ok(parsed_keys) = parse_seq(&binding.keys) else {
				push_problem(
					problems,
					Some(binding.mode),
					&binding.keys,
					&target_desc,
					KeymapProblemKind::InvalidKeySequence,
					"invalid key sequence",
				);
				continue;
			};

			let slot = Slot {
				entry: BindingEntry {
					target: BindingTarget::Action {
						id: action_id,
						count: 1,
						extend: false,
						register: None,
					},
					name: Arc::from(actions.interner.resolve(action_entry.name())),
					description: Arc::from(actions.interner.resolve(action_entry.description())),
					short_desc: Arc::from(actions.interner.resolve(action_entry.short_desc)),
					keys: parsed_keys.clone(),
				},
				parsed_keys,
				target_desc: action_id_str.clone(),
				priority: binding.priority,
			};

			if let Some(existing) = slots.get(&slot_key) {
				// Runtime/plugin bindings should be able to replace preset bindings
				// when they provide equal-or-stronger priority.
				if binding.priority <= existing.priority {
					conflicts.push(KeymapConflict {
						mode: binding.mode,
						keys: Arc::clone(&binding.keys),
						kept_target: action_id_str.clone(),
						dropped_target: existing.target_desc.clone(),
						kept_priority: binding.priority,
						dropped_priority: existing.priority,
					});
					slots.insert(slot_key, slot);
				} else {
					conflicts.push(KeymapConflict {
						mode: binding.mode,
						keys: Arc::clone(&binding.keys),
						kept_target: existing.target_desc.clone(),
						dropped_target: action_id_str,
						kept_priority: existing.priority,
						dropped_priority: binding.priority,
					});
				}
				continue;
			}

			slots.insert(slot_key, slot);
		}
	}
}

/// Phase 1 (legacy path): populate slots from action snapshot bindings.
fn apply_base_layer(
	actions: &Snapshot<ActionEntry, ActionId>,
	slots: &mut HashMap<(BindingMode, Arc<str>), Slot>,
	problems: &mut Vec<KeymapBuildProblem>,
	conflicts: &mut Vec<KeymapConflict>,
) {
	let mut bindings = Vec::new();
	for (idx, action_entry) in actions.table.iter().enumerate() {
		let action_id = ActionId::from_u32(idx as u32);
		for binding in action_entry.bindings.iter() {
			bindings.push((action_id, binding.clone()));
		}
	}

	// Sort for deterministic first-writer-wins.
	bindings.sort_by(|a, b| {
		a.1.mode
			.cmp(&b.1.mode)
			.then_with(|| a.1.keys.cmp(&b.1.keys))
			.then_with(|| a.1.priority.cmp(&b.1.priority))
			.then_with(|| a.1.action.cmp(&b.1.action))
	});

	for (id, def) in bindings {
		let action_entry = &actions.table[id.as_u32() as usize];
		let action_id_str = actions.interner.resolve(action_entry.id()).to_string();
		let slot_key = (def.mode, Arc::clone(&def.keys));

		if let Some(existing) = slots.get(&slot_key) {
			conflicts.push(KeymapConflict {
				mode: def.mode,
				keys: Arc::clone(&def.keys),
				kept_target: existing.target_desc.clone(),
				dropped_target: action_id_str,
				kept_priority: existing.priority,
				dropped_priority: def.priority,
			});
			continue;
		}

		let Ok(parsed_keys) = parse_seq(&def.keys) else {
			push_problem(
				problems,
				Some(def.mode),
				&def.keys,
				&def.action,
				KeymapProblemKind::InvalidKeySequence,
				"invalid key sequence",
			);
			continue;
		};

		let entry = BindingEntry {
			target: BindingTarget::Action {
				id,
				count: 1,
				extend: false,
				register: None,
			},
			name: Arc::from(actions.interner.resolve(action_entry.name())),
			description: Arc::from(actions.interner.resolve(action_entry.description())),
			short_desc: Arc::from(actions.interner.resolve(action_entry.short_desc)),
			keys: parsed_keys.clone(),
		};

		slots.insert(
			slot_key,
			Slot {
				entry,
				parsed_keys,
				target_desc: action_id_str,
				priority: def.priority,
			},
		);
	}
}

/// Phase 2: apply override layer on top of base slots.
///
/// `None` values unbind (remove from final map). `Some` values replace or add.
fn apply_override_layer(
	actions: &Snapshot<ActionEntry, ActionId>,
	overrides: &UnresolvedKeys,
	slots: &mut HashMap<(BindingMode, Arc<str>), Slot>,
	problems: &mut Vec<KeymapBuildProblem>,
	conflicts: &mut Vec<KeymapConflict>,
) {
	let mut entries: Vec<(BindingMode, Arc<str>, Option<Invocation>)> = Vec::new();
	for (mode_name, key_map) in &overrides.modes {
		let Some(mode) = parse_binding_mode(mode_name) else {
			continue;
		};
		for (key_seq, opt_inv) in key_map {
			entries.push((mode, Arc::from(key_seq.as_str()), opt_inv.clone()));
		}
	}

	// Sort for deterministic processing.
	entries.sort_by(|a, b| a.0.cmp(&b.0).then_with(|| a.1.cmp(&b.1)));

	for (mode, key_seq, opt_inv) in entries {
		let slot_key = (mode, Arc::clone(&key_seq));

		// Unbind: remove from final map.
		let Some(inv) = opt_inv else {
			slots.remove(&slot_key);
			continue;
		};

		let target_desc: Arc<str> = Arc::from(inv.describe().as_str());
		let Ok(parsed_keys) = parse_seq(&key_seq) else {
			push_problem(
				problems,
				Some(mode),
				&key_seq,
				&target_desc,
				KeymapProblemKind::InvalidKeySequence,
				"invalid key sequence",
			);
			continue;
		};

		let entry = match resolve_override_entry(actions, &inv, &parsed_keys, mode, &key_seq, &target_desc, problems) {
			Some(e) => e,
			None => continue, // problem already recorded
		};

		// Record conflict if replacing existing slot.
		if let Some(prev) = slots.get(&slot_key) {
			conflicts.push(KeymapConflict {
				mode,
				keys: Arc::clone(&key_seq),
				kept_target: inv.describe(),
				dropped_target: prev.target_desc.clone(),
				kept_priority: i16::MIN,
				dropped_priority: prev.priority,
			});
		}

		slots.insert(
			slot_key,
			Slot {
				entry,
				parsed_keys,
				target_desc: inv.describe(),
				priority: i16::MIN,
			},
		);
	}
}

/// Resolve an override invocation into a `BindingEntry`, returning `None` on
/// validation failure (problem already pushed).
fn resolve_override_entry(
	actions: &Snapshot<ActionEntry, ActionId>,
	inv: &Invocation,
	parsed_keys: &[Node],
	mode: BindingMode,
	key_seq: &Arc<str>,
	target_desc: &Arc<str>,
	problems: &mut Vec<KeymapBuildProblem>,
) -> Option<BindingEntry> {
	match inv {
		Invocation::Action { name, count, extend, register } => {
			let action_id = resolve_action_by_name(actions, name);
			let Some(action_id) = action_id else {
				push_problem(
					problems,
					Some(mode),
					key_seq,
					target_desc,
					KeymapProblemKind::UnknownActionTarget,
					"unknown action target",
				);
				return None;
			};

			let action_entry = &actions.table[action_id.as_u32() as usize];
			Some(BindingEntry {
				target: BindingTarget::Action {
					id: action_id,
					count: *count,
					extend: *extend,
					register: *register,
				},
				name: Arc::from(actions.interner.resolve(action_entry.name())),
				description: Arc::from(actions.interner.resolve(action_entry.description())),
				short_desc: Arc::from(actions.interner.resolve(action_entry.short_desc)),
				keys: parsed_keys.to_vec(),
			})
		}
		Invocation::ActionWithChar { name, .. } => {
			if resolve_action_by_name(actions, name).is_none() {
				push_problem(
					problems,
					Some(mode),
					key_seq,
					target_desc,
					KeymapProblemKind::UnknownActionTarget,
					"unknown action target",
				);
				return None;
			}

			Some(BindingEntry {
				target: BindingTarget::Invocation { inv: inv.clone() },
				name: Arc::from(name.as_str()),
				description: Arc::clone(target_desc),
				short_desc: Arc::from(name.as_str()),
				keys: parsed_keys.to_vec(),
			})
		}
		Invocation::Command(xeno_invocation::CommandInvocation { name, .. }) | Invocation::Nu { name, .. } => Some(BindingEntry {
			target: BindingTarget::Invocation { inv: inv.clone() },
			name: Arc::from(name.as_str()),
			description: Arc::clone(target_desc),
			short_desc: Arc::from(name.as_str()),
			keys: parsed_keys.to_vec(),
		}),
	}
}

/// Resolve an action name to its `ActionId` using the snapshot's interning tables.
fn resolve_action_by_name(actions: &Snapshot<ActionEntry, ActionId>, name: &str) -> Option<ActionId> {
	let sym = actions.interner.get(name)?;
	actions
		.by_id
		.get(&sym)
		.or_else(|| actions.by_name.get(&sym))
		.or_else(|| actions.by_key.get(&sym))
		.copied()
}

/// Phase 3: build per-mode trie matchers from the final slot map.
fn build_matchers(slots: HashMap<(BindingMode, Arc<str>), Slot>) -> HashMap<BindingMode, Matcher<BindingEntry>> {
	// Gather and sort for deterministic trie construction.
	let mut sorted: Vec<_> = slots.into_iter().collect();
	sorted.sort_by(|a, b| a.0.0.cmp(&b.0.0).then_with(|| a.0.1.cmp(&b.0.1)));

	let mut matchers: HashMap<BindingMode, Matcher<BindingEntry>> = HashMap::new();
	for ((mode, _key_str), slot) in sorted {
		matchers.entry(mode).or_default().add(slot.parsed_keys, slot.entry);
	}
	matchers
}

fn push_problem(problems: &mut Vec<KeymapBuildProblem>, mode: Option<BindingMode>, keys: &Arc<str>, target: &Arc<str>, kind: KeymapProblemKind, message: &str) {
	if problems.len() < 50 {
		problems.push(KeymapBuildProblem {
			mode,
			keys: Arc::clone(keys),
			target: Arc::clone(target),
			kind,
			message: Arc::from(message),
		});
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
