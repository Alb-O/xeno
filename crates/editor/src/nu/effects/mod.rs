//! Shared Nu effect processing for macro and hook surfaces.
//!
//! Centralizes capability gating, notification mapping, and per-surface
//! `stop_propagation` semantics so hooks and macros stay behaviorally aligned.

use std::collections::HashSet;

use tracing::warn;
use xeno_invocation::nu::NuTextEditOp;
use xeno_primitives::transaction::Change;
use xeno_primitives::{EditOrigin, Transaction, UndoPolicy};

use crate::buffer::ViewId;
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
	if !batch.warnings.is_empty() {
		let total = batch.warnings.len();
		let preview: Vec<&str> = batch.warnings.iter().take(5).map(String::as_str).collect();
		match mode {
			NuEffectApplyMode::Hook => tracing::debug!(total, ?preview, "Nu hook batch warnings"),
			NuEffectApplyMode::Macro => warn!(total, ?preview, "Nu macro batch warnings"),
		}
	}

	let mut outcome = NuEffectApplyOutcome::default();
	let mut did_record_nu_edit = false;

	// Macro surface must be all-or-nothing for policy validation.
	if matches!(mode, NuEffectApplyMode::Macro) {
		for effect in &batch.effects {
			let required = required_capability_for_effect(effect);
			if !allowed.contains(&required) {
				return Err(NuEffectApplyError::CapabilityDenied { capability: required });
			}
			if matches!(effect, NuEffect::StopPropagation) {
				return Err(NuEffectApplyError::StopPropagationUnsupportedForMacro);
			}
		}
	}

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
			NuEffect::SetClipboard { text } => {
				apply_clipboard_set(editor, text);
				outcome.dirty |= Dirty::FULL;
			}
			NuEffect::StateSet { key, value } => {
				editor.state.core.workspace.nu_state.set(key, value);
			}
			NuEffect::StateUnset { key } => {
				editor.state.core.workspace.nu_state.unset(&key);
			}
			NuEffect::ScheduleSet { key, delay_ms, name, args } => {
				editor.state.nu.schedule_macro(key, delay_ms, name, args, &editor.state.msg_tx);
			}
			NuEffect::ScheduleCancel { key } => {
				editor.state.nu.cancel_schedule(&key);
			}
			NuEffect::EditText { op, text } => {
				if editor.buffer().is_readonly() {
					warn!(mode = mode.label(), "Nu edit effect skipped: buffer is readonly");
					continue;
				}
				let undo_policy = if matches!(mode, NuEffectApplyMode::Macro) && did_record_nu_edit {
					UndoPolicy::MergeWithCurrentGroup
				} else {
					UndoPolicy::Record
				};
				apply_text_edit(editor, op, text, undo_policy);
				did_record_nu_edit = true;
				outcome.dirty |= Dirty::FULL;
			}
		}
	}

	Ok(outcome)
}

/// Write text to the yank register (clipboard).
fn apply_clipboard_set(editor: &mut Editor, text: String) {
	let total_chars = text.chars().count();
	editor.state.core.workspace.registers.yank = crate::types::Yank {
		parts: vec![text],
		total_chars,
	};
}

