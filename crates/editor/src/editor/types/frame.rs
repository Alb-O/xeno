//! Per-frame runtime state.

use std::collections::HashSet;

use crate::buffer::{BufferId, BufferView};

/// Per-frame runtime state.
///
/// Groups hot fields that are accessed every frame for better cache locality.
/// These fields change frequently during normal editor operation.
pub struct FrameState {
	/// Whether a redraw is needed.
	pub needs_redraw: bool,
	/// Whether a command requested the editor to quit.
	pub pending_quit: bool,
	/// Last tick timestamp.
	pub last_tick: std::time::SystemTime,
	/// Buffers with pending content changes for `BufferChange` hooks.
	pub dirty_buffers: HashSet<BufferId>,
	/// Views with sticky focus (resist mouse hover focus changes).
	pub sticky_views: HashSet<BufferView>,
}

impl Default for FrameState {
	fn default() -> Self {
		Self {
			needs_redraw: false,
			pending_quit: false,
			last_tick: std::time::SystemTime::now(),
			dirty_buffers: HashSet::new(),
			sticky_views: HashSet::new(),
		}
	}
}
