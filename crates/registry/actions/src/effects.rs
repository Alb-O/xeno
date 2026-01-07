//! Effect-based action results.
//!
//! This module provides a data-oriented alternative to [`ActionResult`] where
//! actions return composable primitive effects rather than compound behavior variants.
//!
//! # Motivation
//!
//! The traditional `ActionResult` enum models *behaviors* (e.g., `InsertWithMotion`),
//! which couples actions to specific compound operations. Effects model primitive
//! *state mutations* that can be composed freely.
//!
//! # Example
//!
//! ```ignore
//! // Instead of a compound InsertWithMotion variant:
//! ActionEffects::new()
//!     .with(Effect::SetSelection(sel))
//!     .with(Effect::SetMode(Mode::Insert))
//! ```
//!
//! [`ActionResult`]: crate::ActionResult

use xeno_base::direction::{Axis, SeqDirection, SpatialDirection};
use xeno_base::range::CharIdx;
use xeno_base::{Mode, Selection};
use xeno_registry_notifications::Notification;

use crate::{EditAction, PendingAction, ScreenPosition};

/// A collection of effects to apply atomically.
///
/// Actions return this to describe state mutations. Effects are applied
/// in order by the result handler in `xeno-core`.
#[derive(Debug, Clone, Default)]
pub struct ActionEffects {
	effects: Vec<Effect>,
}

impl ActionEffects {
	/// Creates an empty effect set (equivalent to `ActionResult::Ok`).
	#[inline]
	pub fn ok() -> Self {
		Self::default()
	}

	/// Creates an effect set with a single effect.
	#[inline]
	pub fn new() -> Self {
		Self::default()
	}

	/// Creates an effect set from a single effect.
	#[inline]
	pub fn from_effect(effect: Effect) -> Self {
		Self {
			effects: vec![effect],
		}
	}

	/// Adds an effect to this set, returning self for chaining.
	#[inline]
	pub fn with(mut self, effect: impl Into<Effect>) -> Self {
		self.effects.push(effect.into());
		self
	}

	/// Adds an effect to this set in place.
	#[inline]
	pub fn push(&mut self, effect: impl Into<Effect>) {
		self.effects.push(effect.into());
	}

	/// Returns the effects as a slice.
	#[inline]
	pub fn as_slice(&self) -> &[Effect] {
		&self.effects
	}

	/// Returns true if there are no effects.
	#[inline]
	pub fn is_empty(&self) -> bool {
		self.effects.is_empty()
	}

	/// Returns the number of effects.
	#[inline]
	pub fn len(&self) -> usize {
		self.effects.len()
	}

	/// Consumes self and returns the inner Vec.
	#[inline]
	pub fn into_vec(self) -> Vec<Effect> {
		self.effects
	}

	/// Sets the cursor position.
	#[inline]
	pub fn cursor(pos: CharIdx) -> Self {
		Self::from_effect(Effect::SetCursor(pos))
	}

	/// Sets the selection (motion result).
	#[inline]
	pub fn motion(sel: Selection) -> Self {
		Self::from_effect(Effect::SetSelection(sel))
	}

	/// Changes the editor mode.
	#[inline]
	pub fn mode(mode: Mode) -> Self {
		Self::from_effect(Effect::SetMode(mode))
	}

	/// Quits the editor.
	#[inline]
	pub fn quit() -> Self {
		Self::from_effect(Effect::Quit { force: false })
	}

	/// Force quits the editor.
	#[inline]
	pub fn force_quit() -> Self {
		Self::from_effect(Effect::Quit { force: true })
	}

	/// Shows an error message.
	#[inline]
	pub fn error(msg: impl Into<String>) -> Self {
		Self::from_effect(Effect::Error(msg.into()))
	}

	/// Triggers a screen-relative motion.
	#[inline]
	pub fn screen_motion(position: ScreenPosition, count: usize) -> Self {
		Self::from_effect(Effect::ScreenMotion { position, count })
	}

	/// Executes an edit action.
	#[inline]
	pub fn edit(action: EditAction) -> Self {
		Self::from_effect(Effect::Edit(action))
	}

	/// Enters pending state for multi-key action.
	#[inline]
	pub fn pending(action: PendingAction) -> Self {
		Self::from_effect(Effect::Pending(action))
	}
}

impl<E: Into<Effect>> From<E> for ActionEffects {
	fn from(effect: E) -> Self {
		Self::from_effect(effect.into())
	}
}

