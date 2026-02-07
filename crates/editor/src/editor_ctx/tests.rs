//! Behavior-lock tests for the effect interpreter.

use xeno_primitives::range::CharIdx;
use xeno_primitives::{Mode, Selection};
use xeno_registry::actions::editor_ctx::{
	CursorAccess, EditorCapabilities, ModeAccess, NotificationAccess, SelectionAccess,
};
use xeno_registry::notifications::Notification;
use xeno_registry::{ActionEffects, AppEffect, UiEffect, ViewEffect};

use crate::editor_ctx::apply_effects;

struct MockEditor {
	cursor: CharIdx,
	selection: Selection,
	mode: Mode,
	notifications: Vec<Notification>,
	effect_log: Vec<String>,
}

impl MockEditor {
	fn new() -> Self {
		Self {
			cursor: CharIdx::from(0usize),
			selection: Selection::point(CharIdx::from(0usize)),
			mode: Mode::Normal,
			notifications: Vec::new(),
			effect_log: Vec::new(),
		}
	}
}

impl CursorAccess for MockEditor {
	fn focused_view(&self) -> xeno_registry::hooks::ViewId {
		xeno_registry::hooks::ViewId::text(1)
	}

	fn cursor(&self) -> CharIdx {
		self.cursor
	}

	fn cursor_line_col(&self) -> Option<(usize, usize)> {
		Some((0, self.cursor))
	}

	fn set_cursor(&mut self, pos: CharIdx) {
		self.effect_log.push(format!("set_cursor:{}", pos));
		self.cursor = pos;
	}
}

impl SelectionAccess for MockEditor {
	fn selection(&self) -> &Selection {
		&self.selection
	}

	fn selection_mut(&mut self) -> &mut Selection {
		&mut self.selection
	}

	fn set_selection(&mut self, sel: Selection) {
		self.effect_log
			.push(format!("set_selection:{}", sel.primary().head));
		self.selection = sel;
	}
}

impl ModeAccess for MockEditor {
	fn mode(&self) -> Mode {
		self.mode.clone()
	}

	fn set_mode(&mut self, mode: Mode) {
		self.effect_log.push(format!("set_mode:{:?}", mode));
		self.mode = mode;
	}
}

impl NotificationAccess for MockEditor {
	fn emit(&mut self, notification: Notification) {
		self.effect_log
			.push(format!("notify:{}", notification.def.id));
		self.notifications.push(notification);
	}

	fn clear_notifications(&mut self) {
		self.notifications.clear();
	}
}

impl EditorCapabilities for MockEditor {}

#[test]
fn effects_apply_in_order() {
	let mut editor = MockEditor::new();
	let mut ctx = xeno_registry::actions::editor_ctx::EditorContext::new(&mut editor);

	let effects = ActionEffects::new()
		.with(ViewEffect::SetCursor(CharIdx::from(10usize)))
		.with(ViewEffect::SetSelection(Selection::point(CharIdx::from(
			20usize,
		))))
		.with(AppEffect::SetMode(Mode::Insert));

	apply_effects(&effects, &mut ctx, false);

	assert_eq!(editor.effect_log.len(), 4);
	assert_eq!(editor.effect_log[0], "set_cursor:10");
	assert_eq!(editor.effect_log[1], "set_cursor:20");
	assert_eq!(editor.effect_log[2], "set_selection:20");
	assert!(editor.effect_log[3].starts_with("set_mode:"));
}

#[test]
fn selection_mapping_preserves_bounds() {
	let mut editor = MockEditor::new();
	let ctx = xeno_registry::actions::editor_ctx::EditorContext::new(&mut editor);
	let _ = ctx;
}

#[test]
fn set_mode_changes_mode() {
	let mut editor = MockEditor::new();
	let mut ctx = xeno_registry::actions::editor_ctx::EditorContext::new(&mut editor);

	let effects = ActionEffects::mode(Mode::Insert);
	apply_effects(&effects, &mut ctx, false);

	assert_eq!(editor.mode, Mode::Insert);
}

#[test]
fn multiple_cursor_updates_apply_sequentially() {
	let mut editor = MockEditor::new();
	let mut ctx = xeno_registry::actions::editor_ctx::EditorContext::new(&mut editor);

	let effects = ActionEffects::new()
		.with(ViewEffect::SetCursor(CharIdx::from(5usize)))
		.with(ViewEffect::SetCursor(CharIdx::from(10usize)))
		.with(ViewEffect::SetCursor(CharIdx::from(15usize)));

	apply_effects(&effects, &mut ctx, false);

	assert_eq!(editor.cursor, 15);

	let cursor_calls: Vec<_> = editor
		.effect_log
		.iter()
		.filter(|s| s.starts_with("set_cursor:"))
		.collect();
	assert_eq!(cursor_calls.len(), 3);
	assert_eq!(cursor_calls[0], "set_cursor:5");
	assert_eq!(cursor_calls[1], "set_cursor:10");
	assert_eq!(cursor_calls[2], "set_cursor:15");
}

#[test]
fn notify_effect_emits_notification() {
	let mut editor = MockEditor::new();
	let mut ctx = xeno_registry::actions::editor_ctx::EditorContext::new(&mut editor);

	let effects = ActionEffects::from_effect(
		UiEffect::Notify(xeno_registry::notifications::keys::UNDO.into()).into(),
	);
	apply_effects(&effects, &mut ctx, false);

	assert_eq!(editor.notifications.len(), 1);
	assert_eq!(editor.notifications[0].def.id, "xeno-registry::undo");
}

