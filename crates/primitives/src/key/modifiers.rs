//! Key modifier types (Ctrl, Alt, Shift).

/// Key modifiers (Ctrl, Alt, Shift).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct Modifiers {
	/// Whether Ctrl is held.
	pub ctrl: bool,
	/// Whether Alt is held.
	pub alt: bool,
	/// Whether Shift is held.
	pub shift: bool,
}

impl Modifiers {
	/// No modifiers pressed.
	pub const NONE: Self = Self {
		ctrl: false,
		alt: false,
		shift: false,
	};

	/// Only Ctrl pressed.
	pub const CTRL: Self = Self {
		ctrl: true,
		alt: false,
		shift: false,
	};

	/// Only Alt pressed.
	pub const ALT: Self = Self {
		ctrl: false,
		alt: true,
		shift: false,
	};

	/// Only Shift pressed.
	pub const SHIFT: Self = Self {
		ctrl: false,
		alt: false,
		shift: true,
	};

	/// Returns a copy with Ctrl added.
	pub fn ctrl(self) -> Self {
		Self { ctrl: true, ..self }
	}

	/// Returns a copy with Alt added.
	pub fn alt(self) -> Self {
		Self { alt: true, ..self }
	}

	/// Returns a copy with Shift added.
	pub fn shift(self) -> Self {
		Self { shift: true, ..self }
	}

	/// Returns true if no modifiers are set.
	pub fn is_empty(self) -> bool {
		!self.ctrl && !self.alt && !self.shift
	}
}