/// Apply a text edit effect to the focused buffer.
fn apply_text_edit(editor: &mut Editor, op: NuTextEditOp, text: String, undo_policy: UndoPolicy) {
	let buffer_id: ViewId = editor.focused_view();
	let buffer = editor.state.core.buffers.get_buffer_mut(buffer_id).expect("focused buffer must exist");

	let (tx, new_selection) = buffer.with_doc(|doc| {
		let rope = doc.content();
		match op {
			NuTextEditOp::ReplaceSelection => {
				let sel = buffer.selection.primary();
				let start = sel.min().min(rope.len_chars());
				let end = sel.max().min(rope.len_chars());
				let tx = Transaction::change(
					rope.slice(..),
					[Change {
						start,
						end,
						replacement: Some(text),
					}],
				);
				let new_sel = tx.map_selection(&buffer.selection);
				(tx, Some(new_sel))
			}
			NuTextEditOp::ReplaceLine => {
				let cursor = buffer.cursor.min(rope.len_chars());
				let line_idx = rope.char_to_line(cursor);
				let line_start = rope.line_to_char(line_idx);
				let line_slice = rope.line(line_idx);
				let line_len = line_slice.len_chars();
				// Exclude trailing line endings (\n or \r\n) from replacement range.
				let mut trim = 0;
				if line_len > trim && line_slice.char(line_len - 1 - trim) == '\n' {
					trim += 1;
				}
				if line_len > trim && line_slice.char(line_len - 1 - trim) == '\r' {
					trim += 1;
				}
				let content_end = line_start + line_len - trim;
				let tx = Transaction::change(
					rope.slice(..),
					[Change {
						start: line_start,
						end: content_end,
						replacement: Some(text),
					}],
				);
				let new_sel = tx.map_selection(&buffer.selection);
				(tx, Some(new_sel))
			}
		}
	});

	editor.apply_edit(buffer_id, &tx, new_selection, undo_policy, EditOrigin::Internal("nu_edit"));
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
	fn macro_mode_capability_denial_is_atomic() {
		let mut editor = Editor::new_scratch();
		let allowed = HashSet::from([NuCapability::WriteState]);
		let batch = batch(vec![
			NuEffect::StateSet {
				key: "foo".to_string(),
				value: "bar".to_string(),
			},
			NuEffect::Dispatch(Invocation::action("move_right")),
		]);

		let err = apply_effect_batch(&mut editor, batch, NuEffectApplyMode::Macro, &allowed).expect_err("macro denial should error");
		assert!(matches!(
			err,
			NuEffectApplyError::CapabilityDenied {
				capability: NuCapability::DispatchAction
			}
		));
		assert!(editor.state.core.workspace.nu_state.iter().next().is_none());
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

	#[test]
	fn edit_text_replace_selection_modifies_buffer() {
		let mut editor = Editor::new_scratch();
		editor.buffer_mut().reset_content("hello world");
		// Select "world" (chars 6..11)
		editor.buffer_mut().set_selection(xeno_primitives::Selection::single(6, 11));

		let allowed = HashSet::from([NuCapability::EditText]);
		let b = batch(vec![NuEffect::EditText {
			op: NuTextEditOp::ReplaceSelection,
			text: "WORLD".to_string(),
		}]);
		let outcome = apply_effect_batch(&mut editor, b, NuEffectApplyMode::Macro, &allowed).expect("edit should succeed");
		assert_eq!(outcome.dirty, Dirty::FULL);

		let text = editor.buffer().with_doc(|doc| doc.content().to_string());
		assert_eq!(text, "hello WORLD");
	}

	#[test]
	fn edit_text_replace_line_modifies_buffer() {
		let mut editor = Editor::new_scratch();
		editor.buffer_mut().reset_content("line one\nline two\n");
		// Cursor on line 1 (second line)
		editor.buffer_mut().cursor = 9; // start of "line two"

		let allowed = HashSet::from([NuCapability::EditText]);
		let b = batch(vec![NuEffect::EditText {
			op: NuTextEditOp::ReplaceLine,
			text: "LINE TWO".to_string(),
		}]);
		let outcome = apply_effect_batch(&mut editor, b, NuEffectApplyMode::Macro, &allowed).expect("edit should succeed");
		assert_eq!(outcome.dirty, Dirty::FULL);

		let text = editor.buffer().with_doc(|doc| doc.content().to_string());
		assert_eq!(text, "line one\nLINE TWO\n");
	}

	#[test]
	fn edit_text_replace_selection_deletes_on_empty_text() {
		let mut editor = Editor::new_scratch();
		editor.buffer_mut().reset_content("hello world");
		editor.buffer_mut().set_selection(xeno_primitives::Selection::single(5, 11));

		let allowed = HashSet::from([NuCapability::EditText]);
		let b = batch(vec![NuEffect::EditText {
			op: NuTextEditOp::ReplaceSelection,
			text: String::new(),
		}]);
		apply_effect_batch(&mut editor, b, NuEffectApplyMode::Macro, &allowed).expect("edit should succeed");

		let text = editor.buffer().with_doc(|doc| doc.content().to_string());
		assert_eq!(text, "hello");
	}

	#[test]
	fn clipboard_set_populates_yank_register() {
		let mut editor = Editor::new_scratch();
		let allowed = HashSet::from([NuCapability::SetClipboard]);
		let b = batch(vec![NuEffect::SetClipboard { text: "COPIED".to_string() }]);
		let outcome = apply_effect_batch(&mut editor, b, NuEffectApplyMode::Macro, &allowed).expect("clipboard should succeed");
		assert_eq!(outcome.dirty, Dirty::FULL);

		let yank = &editor.state.core.workspace.registers.yank;
		assert_eq!(yank.parts, vec!["COPIED"]);
		assert_eq!(yank.total_chars, 6);
	}

	#[test]
	fn clipboard_set_denied_without_capability() {
		let mut editor = Editor::new_scratch();
		let b = batch(vec![NuEffect::SetClipboard { text: "X".to_string() }]);
		let err = apply_effect_batch(&mut editor, b, NuEffectApplyMode::Macro, &HashSet::new()).expect_err("should deny");
		assert!(matches!(
			err,
			NuEffectApplyError::CapabilityDenied {
				capability: NuCapability::SetClipboard
			}
		));
	}

	#[test]
	fn clipboard_hook_denied_drops_silently() {
		let mut editor = Editor::new_scratch();
		let b = batch(vec![NuEffect::SetClipboard { text: "X".to_string() }]);
		let outcome = apply_effect_batch(&mut editor, b, NuEffectApplyMode::Hook, &HashSet::new()).expect("hook denial should be non-fatal");
		assert!(editor.state.core.workspace.registers.yank.is_empty());
		assert_eq!(outcome.dirty, Dirty::NONE);
	}

	#[test]
	fn state_set_populates_store() {
		let mut editor = Editor::new_scratch();
		let allowed = HashSet::from([NuCapability::WriteState]);
		let b = batch(vec![NuEffect::StateSet {
			key: "foo".to_string(),
			value: "bar".to_string(),
		}]);
		apply_effect_batch(&mut editor, b, NuEffectApplyMode::Macro, &allowed).expect("state set should succeed");

		let entries: Vec<_> = editor.state.core.workspace.nu_state.iter().collect();
		assert_eq!(entries, vec![("foo", "bar")]);
	}

	#[test]
	fn state_unset_removes_key() {
		let mut editor = Editor::new_scratch();
		editor.state.core.workspace.nu_state.set("foo".to_string(), "bar".to_string());
		let allowed = HashSet::from([NuCapability::WriteState]);
		let b = batch(vec![NuEffect::StateUnset { key: "foo".to_string() }]);
		apply_effect_batch(&mut editor, b, NuEffectApplyMode::Macro, &allowed).expect("state unset should succeed");

		let entries: Vec<_> = editor.state.core.workspace.nu_state.iter().collect();
		assert!(entries.is_empty());
	}

	#[tokio::test]
	async fn schedule_set_creates_entry() {
		let mut editor = Editor::new_scratch();
		let allowed = HashSet::from([NuCapability::ScheduleMacro]);
		let b = batch(vec![NuEffect::ScheduleSet {
			key: "autosave".to_string(),
			delay_ms: 60_000,
			name: "save-all".to_string(),
			args: vec![],
		}]);
		apply_effect_batch(&mut editor, b, NuEffectApplyMode::Macro, &allowed).expect("schedule should succeed");
		// Timer is running (we won't wait for it, just verify no panic)
	}

	#[tokio::test]
	async fn schedule_cancel_removes_entry() {
		let mut editor = Editor::new_scratch();
		let allowed = HashSet::from([NuCapability::ScheduleMacro]);
		// Set then cancel
		let b1 = batch(vec![NuEffect::ScheduleSet {
			key: "autosave".to_string(),
			delay_ms: 60_000,
			name: "save-all".to_string(),
			args: vec![],
		}]);
		apply_effect_batch(&mut editor, b1, NuEffectApplyMode::Macro, &allowed).expect("schedule should succeed");

		let b2 = batch(vec![NuEffect::ScheduleCancel { key: "autosave".to_string() }]);
		apply_effect_batch(&mut editor, b2, NuEffectApplyMode::Macro, &allowed).expect("cancel should succeed");
	}

	#[tokio::test]
	async fn schedule_reschedule_replaces_entry() {
		let mut editor = Editor::new_scratch();
		let allowed = HashSet::from([NuCapability::ScheduleMacro]);
		let b1 = batch(vec![NuEffect::ScheduleSet {
			key: "fmt".to_string(),
			delay_ms: 300,
			name: "format-buffer".to_string(),
			args: vec![],
		}]);
		apply_effect_batch(&mut editor, b1, NuEffectApplyMode::Macro, &allowed).expect("schedule should succeed");

		// Reschedule same key replaces without panic
		let b2 = batch(vec![NuEffect::ScheduleSet {
			key: "fmt".to_string(),
			delay_ms: 500,
			name: "format-buffer-v2".to_string(),
			args: vec![],
		}]);
		apply_effect_batch(&mut editor, b2, NuEffectApplyMode::Macro, &allowed).expect("reschedule should succeed");
	}

	#[tokio::test]
	async fn schedule_fire_enqueues_invocation() {
		let mut editor = Editor::new_scratch();
		let allowed = HashSet::from([NuCapability::ScheduleMacro]);
		// Schedule with 0ms delay
		let b = batch(vec![NuEffect::ScheduleSet {
			key: "test".to_string(),
			delay_ms: 0,
			name: "my-macro".to_string(),
			args: vec!["arg1".to_string()],
		}]);
		apply_effect_batch(&mut editor, b, NuEffectApplyMode::Macro, &allowed).expect("schedule should succeed");

		// Give the 0ms timer time to fire
		tokio::time::sleep(std::time::Duration::from_millis(50)).await;
		editor.drain_messages();

		let queued = editor.pop_runtime_work().expect("scheduled invocation should be enqueued");
		let crate::runtime::work_queue::RuntimeWorkKind::Invocation(queued) = queued.kind else {
			panic!("expected queued invocation work");
		};
		let inv = queued.invocation;
		assert!(matches!(inv, Invocation::Nu { ref name, ref args } if name == "my-macro" && args == &["arg1"]));
	}

	#[tokio::test]
	async fn stale_schedule_fire_ignored() {
		use crate::nu::coordinator::NuScheduleFiredMsg;

		let mut state = crate::nu::coordinator::NuCoordinatorState::new();
		// Simulate a fire with no matching entry
		let fired = state.apply_schedule_fired(NuScheduleFiredMsg {
			key: "nonexistent".to_string(),
			token: 999,
			name: "m".to_string(),
			args: vec![],
		});
		assert!(fired.is_none());
	}

	#[tokio::test]
	async fn stale_schedule_token_does_not_cancel_current_schedule() {
		use crate::nu::coordinator::NuScheduleFiredMsg;

		let mut editor = Editor::new_scratch();
		let allowed = HashSet::from([NuCapability::ScheduleMacro]);

		// Token 1: replaced by the next schedule set on the same key.
		let old = batch(vec![NuEffect::ScheduleSet {
			key: "debounce".to_string(),
			delay_ms: 60_000,
			name: "old".to_string(),
			args: vec![],
		}]);
		apply_effect_batch(&mut editor, old, NuEffectApplyMode::Macro, &allowed).expect("first schedule should succeed");

		// Token 2: current active entry.
		let current = batch(vec![NuEffect::ScheduleSet {
			key: "debounce".to_string(),
			delay_ms: 10,
			name: "current".to_string(),
			args: vec!["arg".to_string()],
		}]);
		apply_effect_batch(&mut editor, current, NuEffectApplyMode::Macro, &allowed).expect("reschedule should succeed");

		// Stale fire for token 1 should not remove the active token 2 schedule.
		let fired = editor.state.nu.apply_schedule_fired(NuScheduleFiredMsg {
			key: "debounce".to_string(),
			token: 1,
			name: "stale".to_string(),
			args: vec![],
		});
		assert!(fired.is_none());

		tokio::time::sleep(std::time::Duration::from_millis(50)).await;
		editor.drain_messages();

		let queued = editor.pop_runtime_work().expect("current schedule should still fire");
		let crate::runtime::work_queue::RuntimeWorkKind::Invocation(queued) = queued.kind else {
			panic!("expected queued invocation work");
		};
		let inv = queued.invocation;
		assert!(matches!(inv, Invocation::Nu { ref name, ref args } if name == "current" && args == &["arg"]));
	}

	#[test]
	fn edit_text_denied_without_capability() {
		let mut editor = Editor::new_scratch();
		editor.buffer_mut().reset_content("hello");

		let b = batch(vec![NuEffect::EditText {
			op: NuTextEditOp::ReplaceSelection,
			text: "X".to_string(),
		}]);
		let err = apply_effect_batch(&mut editor, b, NuEffectApplyMode::Macro, &HashSet::new()).expect_err("should deny");
		assert!(matches!(
			err,
			NuEffectApplyError::CapabilityDenied {
				capability: NuCapability::EditText
			}
		));
	}
}
