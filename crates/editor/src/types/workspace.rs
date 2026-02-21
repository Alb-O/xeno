//! Editing session state.

use std::collections::{HashMap, VecDeque};

use xeno_primitives::{CharIdx, Key};

use crate::buffer::ViewId;

/// Represents yanked content, preserving individual selection fragments.
#[derive(Debug, Clone, Default)]
pub struct Yank {
	/// Text fragments from each selection range.
	pub parts: Vec<String>,
	/// Total character count across all parts.
	pub total_chars: usize,
}

impl Yank {
	/// Returns the flattened string representation of the yanked content.
	pub fn joined(&self) -> String {
		self.parts.join("\n")
	}

	/// Returns true if the yanked content is empty.
	pub fn is_empty(&self) -> bool {
		self.parts.is_empty()
	}
}

/// Named registers for storing yanked text.
#[derive(Default)]
pub struct Registers {
	/// Default yank register content.
	pub yank: Yank,
}

/// A saved position in the jump list.
#[derive(Clone, Debug)]
pub struct JumpLocation {
	/// The buffer containing this jump.
	pub buffer_id: ViewId,
	/// Cursor position at time of jump.
	pub cursor: CharIdx,
}

/// Jump list for navigating between locations.
///
/// Maintains a history of cursor positions with a current index for
/// forward/backward navigation. Similar to Vim's jumplist.
#[derive(Default)]
pub struct JumpList {
	locations: Vec<JumpLocation>,
	index: usize,
}

impl JumpList {
	const MAX_JUMPS: usize = 100;

	/// Saves the current position to the jump list.
	pub fn push(&mut self, location: JumpLocation) {
		self.locations.truncate(self.index);
		self.locations.push(location);

		if self.locations.len() > Self::MAX_JUMPS {
			self.locations.remove(0);
		} else {
			self.index = self.locations.len();
		}
	}

	/// Jumps backward in history. Returns `None` if at the beginning.
	pub fn jump_backward(&mut self) -> Option<&JumpLocation> {
		if self.index > 0 {
			self.index -= 1;
			self.locations.get(self.index)
		} else {
			None
		}
	}

	/// Jumps forward in history. Returns `None` if at the end.
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
	recording_register: Option<char>,
	recording_keys: Vec<Key>,
	macros: HashMap<char, Vec<Key>>,
	last_register: Option<char>,
}

impl MacroState {
	/// Starts recording a macro into the given register.
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
	pub fn record_key(&mut self, key: Key) {
		if self.recording_register.is_some() {
			self.recording_keys.push(key);
		}
	}

	/// Returns the macro for a register, if any.
	pub fn get(&self, register: char) -> Option<&[Key]> {
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

/// Per-session key-value store for Nu script state persistence.
///
/// Bounded FIFO store: when full, oldest entries are evicted on insert.
/// Keys and values are plain strings capped at the invocation string limit.
/// Provides ordered iteration for XENO_CTX serialization.
#[derive(Default)]
pub struct NuState {
	entries: VecDeque<(String, String)>,
}

impl NuState {
	/// Maximum number of state entries.
	pub const MAX_ENTRIES: usize = 64;

	/// Set a key-value pair. Updates in-place if key exists, otherwise appends.
	/// Evicts the oldest entry if at capacity.
	pub fn set(&mut self, key: String, value: String) {
		if let Some(pos) = self.entries.iter().position(|(k, _)| k == &key) {
			self.entries[pos].1 = value;
			return;
		}
		if self.entries.len() >= Self::MAX_ENTRIES {
			self.entries.pop_front();
		}
		self.entries.push_back((key, value));
	}

	/// Remove a key. No-op if key doesn't exist.
	pub fn unset(&mut self, key: &str) {
		if let Some(pos) = self.entries.iter().position(|(k, _)| k == key) {
			self.entries.remove(pos);
		}
	}

	/// Iterate over entries in insertion order.
	pub fn iter(&self) -> impl Iterator<Item = (&str, &str)> {
		self.entries.iter().map(|(k, v)| (k.as_str(), v.as_str()))
	}
}

/// Editing session state.
///
/// Groups workspace-level state that persists across buffer switches:
/// registers, jump list, macros, and Nu script state.
#[derive(Default)]
pub struct Workspace {
	/// Named registers (yank buffer, etc.).
	pub registers: Registers,
	/// Jump list for navigation.
	pub jump_list: JumpList,
	/// Macro recording and playback state.
	pub macro_state: MacroState,
	/// Per-session Nu script state store.
	pub nu_state: NuState,
}