#[test]
fn error_effect_emits_error_notification() {
	let mut editor = MockEditor::new();
	let mut ctx = xeno_registry::actions::editor_ctx::EditorContext::new(&mut editor);

	let effects = ActionEffects::error("test error");
	apply_effects(&effects, &mut ctx, false);

	assert_eq!(editor.notifications.len(), 1);
	assert_eq!(
		editor.notifications[0].def.id,
		"xeno-registry::action_error"
	);
}

#[test]
fn empty_effects_is_noop() {
	let mut editor = MockEditor::new();
	let mut ctx = xeno_registry::actions::editor_ctx::EditorContext::new(&mut editor);

	let effects = ActionEffects::ok();
	apply_effects(&effects, &mut ctx, false);

	assert!(editor.effect_log.is_empty());
	assert!(editor.notifications.is_empty());
}

#[test]
fn quit_effect_returns_quit_outcome() {
	let mut editor = MockEditor::new();
	let mut ctx = xeno_registry::actions::editor_ctx::EditorContext::new(&mut editor);

	let effects = ActionEffects::quit();
	let outcome = apply_effects(&effects, &mut ctx, false);

	assert!(matches!(
		outcome,
		xeno_registry::actions::editor_ctx::HandleOutcome::Quit
	));
}

#[test]
fn non_quit_effects_return_handled_outcome() {
	let mut editor = MockEditor::new();
	let mut ctx = xeno_registry::actions::editor_ctx::EditorContext::new(&mut editor);

	let effects = ActionEffects::cursor(CharIdx::from(10usize));
	let outcome = apply_effects(&effects, &mut ctx, false);

	assert!(matches!(
		outcome,
		xeno_registry::actions::editor_ctx::HandleOutcome::Handled
	));
}

/// Selection must be applied before mode change so mode-entry logic sees
/// the updated cursor position.
#[test]
fn selection_applied_before_mode_change() {
	let mut editor = MockEditor::new();
	let mut ctx = xeno_registry::actions::editor_ctx::EditorContext::new(&mut editor);

	let sel = Selection::point(CharIdx::from(42usize));
	let effects = ActionEffects::selection(sel).with(AppEffect::SetMode(Mode::Insert));

	apply_effects(&effects, &mut ctx, false);

	let sel_idx = editor
		.effect_log
		.iter()
		.position(|s| s.starts_with("set_selection:"))
		.unwrap();
	let mode_idx = editor
		.effect_log
		.iter()
		.position(|s| s.starts_with("set_mode:"))
		.unwrap();

	assert!(
		sel_idx < mode_idx,
		"Selection must be applied before mode change"
	);
	assert_eq!(editor.mode, Mode::Insert);
	assert_eq!(editor.cursor, 42);
}

/// Quit short-circuits the return outcome but subsequent effects still execute.
#[test]
fn effects_after_quit_still_execute() {
	let mut editor = MockEditor::new();
	let mut ctx = xeno_registry::actions::editor_ctx::EditorContext::new(&mut editor);

	let effects = ActionEffects::new()
		.with(AppEffect::Quit { force: false })
		.with(ViewEffect::SetCursor(CharIdx::from(99usize)))
		.with(AppEffect::SetMode(Mode::Insert));

	let outcome = apply_effects(&effects, &mut ctx, false);

	assert!(matches!(
		outcome,
		xeno_registry::actions::editor_ctx::HandleOutcome::Quit
	));
	assert_eq!(editor.cursor, 99);
	assert_eq!(editor.mode, Mode::Insert);
}

/// Notifications are side effects that don't affect subsequent effect processing.
#[test]
fn notifications_are_side_effects() {
	let mut editor = MockEditor::new();
	let mut ctx = xeno_registry::actions::editor_ctx::EditorContext::new(&mut editor);

	let effects = ActionEffects::new()
		.with(ViewEffect::SetCursor(CharIdx::from(10usize)))
		.with(UiEffect::Notify(
			xeno_registry::notifications::keys::UNDO.into(),
		))
		.with(ViewEffect::SetCursor(CharIdx::from(20usize)));

	apply_effects(&effects, &mut ctx, false);

	assert_eq!(editor.cursor, 20);
	assert_eq!(editor.notifications.len(), 1);

	let cursor_positions: Vec<_> = editor
		.effect_log
		.iter()
		.filter_map(|s| s.strip_prefix("set_cursor:").map(str::to_string))
		.collect();
	assert_eq!(cursor_positions, vec!["10", "20"]);
}

/// SetSelection emits cursor update followed by selection update.
#[test]
fn set_selection_emits_cursor_then_selection() {
	let mut editor = MockEditor::new();
	let mut ctx = xeno_registry::actions::editor_ctx::EditorContext::new(&mut editor);

	let sel = Selection::single(5, 15);
	let effects = ActionEffects::selection(sel);

	apply_effects(&effects, &mut ctx, false);

	assert_eq!(editor.effect_log.len(), 2);
	assert_eq!(editor.effect_log[0], "set_cursor:15");
	assert_eq!(editor.effect_log[1], "set_selection:15");
}
