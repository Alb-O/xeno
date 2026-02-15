//! Per-frame runtime state.

use std::collections::HashSet;

use crate::buffer::ViewId;

/// Per-frame runtime state.
///
/// Groups hot fields that are accessed every frame for better cache locality.
/// These fields change frequently during normal editor operation.
pub struct FrameState {
	/// Whether a redraw is needed.
	pub needs_redraw: bool,
	/// Whether a command requested the editor to quit.
	pub pending_quit: bool,
	/// Deferred overlay commit awaiting runtime pump.
	///
	/// Set when a `CloseModal { Commit }` effect arrives during the
	/// synchronous flush loop; consumed by [`crate::Editor::pump`].
	pub pending_overlay_commit: bool,
	/// Workspace edits queued from async overlay tasks.
	#[cfg(feature = "lsp")]
	pub pending_workspace_edits: Vec<xeno_lsp::lsp_types::WorkspaceEdit>,
	/// Last tick timestamp.
	pub last_tick: std::time::SystemTime,
	/// Buffers with pending content changes for `BufferChange` hooks.
	pub dirty_buffers: HashSet<ViewId>,
	/// Views with sticky focus (resist mouse hover focus changes).
	pub sticky_views: HashSet<ViewId>,
}

impl Default for FrameState {
	fn default() -> Self {
		Self {
			needs_redraw: false,
			pending_quit: false,
			pending_overlay_commit: false,
			#[cfg(feature = "lsp")]
			pending_workspace_edits: Vec::new(),
			last_tick: std::time::SystemTime::now(),
			dirty_buffers: HashSet::new(),
			sticky_views: HashSet::new(),
		}
	}
}
