//! Event type conversions.
//!
//! Converts terminal library events to split buffer events.

use evildoer_core::{
	SplitKey, SplitKeyCode, SplitModifiers, SplitMouse, SplitMouseAction, SplitMouseButton,
	SplitPosition,
};
use termina::event::{KeyCode, Modifiers};

/// Converts a termina KeyEvent to a SplitKey for terminal input.
pub fn convert_termina_key(key: &termina::event::KeyEvent) -> Option<SplitKey> {
	let code = match key.code {
		KeyCode::Char(c) => SplitKeyCode::Char(c),
		KeyCode::Enter => SplitKeyCode::Enter,
		KeyCode::Escape => SplitKeyCode::Escape,
		KeyCode::Backspace => SplitKeyCode::Backspace,
		KeyCode::Tab => SplitKeyCode::Tab,
		KeyCode::Up => SplitKeyCode::Up,
		KeyCode::Down => SplitKeyCode::Down,
		KeyCode::Left => SplitKeyCode::Left,
		KeyCode::Right => SplitKeyCode::Right,
		KeyCode::Home => SplitKeyCode::Home,
		KeyCode::End => SplitKeyCode::End,
		KeyCode::PageUp => SplitKeyCode::PageUp,
		KeyCode::PageDown => SplitKeyCode::PageDown,
		KeyCode::Delete => SplitKeyCode::Delete,
		KeyCode::Insert => SplitKeyCode::Insert,
		_ => return None,
	};

	let mut modifiers = SplitModifiers::NONE;
	if key.modifiers.contains(Modifiers::CONTROL) {
		modifiers = modifiers.union(SplitModifiers::CTRL);
	}
	if key.modifiers.contains(Modifiers::ALT) {
		modifiers = modifiers.union(SplitModifiers::ALT);
	}
	if key.modifiers.contains(Modifiers::SHIFT) {
		modifiers = modifiers.union(SplitModifiers::SHIFT);
	}

	Some(SplitKey::new(code, modifiers))
}

pub fn convert_mouse_event(
	mouse: &termina::event::MouseEvent,
	local_x: u16,
	local_y: u16,
) -> Option<SplitMouse> {
	use termina::event::{MouseButton, MouseEventKind};

	let btn = |b: MouseButton| match b {
		MouseButton::Left => SplitMouseButton::Left,
		MouseButton::Right => SplitMouseButton::Right,
		MouseButton::Middle => SplitMouseButton::Middle,
	};

	let action = match mouse.kind {
		MouseEventKind::Down(b) => SplitMouseAction::Press(btn(b)),
		MouseEventKind::Up(b) => SplitMouseAction::Release(btn(b)),
		MouseEventKind::Drag(b) => SplitMouseAction::Drag(btn(b)),
		MouseEventKind::ScrollUp => SplitMouseAction::ScrollUp,
		MouseEventKind::ScrollDown => SplitMouseAction::ScrollDown,
		_ => return None,
	};

	Some(SplitMouse {
		position: SplitPosition::new(local_x, local_y),
		action,
	})
}