impl IntoIterator for ActionEffects {
	type Item = Effect;
	type IntoIter = std::vec::IntoIter<Effect>;

	fn into_iter(self) -> Self::IntoIter {
		self.effects.into_iter()
	}
}

impl<'a> IntoIterator for &'a ActionEffects {
	type Item = &'a Effect;
	type IntoIter = std::slice::Iter<'a, Effect>;

	fn into_iter(self) -> Self::IntoIter {
		self.effects.iter()
	}
}

/// Primitive state mutation.
///
/// Effects are the atomic units of editor state change. Unlike `ActionResult`
/// variants which represent compound behaviors, effects represent single
/// state mutations that can be composed.
///
/// # Categories
///
/// - **Cursor/Selection**: `SetCursor`, `SetSelection`, `ScreenMotion`
/// - **Mode**: `SetMode`, `Pending`
/// - **Text**: `Edit` (wraps `EditAction`)
/// - **Navigation**: `FocusBuffer`, `FocusSplit`, `Split`, `CloseSplit`
/// - **UI**: `Notify`, `OpenPalette`, `ClosePalette`, `ExecutePalette`
/// - **Lifecycle**: `Quit`, `ForceRedraw`
/// - **Search**: `Search`, `UseSelectionAsSearch`
/// - **Deferred**: `QueueCommand`
#[derive(Debug, Clone)]
pub enum Effect {
	/// Set cursor to absolute position.
	SetCursor(CharIdx),

	/// Set selection (includes cursor at primary head).
	SetSelection(Selection),

	/// Move cursor to screen-relative position (H/M/L).
	ScreenMotion {
		/// Screen-relative position.
		position: ScreenPosition,
		/// 1-based offset from the target edge.
		count: usize,
	},

	/// Change editor mode.
	SetMode(Mode),

	/// Enter pending state for multi-key action.
	Pending(PendingAction),

	/// Execute an edit action (delete, change, yank, etc.).
	Edit(EditAction),

	/// Switch buffer in sequential direction.
	FocusBuffer(SeqDirection),

	/// Focus split in spatial direction.
	FocusSplit(SpatialDirection),

	/// Create a new split.
	Split(Axis),

	/// Close current split.
	CloseSplit,

	/// Close all other buffers.
	CloseOtherBuffers,

	/// Show a notification.
	Notify(Notification),

	/// Display an error message.
	Error(String),

	/// Open command palette.
	OpenPalette,

	/// Close command palette.
	ClosePalette,

	/// Execute command in palette.
	ExecutePalette,

	/// Force a redraw.
	ForceRedraw,

	/// Search in direction.
	Search {
		/// Direction to search.
		direction: SeqDirection,
		/// Whether to add matches to existing selections.
		add_selection: bool,
	},

	/// Use current selection as search pattern.
	UseSelectionAsSearch,

	/// Quit the editor.
	Quit {
		/// Whether to force quit without save prompts.
		force: bool,
	},

	/// Queue a command for async execution.
	QueueCommand {
		/// Command name.
		name: &'static str,
		/// Command arguments.
		args: Vec<String>,
	},
}

impl From<Selection> for Effect {
	fn from(sel: Selection) -> Self {
		Effect::SetSelection(sel)
	}
}

impl From<Mode> for Effect {
	fn from(mode: Mode) -> Self {
		Effect::SetMode(mode)
	}
}

impl From<EditAction> for Effect {
	fn from(action: EditAction) -> Self {
		Effect::Edit(action)
	}
}

impl From<PendingAction> for Effect {
	fn from(action: PendingAction) -> Self {
		Effect::Pending(action)
	}
}

impl From<Notification> for Effect {
	fn from(notification: Notification) -> Self {
		Effect::Notify(notification)
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn test_effects_composition() {
		let sel = Selection::single(10, 10);
		let effects = ActionEffects::motion(sel.clone()).with(Effect::SetMode(Mode::Insert));

		assert_eq!(effects.len(), 2);
		assert!(matches!(effects.as_slice()[0], Effect::SetSelection(_)));
		assert!(matches!(
			effects.as_slice()[1],
			Effect::SetMode(Mode::Insert)
		));
	}

	#[test]
	fn test_effects_ok_is_empty() {
		let effects = ActionEffects::ok();
		assert!(effects.is_empty());
	}

	#[test]
	fn test_from_effect() {
		let effects: ActionEffects = Effect::SetMode(Mode::Normal).into();
		assert_eq!(effects.len(), 1);
	}
}
