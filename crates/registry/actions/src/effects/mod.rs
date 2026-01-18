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
//!     .with(ViewEffect::SetSelection(sel))
//!     .with(AppEffect::SetMode(Mode::Insert))
//! ```
//!
//! [`ActionResult`]: crate::ActionResult

use xeno_primitives::direction::{Axis, SeqDirection, SpatialDirection};
use xeno_primitives::range::{CharIdx, Direction};
use xeno_primitives::{Mode, Selection};
use xeno_registry_notifications::Notification;

use crate::{PendingAction, ScreenPosition};

#[cfg(test)]
mod tests;

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
		Self::from_effect(ViewEffect::SetCursor(pos).into())
	}

	/// Sets the selection (motion result).
	#[inline]
	pub fn motion(sel: Selection) -> Self {
		Self::from_effect(ViewEffect::SetSelection(sel).into())
	}

	/// Changes the editor mode.
	#[inline]
	pub fn mode(mode: Mode) -> Self {
		Self::from_effect(AppEffect::SetMode(mode).into())
	}

	/// Quits the editor.
	#[inline]
	pub fn quit() -> Self {
		Self::from_effect(AppEffect::Quit { force: false }.into())
	}

	/// Force quits the editor.
	#[inline]
	pub fn force_quit() -> Self {
		Self::from_effect(AppEffect::Quit { force: true }.into())
	}

	/// Shows an error message.
	#[inline]
	pub fn error(msg: impl Into<String>) -> Self {
		Self::from_effect(UiEffect::Error(msg.into()).into())
	}

	/// Triggers a screen-relative motion.
	#[inline]
	pub fn screen_motion(position: ScreenPosition, count: usize) -> Self {
		Self::from_effect(ViewEffect::ScreenMotion { position, count }.into())
	}

	/// Executes a data-oriented edit operation.
	#[inline]
	pub fn edit_op(op: crate::edit_op::EditOp) -> Self {
		Self::from_effect(EditEffect::EditOp(op).into())
	}

	/// Scrolls the viewport.
	#[inline]
	pub fn scroll(direction: Direction, amount: ScrollAmount, extend: bool) -> Self {
		Self::from_effect(
			ViewEffect::Scroll {
				direction,
				amount,
				extend,
			}
			.into(),
		)
	}

	/// Moves cursor visually (wrapped lines).
	#[inline]
	pub fn visual_move(direction: Direction, count: usize, extend: bool) -> Self {
		Self::from_effect(
			ViewEffect::VisualMove {
				direction,
				count,
				extend,
			}
			.into(),
		)
	}

	/// Pastes from yank register.
	#[inline]
	pub fn paste(before: bool) -> Self {
		Self::from_effect(EditEffect::Paste { before }.into())
	}

	/// Enters pending state for multi-key action.
	#[inline]
	pub fn pending(action: PendingAction) -> Self {
		Self::from_effect(AppEffect::Pending(action).into())
	}
}

impl<E: Into<Effect>> From<E> for ActionEffects {
	fn from(effect: E) -> Self {
		Self::from_effect(effect.into())
	}
}

/// Amount to scroll (lines or page fraction).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScrollAmount {
	/// Scroll by a specific number of lines.
	Line(usize),
	/// Scroll by half a page.
	HalfPage,
	/// Scroll by a full page.
	FullPage,
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

/// View-related effects (cursor, selection, viewport).
#[derive(Debug, Clone)]
#[non_exhaustive]
pub enum ViewEffect {
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

	/// Scroll the viewport.
	Scroll {
		/// Direction to scroll (Forward = down, Backward = up).
		direction: Direction,
		/// How much to scroll.
		amount: ScrollAmount,
		/// Whether to extend selection while scrolling.
		extend: bool,
	},

	/// Move cursor visually (wrapped lines).
	VisualMove {
		/// Direction to move (Forward = down, Backward = up).
		direction: Direction,
		/// Number of visual lines to move.
		count: usize,
		/// Whether to extend selection rather than move.
		extend: bool,
	},

	/// Search in direction.
	Search {
		/// Direction to search.
		direction: SeqDirection,
		/// Whether to add matches to existing selections.
		add_selection: bool,
	},

	/// Use current selection as search pattern.
	UseSelectionAsSearch,
}

