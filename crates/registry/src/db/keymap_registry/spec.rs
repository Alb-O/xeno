use std::sync::Arc;

use crate::actions::BindingMode;
use crate::core::ActionId;
use crate::invocation::Invocation;

/// Source layer for a key binding candidate.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeymapBindingSource {
	ActionDefault,
	RuntimeAction,
	Preset,
	Override,
}

impl KeymapBindingSource {
	pub(crate) fn rank(self) -> u8 {
		match self {
			Self::ActionDefault => 1,
			Self::RuntimeAction => 2,
			Self::Preset => 3,
			Self::Override => 4,
		}
	}
}

/// Logical key for slotting source candidates.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct SlotKey {
	pub mode: BindingMode,
	pub sequence: Arc<str>,
}

/// Target candidate before compile-time resolution into runtime bindings.
#[derive(Debug, Clone)]
pub enum SpecBindingTarget {
	Action {
		id: ActionId,
		count: usize,
		extend: bool,
		register: Option<char>,
	},
	Invocation(Invocation),
	Unbind,
}

/// A collected source binding candidate.
#[derive(Debug, Clone)]
pub struct SpecBinding {
	pub source: KeymapBindingSource,
	pub ordinal: usize,
	pub slot: SlotKey,
	pub target: SpecBindingTarget,
	pub target_desc: Arc<str>,
	pub priority: i16,
}

impl SpecBinding {
	pub fn mode(&self) -> BindingMode {
		self.slot.mode
	}

	pub fn sequence(&self) -> &Arc<str> {
		&self.slot.sequence
	}
}

/// Prefix metadata passed through to runtime which-key lookups.
#[derive(Debug, Clone)]
pub struct SpecPrefix {
	pub mode: BindingMode,
	pub keys: Arc<str>,
	pub description: Arc<str>,
}

/// Collected keymap compile input.
#[derive(Debug, Default)]
pub struct KeymapSpec {
	pub bindings: Vec<SpecBinding>,
	pub prefixes: Vec<SpecPrefix>,
	pub problems: Vec<super::diagnostics::KeymapBuildProblem>,
}

pub(crate) fn parse_binding_mode(mode: &str) -> Option<BindingMode> {
	match mode.trim().to_ascii_lowercase().as_str() {
		"normal" | "n" => Some(BindingMode::Normal),
		"insert" | "i" => Some(BindingMode::Insert),
		"match" | "m" => Some(BindingMode::Match),
		"space" | "spc" => Some(BindingMode::Space),
		_ => None,
	}
}
