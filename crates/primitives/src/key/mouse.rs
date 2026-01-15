//! Mouse event types (clicks, drags, scrolls).

use super::Modifiers;

/// Mouse button types.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MouseButton {
	/// Left mouse button.
	Left,
	/// Right mouse button.
	Right,
	/// Middle mouse button (scroll wheel click).
	Middle,
}

/// Mouse event types.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MouseEvent {
	/// Mouse button pressed.
	Press {
		/// Which button was pressed.
		button: MouseButton,
		/// Row position (0-indexed).
		row: u16,
		/// Column position (0-indexed).
		col: u16,
		/// Active modifiers during press.
		modifiers: Modifiers,
	},
	/// Mouse button released.
	Release {
		/// Row position (0-indexed).
		row: u16,
		/// Column position (0-indexed).
		col: u16,
	},
	/// Mouse dragged while button held.
	Drag {
		/// Which button is held.
		button: MouseButton,
		/// Row position (0-indexed).
		row: u16,
		/// Column position (0-indexed).
		col: u16,
		/// Active modifiers during drag.
		modifiers: Modifiers,
	},
	/// Mouse scroll wheel event.
	Scroll {
		/// Scroll direction.
		direction: ScrollDirection,
		/// Row position (0-indexed).
		row: u16,
		/// Column position (0-indexed).
		col: u16,
		/// Active modifiers during scroll.
		modifiers: Modifiers,
	},
	/// Mouse moved (no buttons pressed).
	Move {
		/// Row position (0-indexed).
		row: u16,
		/// Column position (0-indexed).
		col: u16,
	},
}

/// Scroll direction.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ScrollDirection {
	/// Scroll up (content moves down).
	Up,
	/// Scroll down (content moves up).
	Down,
	/// Scroll left.
	Left,
	/// Scroll right.
	Right,
}

impl MouseEvent {
	/// Returns the row position of this mouse event.
	pub fn row(&self) -> u16 {
		match self {
			MouseEvent::Press { row, .. }
			| MouseEvent::Release { row, .. }
			| MouseEvent::Drag { row, .. }
			| MouseEvent::Scroll { row, .. }
			| MouseEvent::Move { row, .. } => *row,
		}
	}

	/// Returns the column position of this mouse event.
	pub fn col(&self) -> u16 {
		match self {
			MouseEvent::Press { col, .. }
			| MouseEvent::Release { col, .. }
			| MouseEvent::Drag { col, .. }
			| MouseEvent::Scroll { col, .. }
			| MouseEvent::Move { col, .. } => *col,
		}
	}

	/// Returns the modifiers active during this mouse event.
	pub fn modifiers(&self) -> Modifiers {
		match self {
			MouseEvent::Press { modifiers, .. }
			| MouseEvent::Drag { modifiers, .. }
			| MouseEvent::Scroll { modifiers, .. } => *modifiers,
			MouseEvent::Release { .. } | MouseEvent::Move { .. } => Modifiers::NONE,
		}
	}
}

impl From<termina::event::MouseEvent> for MouseEvent {
	fn from(event: termina::event::MouseEvent) -> Self {
		use termina::event::{Modifiers as TmModifiers, MouseButton as TmButton, MouseEventKind};

		let modifiers = Modifiers {
			ctrl: event.modifiers.contains(TmModifiers::CONTROL),
			alt: event.modifiers.contains(TmModifiers::ALT),
			shift: event.modifiers.contains(TmModifiers::SHIFT),
		};

		let convert_button = |btn: TmButton| match btn {
			TmButton::Left => MouseButton::Left,
			TmButton::Right => MouseButton::Right,
			TmButton::Middle => MouseButton::Middle,
		};

		match event.kind {
			MouseEventKind::Down(btn) => MouseEvent::Press {
				button: convert_button(btn),
				row: event.row,
				col: event.column,
				modifiers,
			},
			MouseEventKind::Up(_) => MouseEvent::Release {
				row: event.row,
				col: event.column,
			},
			MouseEventKind::Drag(btn) => MouseEvent::Drag {
				button: convert_button(btn),
				row: event.row,
				col: event.column,
				modifiers,
			},
			MouseEventKind::ScrollUp => MouseEvent::Scroll {
				direction: ScrollDirection::Up,
				row: event.row,
				col: event.column,
				modifiers,
			},
			MouseEventKind::ScrollDown => MouseEvent::Scroll {
				direction: ScrollDirection::Down,
				row: event.row,
				col: event.column,
				modifiers,
			},
			MouseEventKind::ScrollLeft => MouseEvent::Scroll {
				direction: ScrollDirection::Left,
				row: event.row,
				col: event.column,
				modifiers,
			},
			MouseEventKind::ScrollRight => MouseEvent::Scroll {
				direction: ScrollDirection::Right,
				row: event.row,
				col: event.column,
				modifiers,
			},
			MouseEventKind::Moved => MouseEvent::Move {
				row: event.row,
				col: event.column,
			},
		}
	}
}
