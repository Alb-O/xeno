use xeno_primitives::range::CharIdx;
use xeno_primitives::{Mode, Selection};
use xeno_registry::actions::editor_ctx::{CursorAccess, EditorCapabilities, ModeAccess, NotificationAccess, SelectionAccess};
use xeno_registry::actions::{ActionEffects, AppEffect, ViewEffect};
use xeno_registry::notifications::Notification;

use super::{ActionEffectsEnvelope, EffectsCmd, EffectsEvt, execute_effects_cmd};

struct MockEditor {
	cursor: CharIdx,
	selection: Selection,
	mode: Mode,
	notifications: Vec<Notification>,
}

impl MockEditor {
	fn new() -> Self {
		Self {
			cursor: CharIdx::from(0usize),
			selection: Selection::point(CharIdx::from(0usize)),
			mode: Mode::Normal,
			notifications: Vec::new(),
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
		self.selection = sel;
	}
}

impl ModeAccess for MockEditor {
	fn mode(&self) -> Mode {
		self.mode.clone()
	}

	fn set_mode(&mut self, mode: Mode) {
		self.mode = mode;
	}
}

impl NotificationAccess for MockEditor {
	fn emit(&mut self, notification: Notification) {
		self.notifications.push(notification);
	}

	fn clear_notifications(&mut self) {
		self.notifications.clear();
	}
}

impl EditorCapabilities for MockEditor {}

/// Must keep effects interpreter capability-honest and editor-agnostic.
///
/// * Enforced in: `editor_ctx::execute_effects_cmd`, `editor_ctx::apply_effects`
/// * Failure symptom: registry effects require concrete `Editor` downcasts.
#[cfg_attr(test, test)]
pub fn test_honesty_rule() {
	let mut editor = MockEditor::new();
	let mut ctx = xeno_registry::actions::editor_ctx::EditorContext::new(&mut editor);
	let cmd = EffectsCmd::Apply(ActionEffectsEnvelope {
		effects: ActionEffects::cursor(CharIdx::from(4usize)),
		extend: false,
	});
	let evt = execute_effects_cmd(cmd, &mut ctx);
	assert_eq!(evt, EffectsEvt::Applied { should_quit: false });
	assert_eq!(editor.cursor, CharIdx::from(4usize));
}

/// Must route action effects through `EffectsCmd::Apply` and preserve outcome semantics.
///
/// * Enforced in: `editor_ctx::execute_effects_cmd`
/// * Failure symptom: quit effects are lost or interpreted outside the protocol boundary.
#[cfg_attr(test, test)]
pub fn test_single_path_side_effects() {
	let mut editor = MockEditor::new();
	let mut ctx = xeno_registry::actions::editor_ctx::EditorContext::new(&mut editor);
	let cmd = EffectsCmd::Apply(ActionEffectsEnvelope {
		effects: ActionEffects::new()
			.with(ViewEffect::SetCursor(CharIdx::from(9usize)))
			.with(AppEffect::Quit { force: false }),
		extend: false,
	});
	let evt = execute_effects_cmd(cmd, &mut ctx);
	assert_eq!(evt, EffectsEvt::Applied { should_quit: true });
	assert_eq!(editor.cursor, CharIdx::from(9usize));
}
