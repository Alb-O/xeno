use std::collections::HashMap;
use std::sync::Arc;

use tracing::warn;
use xeno_keymap_core::Matcher;
use xeno_keymap_core::parser::{Node, parse_seq};

use super::diagnostics::{KeymapBuildProblem, KeymapConflict, KeymapProblemKind, push_problem};
use super::precedence::{CandidatePrecedence, compare_candidates};
use super::snapshot::{CompiledBinding, CompiledBindingTarget, KeymapSnapshot};
use super::spec::{KeymapSpec, SlotKey, SpecBinding, SpecBindingTarget};
use crate::actions::{ActionEntry, BindingMode};
use crate::core::index::Snapshot;
use crate::core::{ActionId, DenseId, RegistryEntry};
use crate::invocation::Invocation;

/// One compiled slot entry produced by keymap compilation.
#[derive(Debug, Clone)]
pub struct CompiledSlot {
	pub slot: SlotKey,
	pub parsed_keys: Vec<Node>,
	pub binding: Option<CompiledBinding>,
}

/// Compiler artifact: resolved entries + diagnostics before runtime index materialization.
#[derive(Debug, Clone)]
pub struct CompiledKeymap {
	slots: Vec<CompiledSlot>,
	prefixes: Vec<(BindingMode, Arc<str>, Arc<str>)>,
	conflicts: Vec<KeymapConflict>,
	problems: Vec<KeymapBuildProblem>,
}

impl CompiledKeymap {
	pub fn slots(&self) -> &[CompiledSlot] {
		&self.slots
	}

	pub fn conflicts(&self) -> &[KeymapConflict] {
		&self.conflicts
	}

	pub fn problems(&self) -> &[KeymapBuildProblem] {
		&self.problems
	}

	pub fn into_snapshot(self) -> KeymapSnapshot {
		let matchers = build_matchers(&self.slots);
		KeymapSnapshot::from_parts(matchers, self.prefixes, self.conflicts, self.problems)
	}
}

/// Keymap compiler turning collected source bindings into resolved runtime entries.
pub struct KeymapCompiler<'a> {
	actions: &'a Snapshot<ActionEntry, ActionId>,
	spec: KeymapSpec,
}

#[derive(Debug, Clone)]
struct Candidate {
	source: super::spec::KeymapBindingSource,
	ordinal: usize,
	priority: i16,
	target_desc: String,
	parsed_keys: Vec<Node>,
	binding: Option<CompiledBinding>,
}

impl<'a> KeymapCompiler<'a> {
	pub fn new(actions: &'a Snapshot<ActionEntry, ActionId>, spec: KeymapSpec) -> Self {
		Self { actions, spec }
	}

	pub fn compile(self) -> CompiledKeymap {
		let mut problems = self.spec.problems;
		let mut conflicts = Vec::new();
		let mut candidates: HashMap<SlotKey, Vec<Candidate>> = HashMap::new();

		for binding in self.spec.bindings {
			let parsed_keys = match parse_seq(binding.sequence()) {
				Ok(keys) => keys,
				Err(_) => {
					push_problem(
						&mut problems,
						Some(binding.mode()),
						binding.sequence(),
						&binding.target_desc,
						KeymapProblemKind::InvalidKeySequence,
						"invalid key sequence",
					);
					continue;
				}
			};

			let resolved = match resolve_binding_target(self.actions, &binding, &parsed_keys, &mut problems) {
				Some(value) => value,
				None => continue,
			};

			let candidate = Candidate {
				source: binding.source,
				ordinal: binding.ordinal,
				priority: binding.priority,
				target_desc: resolved.target_desc,
				parsed_keys,
				binding: resolved.binding,
			};

			candidates.entry(binding.slot).or_default().push(candidate);
		}

		let mut slots = Vec::new();
		for (slot, mut slot_candidates) in candidates {
			slot_candidates.sort_by(|a, b| {
				compare_candidates(
					CandidatePrecedence {
						source: a.source,
						ordinal: a.ordinal,
						priority: a.priority,
						target_desc: &a.target_desc,
					},
					CandidatePrecedence {
						source: b.source,
						ordinal: b.ordinal,
						priority: b.priority,
						target_desc: &b.target_desc,
					},
				)
			});
			let winner = slot_candidates.pop().expect("slot has at least one candidate");
			for loser in slot_candidates {
				conflicts.push(KeymapConflict {
					mode: slot.mode,
					keys: Arc::clone(&slot.sequence),
					kept_target: winner.target_desc.clone(),
					dropped_target: loser.target_desc,
					kept_priority: winner.priority,
					dropped_priority: loser.priority,
				});
			}

			slots.push(CompiledSlot {
				slot,
				parsed_keys: winner.parsed_keys,
				binding: winner.binding,
			});
		}

		let prefixes = self
			.spec
			.prefixes
			.into_iter()
			.map(|prefix| (prefix.mode, prefix.keys, prefix.description))
			.collect();

		if !conflicts.is_empty() {
			let samples: Vec<_> = conflicts.iter().take(5).collect();
			tracing::debug!(count = conflicts.len(), ?samples, "keymap conflicts detected");
		}

		if !problems.is_empty() {
			let samples: Vec<_> = problems.iter().take(5).collect();
			warn!(count = problems.len(), ?samples, "keymap compile problems");
		}

		CompiledKeymap {
			slots,
			prefixes,
			conflicts,
			problems,
		}
	}
}

