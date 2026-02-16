//! Shared Nu effect processing for macro and hook surfaces.
//!
//! Centralizes capability gating, notification mapping, and per-surface
//! `stop_propagation` semantics so hooks and macros stay behaviorally aligned.

use std::collections::HashSet;

use tracing::warn;

use crate::impls::Editor;
use crate::msg::Dirty;
use crate::nu::{NuCapability, NuEffect, NuEffectBatch, NuNotifyLevel, required_capability_for_effect};
use crate::types::Invocation;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum NuEffectApplyMode {
	Hook,
	Macro,
}

impl NuEffectApplyMode {
	const fn label(self) -> &'static str {
		match self {
			Self::Hook => "hook",
			Self::Macro => "macro",
		}
	}
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum NuEffectApplyError {
	CapabilityDenied { capability: NuCapability },
	StopPropagationUnsupportedForMacro,
}

#[derive(Debug, Default)]
pub(crate) struct NuEffectApplyOutcome {
	pub dirty: Dirty,
	pub dispatches: Vec<Invocation>,
	pub stop_requested: bool,
}

/// Apply a decoded Nu effect batch under explicit surface policy.
///
/// Hook mode drops denied-capability effects and continues.
/// Macro mode rejects denied-capability effects and stop-propagation.
pub(crate) fn apply_effect_batch(
	editor: &mut Editor,
	batch: NuEffectBatch,
	mode: NuEffectApplyMode,
	allowed: &HashSet<NuCapability>,
) -> Result<NuEffectApplyOutcome, NuEffectApplyError> {
	let mut outcome = NuEffectApplyOutcome::default();

	for effect in batch.effects {
		let required = required_capability_for_effect(&effect);
		if !allowed.contains(&required) {
			match mode {
				NuEffectApplyMode::Hook => {
					warn!(mode = mode.label(), capability = %required.as_str(), "Nu effect denied by capability policy");
					continue;
				}
				NuEffectApplyMode::Macro => {
					return Err(NuEffectApplyError::CapabilityDenied { capability: required });
				}
			}
		}

		match effect {
			NuEffect::Dispatch(invocation) => {
				outcome.dispatches.push(invocation);
				outcome.dirty |= Dirty::FULL;
			}
			NuEffect::Notify { level, message } => {
				emit_nu_notification(editor, level, message);
				outcome.dirty |= Dirty::FULL;
			}
			NuEffect::StopPropagation => match mode {
				NuEffectApplyMode::Hook => {
					outcome.stop_requested = true;
					outcome.dirty |= Dirty::FULL;
					break;
				}
				NuEffectApplyMode::Macro => return Err(NuEffectApplyError::StopPropagationUnsupportedForMacro),
			},
		}
	}

	Ok(outcome)
}

pub(crate) fn emit_nu_notification(editor: &mut Editor, level: NuNotifyLevel, message: String) {
	use xeno_registry::notifications::keys;

	match level {
		NuNotifyLevel::Debug => editor.notify(keys::debug(message)),
		NuNotifyLevel::Info => editor.notify(keys::info(message)),
		NuNotifyLevel::Warn => editor.notify(keys::warn(message)),
		NuNotifyLevel::Error => editor.notify(keys::error(message)),
		NuNotifyLevel::Success => editor.notify(keys::success(message)),
	}
}

#[cfg(test)]
mod tests {
	use std::collections::HashSet;

	use super::*;

	fn batch(effects: Vec<NuEffect>) -> NuEffectBatch {
		NuEffectBatch {
			effects,
			..NuEffectBatch::default()
		}
	}

	#[test]
	fn hook_mode_capability_denial_is_non_fatal() {
		let mut editor = Editor::new_scratch();
		let batch = batch(vec![NuEffect::Dispatch(Invocation::action("move_right"))]);

		let outcome = apply_effect_batch(&mut editor, batch, NuEffectApplyMode::Hook, &HashSet::new()).expect("hook denial should be non-fatal");

		assert_eq!(outcome.dirty, Dirty::NONE);
		assert!(outcome.dispatches.is_empty());
		assert!(!outcome.stop_requested);
	}

	#[test]
	fn macro_mode_capability_denial_is_error() {
		let mut editor = Editor::new_scratch();
		let batch = batch(vec![NuEffect::Dispatch(Invocation::action("move_right"))]);

		let err = apply_effect_batch(&mut editor, batch, NuEffectApplyMode::Macro, &HashSet::new()).expect_err("macro denial should error");
		assert!(matches!(
			err,
			NuEffectApplyError::CapabilityDenied {
				capability: NuCapability::DispatchAction
			}
		));
	}

	#[test]
	fn stop_propagation_is_hook_only() {
		let mut editor = Editor::new_scratch();
		let mut allowed = HashSet::new();
		allowed.insert(NuCapability::StopPropagation);

		let hook_outcome =
			apply_effect_batch(&mut editor, batch(vec![NuEffect::StopPropagation]), NuEffectApplyMode::Hook, &allowed).expect("hook stop should succeed");
		assert!(hook_outcome.stop_requested);
		assert_eq!(hook_outcome.dirty, Dirty::FULL);

		let macro_err =
			apply_effect_batch(&mut editor, batch(vec![NuEffect::StopPropagation]), NuEffectApplyMode::Macro, &allowed).expect_err("macro stop should fail");
		assert!(matches!(macro_err, NuEffectApplyError::StopPropagationUnsupportedForMacro));
	}

	#[test]
	fn notify_levels_map_to_editor_notifications() {
		let mut editor = Editor::new_scratch();
		let allowed = HashSet::from([NuCapability::Notify]);
		let batch = batch(vec![
			NuEffect::Notify {
				level: NuNotifyLevel::Debug,
				message: "d".to_string(),
			},
			NuEffect::Notify {
				level: NuNotifyLevel::Info,
				message: "i".to_string(),
			},
			NuEffect::Notify {
				level: NuNotifyLevel::Warn,
				message: "w".to_string(),
			},
			NuEffect::Notify {
				level: NuNotifyLevel::Error,
				message: "e".to_string(),
			},
			NuEffect::Notify {
				level: NuNotifyLevel::Success,
				message: "s".to_string(),
			},
		]);

		let outcome = apply_effect_batch(&mut editor, batch, NuEffectApplyMode::Macro, &allowed).expect("notify effects should succeed");
		assert_eq!(outcome.dirty, Dirty::FULL);

		let pending = editor.state.notifications.take_pending();
		assert_eq!(pending.len(), 5);
		assert_eq!(pending[0].message, "d");
		assert_eq!(&*pending[0].id, "xeno-registry::debug");
		assert_eq!(pending[1].message, "i");
		assert_eq!(&*pending[1].id, "xeno-registry::info");
		assert_eq!(pending[2].message, "w");
		assert_eq!(&*pending[2].id, "xeno-registry::warn");
		assert_eq!(pending[3].message, "e");
		assert_eq!(&*pending[3].id, "xeno-registry::error");
		assert_eq!(pending[4].message, "s");
		assert_eq!(&*pending[4].id, "xeno-registry::success");
	}
}
