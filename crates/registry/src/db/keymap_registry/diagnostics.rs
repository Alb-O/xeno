use std::sync::Arc;

use crate::actions::BindingMode;

/// Classification of a keymap compile problem.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeymapProblemKind {
	/// Key sequence string couldn't be parsed.
	InvalidKeySequence,
	/// Invocation target spec couldn't be parsed.
	InvalidTargetSpec,
	/// Action target name couldn't be resolved in the action registry.
	UnknownActionTarget,
}

/// A non-fatal problem encountered during keymap compilation.
#[derive(Debug, Clone)]
pub struct KeymapBuildProblem {
	pub mode: Option<BindingMode>,
	pub keys: Arc<str>,
	pub target: Arc<str>,
	pub kind: KeymapProblemKind,
	pub message: Arc<str>,
}

/// Conflict metadata for multiple candidates targeting the same `(mode, keys)`.
#[derive(Debug, Clone)]
pub struct KeymapConflict {
	pub mode: BindingMode,
	pub keys: Arc<str>,
	pub kept_target: String,
	pub dropped_target: String,
	pub kept_priority: i16,
	pub dropped_priority: i16,
}

pub(crate) fn push_problem(
	problems: &mut Vec<KeymapBuildProblem>,
	mode: Option<BindingMode>,
	keys: &Arc<str>,
	target: &Arc<str>,
	kind: KeymapProblemKind,
	message: &str,
) {
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