struct ResolvedTarget {
	binding: Option<CompiledBinding>,
	target_desc: String,
}

fn resolve_binding_target(
	actions: &Snapshot<ActionEntry, ActionId>,
	binding: &SpecBinding,
	parsed_keys: &[Node],
	problems: &mut Vec<KeymapBuildProblem>,
) -> Option<ResolvedTarget> {
	match &binding.target {
		SpecBindingTarget::Unbind => Some(ResolvedTarget {
			binding: None,
			target_desc: binding.target_desc.to_string(),
		}),
		SpecBindingTarget::Action { id, count, extend, register } => {
			let action_entry = &actions.table[id.as_u32() as usize];
			let target_desc = canonical_action_id(actions, *id);
			Some(ResolvedTarget {
				binding: Some(CompiledBinding::new(
					CompiledBindingTarget::Action {
						id: *id,
						count: (*count).max(1),
						extend: *extend,
						register: *register,
					},
					Arc::from(actions.interner.resolve(action_entry.name())),
					Arc::from(actions.interner.resolve(action_entry.description())),
					Arc::from(actions.interner.resolve(action_entry.short_desc)),
					parsed_keys.to_vec(),
				)),
				target_desc,
			})
		}
		SpecBindingTarget::Invocation(inv) => resolve_invocation(actions, binding, inv, parsed_keys, problems),
	}
}

fn resolve_invocation(
	actions: &Snapshot<ActionEntry, ActionId>,
	binding: &SpecBinding,
	inv: &Invocation,
	parsed_keys: &[Node],
	problems: &mut Vec<KeymapBuildProblem>,
) -> Option<ResolvedTarget> {
	match inv {
		Invocation::Action { name, count, extend, register } => {
			let Some(action_id) = resolve_action_by_name(actions, name) else {
				push_problem(
					problems,
					Some(binding.mode()),
					binding.sequence(),
					&binding.target_desc,
					KeymapProblemKind::UnknownActionTarget,
					"unknown action target",
				);
				return None;
			};

			let action_entry = &actions.table[action_id.as_u32() as usize];
			let target_desc = canonical_action_id(actions, action_id);
			Some(ResolvedTarget {
				binding: Some(CompiledBinding::new(
					CompiledBindingTarget::Action {
						id: action_id,
						count: (*count).max(1),
						extend: *extend,
						register: *register,
					},
					Arc::from(actions.interner.resolve(action_entry.name())),
					Arc::from(actions.interner.resolve(action_entry.description())),
					Arc::from(actions.interner.resolve(action_entry.short_desc)),
					parsed_keys.to_vec(),
				)),
				target_desc,
			})
		}
		Invocation::ActionWithChar { name, .. } => {
			if resolve_action_by_name(actions, name).is_none() {
				push_problem(
					problems,
					Some(binding.mode()),
					binding.sequence(),
					&binding.target_desc,
					KeymapProblemKind::UnknownActionTarget,
					"unknown action target",
				);
				return None;
			}

			Some(ResolvedTarget {
				binding: Some(CompiledBinding::new(
					CompiledBindingTarget::Invocation { inv: inv.clone() },
					Arc::from(name.as_str()),
					Arc::from(binding.target_desc.as_ref()),
					Arc::from(name.as_str()),
					parsed_keys.to_vec(),
				)),
				target_desc: binding.target_desc.to_string(),
			})
		}
		Invocation::Command(xeno_invocation::CommandInvocation { name, .. }) | Invocation::Nu { name, .. } => Some(ResolvedTarget {
			binding: Some(CompiledBinding::new(
				CompiledBindingTarget::Invocation { inv: inv.clone() },
				Arc::from(name.as_str()),
				Arc::from(binding.target_desc.as_ref()),
				Arc::from(name.as_str()),
				parsed_keys.to_vec(),
			)),
			target_desc: binding.target_desc.to_string(),
		}),
	}
}

fn resolve_action_by_name(actions: &Snapshot<ActionEntry, ActionId>, name: &str) -> Option<ActionId> {
	let sym = actions.interner.get(name)?;
	actions
		.by_id
		.get(&sym)
		.or_else(|| actions.by_name.get(&sym))
		.or_else(|| actions.by_key.get(&sym))
		.copied()
}

fn build_matchers(slots: &[CompiledSlot]) -> HashMap<BindingMode, Matcher<CompiledBinding>> {
	let mut sorted: Vec<_> = slots.iter().filter(|slot| slot.binding.is_some()).cloned().collect();
	sorted.sort_by(|a, b| a.slot.cmp(&b.slot));

	let mut matchers: HashMap<BindingMode, Matcher<CompiledBinding>> = HashMap::new();
	for slot in sorted {
		if let Some(binding) = slot.binding {
			matchers.entry(slot.slot.mode).or_default().add(slot.parsed_keys, binding);
		}
	}
	matchers
}

fn canonical_action_id(actions: &Snapshot<ActionEntry, ActionId>, action_id: ActionId) -> String {
	let action_entry = &actions.table[action_id.as_u32() as usize];
	actions.interner.resolve(action_entry.id()).to_string()
}
