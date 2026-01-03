use std::collections::HashMap;

use termina::event::KeyEvent;
use xeno_base::range::CharIdx;
use xeno_base::{Rope, Selection};
use xeno_core::CompletionItem;

use crate::buffer::BufferId;

/// Undo/redo history entry storing document state and per-view selections.
#[derive(Clone)]
pub struct HistoryEntry {
	/// Document content at this point in history.
	pub doc: Rope,
	/// Per-buffer selections at this point in history.
	pub selections: HashMap<BufferId, Selection>,
}

/// Named registers for storing yanked text.
#[derive(Default)]
pub struct Registers {
	/// Default yank register content.
	pub yank: String,
}

/// A saved position in the jump list.
///
/// Jump locations track buffer and cursor position for navigation commands
/// like `<C-o>` (jump back) and `<C-i>` (jump forward).
#[derive(Clone, Debug)]
pub struct JumpLocation {
	/// The buffer containing this jump.
	pub buffer_id: BufferId,
	/// Cursor position at time of jump.
	pub cursor: CharIdx,
}

/// Jump list for navigating between locations.
///
/// Maintains a history of cursor positions with a current index for
/// forward/backward navigation. Similar to Vim's jumplist.
#[derive(Default)]
pub struct JumpList {
	/// Stack of jump locations.
	locations: Vec<JumpLocation>,
	/// Current position in the jump list (points after the "current" location).
	index: usize,
}

impl JumpList {
	/// Maximum number of jumps to remember.
	const MAX_JUMPS: usize = 100;

	/// Saves the current position to the jump list.
	///
	/// Truncates any forward history and appends the new location.
	pub fn push(&mut self, location: JumpLocation) {
		// Truncate forward history when pushing a new jump
		self.locations.truncate(self.index);
		self.locations.push(location);

		if self.locations.len() > Self::MAX_JUMPS {
			self.locations.remove(0);
		} else {
			self.index = self.locations.len();
		}
	}

	/// Jumps backward in history, returning the location to jump to.
	///
	/// Returns `None` if at the beginning of the list.
	pub fn jump_backward(&mut self) -> Option<&JumpLocation> {
		if self.index > 0 {
			self.index -= 1;
			self.locations.get(self.index)
		} else {
			None
		}
	}

	/// Jumps forward in history, returning the location to jump to.
	///
	/// Returns `None` if at the end of the list.
	pub fn jump_forward(&mut self) -> Option<&JumpLocation> {
		if self.index < self.locations.len() {
			let loc = self.locations.get(self.index);
			self.index += 1;
			loc
		} else {
			None
		}
	}
}

/// State for macro recording and playback.
#[derive(Default)]
pub struct MacroState {
	/// Currently recording macro register (None if not recording).
	recording_register: Option<char>,
	/// Keys recorded so far for the current macro.
	recording_keys: Vec<KeyEvent>,
	/// Stored macros by register.
	macros: HashMap<char, Vec<KeyEvent>>,
	/// Last used macro register for `@@` replay.
	last_register: Option<char>,
}

impl MacroState {
	/// Starts recording a macro into the given register.
	///
	/// If already recording, stops the current recording first.
	pub fn start_recording(&mut self, register: char) {
		if self.recording_register.is_some() {
			self.stop_recording();
		}
		self.recording_register = Some(register);
		self.recording_keys.clear();
	}

	/// Stops recording and saves the macro to its register.
	pub fn stop_recording(&mut self) {
		if let Some(register) = self.recording_register.take() {
			let keys = std::mem::take(&mut self.recording_keys);
			if !keys.is_empty() {
				self.macros.insert(register, keys);
				self.last_register = Some(register);
			}
		}
	}

	/// Records a key event if currently recording.
	pub fn record_key(&mut self, key: KeyEvent) {
		if self.recording_register.is_some() {
			self.recording_keys.push(key);
		}
	}

	/// Returns the macro for a register, if any.
	pub fn get(&self, register: char) -> Option<&[KeyEvent]> {
		self.macros.get(&register).map(|v| v.as_slice())
	}

	/// Returns the last used macro register.
	pub fn last_register(&self) -> Option<char> {
		self.last_register
	}

	/// Returns true if currently recording.
	pub fn is_recording(&self) -> bool {
		self.recording_register.is_some()
	}

	/// Returns the register currently being recorded, if any.
	pub fn recording_register(&self) -> Option<char> {
		self.recording_register
	}
}

/// State for managing the completion menu.
#[derive(Clone, Default)]
pub struct CompletionState {
	/// Available completion items.
	pub items: Vec<CompletionItem>,
	/// Index of the currently selected item.
	pub selected_idx: Option<usize>,
	/// Whether the completion menu is active and visible.
	pub active: bool,
	/// Start position in the input where replacement begins.
	/// When a completion is accepted, text from this position to cursor is replaced.
	pub replace_start: usize,
	/// Scroll offset for the completion menu viewport.
	/// This is the index of the first visible item when there are more items than can fit.
	pub scroll_offset: usize,
}

impl CompletionState {
	/// Maximum number of visible items in the completion menu.
	pub const MAX_VISIBLE: usize = 10;

	/// Ensures the selected item is visible within the viewport by adjusting scroll_offset.
	pub fn ensure_selected_visible(&mut self) {
		let Some(selected) = self.selected_idx else {
			return;
		};

		// If selection is above the viewport, scroll up
		if selected < self.scroll_offset {
			self.scroll_offset = selected;
		}

		// If selection is below the viewport, scroll down
		let visible_end = self.scroll_offset + Self::MAX_VISIBLE;
		if selected >= visible_end {
			self.scroll_offset = selected.saturating_sub(Self::MAX_VISIBLE - 1);
		}
	}

	/// Returns the range of visible items (start..end indices).
	pub fn visible_range(&self) -> std::ops::Range<usize> {
		let end = (self.scroll_offset + Self::MAX_VISIBLE).min(self.items.len());
		self.scroll_offset..end
	}
}