/// Text editing effects.
#[derive(Debug, Clone)]
#[non_exhaustive]
pub enum EditEffect {
	/// Execute a data-oriented edit operation.
	///
	/// This is the preferred way to express text edits. EditOp records
	/// are composable and processed by a single executor function.
	EditOp(crate::edit_op::EditOp),

	/// Paste from yank register.
	Paste {
		/// Whether to paste before cursor (vs after).
		before: bool,
	},
}

/// UI-related effects (notifications, palette, redraw).
#[derive(Debug, Clone)]
#[non_exhaustive]
pub enum UiEffect {
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
}

/// Application-level effects (mode, focus, lifecycle).
#[derive(Debug, Clone)]
#[non_exhaustive]
pub enum AppEffect {
	/// Change editor mode.
	SetMode(Mode),

	/// Enter pending state for multi-key action.
	Pending(PendingAction),

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

/// Primitive state mutation.
///
/// Effects are the atomic units of editor state change. Unlike `ActionResult`
/// variants which represent compound behaviors, effects represent single
/// state mutations that can be composed.
///
/// # Categories
///
/// Effects are organized into domain-specific nested enums:
///
/// - [`ViewEffect`]: Cursor, selection, viewport, search
/// - [`EditEffect`]: Text modifications (EditOp, Paste)
/// - [`UiEffect`]: Notifications, palette, redraw
/// - [`AppEffect`]: Mode, focus, splits, lifecycle
///
/// # Ordering Invariants
///
/// Effects are applied **sequentially in the order they appear** in the
/// `ActionEffects` collection. The interpreter does not reorder effects.
///
/// ## Semantic ordering expectations
///
/// When composing effects, follow these conventions:
///
/// 1. **Cursor/Selection before Mode** - Set cursor/selection before changing
///    mode, so mode-entry logic sees the correct position.
///    ```ignore
///    ActionEffects::motion(sel).with(Effect::App(AppEffect::SetMode(Mode::Insert)))
///    ```
///
/// 2. **EditOp is self-contained** - `EditOp` effects handle their own cursor
///    updates internally. Don't combine `SetCursor` with `EditOp` for the same
///    logical edit.
///
/// 3. **Notifications are side effects** - Place `Notify` effects at the end
///    since they don't affect subsequent effects.
///
/// 4. **Quit short-circuits outcome** - Once `Quit` is processed, the return
///    outcome becomes `HandleOutcome::Quit`. Subsequent effects still execute
///    but the final outcome is quit.
///
/// ## Hook emissions
///
/// - `SetCursor` emits `CursorMove` hook
/// - `SetSelection` emits both `CursorMove` and `SelectionChange` hooks
/// - `ScreenMotion` emits both hooks after computing the target position
///
/// These hooks fire immediately after each effect, not batched at the end.
#[derive(Debug, Clone)]
#[non_exhaustive]
pub enum Effect {
	/// View-related effect (cursor, selection, viewport, search).
	View(ViewEffect),

	/// Text editing effect.
	Edit(EditEffect),

	/// UI-related effect (notifications, palette).
	Ui(UiEffect),

	/// Application-level effect (mode, focus, lifecycle).
	App(AppEffect),
}

impl From<ViewEffect> for Effect {
	fn from(effect: ViewEffect) -> Self {
		Effect::View(effect)
	}
}

impl From<EditEffect> for Effect {
	fn from(effect: EditEffect) -> Self {
		Effect::Edit(effect)
	}
}

impl From<UiEffect> for Effect {
	fn from(effect: UiEffect) -> Self {
		Effect::Ui(effect)
	}
}

impl From<AppEffect> for Effect {
	fn from(effect: AppEffect) -> Self {
		Effect::App(effect)
	}
}

impl From<Selection> for Effect {
	fn from(sel: Selection) -> Self {
		Effect::View(ViewEffect::SetSelection(sel))
	}
}

impl From<Mode> for Effect {
	fn from(mode: Mode) -> Self {
		Effect::App(AppEffect::SetMode(mode))
	}
}

impl From<crate::edit_op::EditOp> for Effect {
	fn from(op: crate::edit_op::EditOp) -> Self {
		Effect::Edit(EditEffect::EditOp(op))
	}
}

impl From<PendingAction> for Effect {
	fn from(action: PendingAction) -> Self {
		Effect::App(AppEffect::Pending(action))
	}
}

impl From<Notification> for Effect {
	fn from(notification: Notification) -> Self {
		Effect::Ui(UiEffect::Notify(notification))
	}
}
