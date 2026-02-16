//! Per-frame runtime state.

use std::collections::{HashSet, VecDeque};

use crate::buffer::ViewId;

/// Deferred work item produced by synchronous editor paths and consumed in `pump`.
pub enum DeferredWorkItem {
	/// Deferred modal commit request.
	OverlayCommit,
	/// Deferred workspace edit from async overlay flows.
	#[cfg(feature = "lsp")]
	ApplyWorkspaceEdit(xeno_lsp::lsp_types::WorkspaceEdit),
}

/// Queue of deferred work items drained by the runtime pump.
#[derive(Default)]
pub struct DeferredWorkQueue {
	items: VecDeque<DeferredWorkItem>,
}

impl DeferredWorkQueue {
	/// Appends one deferred work item.
	pub fn push(&mut self, item: DeferredWorkItem) {
		self.items.push_back(item);
	}

	/// Returns whether any deferred overlay commit is pending.
	pub fn has_overlay_commit(&self) -> bool {
		self.items.iter().any(|item| matches!(item, DeferredWorkItem::OverlayCommit))
	}

	/// Removes and returns one deferred overlay commit request, if present.
	pub fn take_overlay_commit(&mut self) -> bool {
		let Some(index) = self.items.iter().position(|item| matches!(item, DeferredWorkItem::OverlayCommit)) else {
			return false;
		};

		self.items.remove(index).is_some()
	}

	/// Drains all pending deferred workspace edits.
	#[cfg(feature = "lsp")]
	pub fn take_workspace_edits(&mut self) -> Vec<xeno_lsp::lsp_types::WorkspaceEdit> {
		let mut edits = Vec::new();
		let mut retained = VecDeque::with_capacity(self.items.len());
		while let Some(item) = self.items.pop_front() {
			match item {
				DeferredWorkItem::ApplyWorkspaceEdit(edit) => edits.push(edit),
				other => retained.push_back(other),
			}
		}
		self.items = retained;
		edits
	}

	/// Returns the number of pending deferred workspace edits.
	#[cfg(feature = "lsp")]
	pub fn pending_workspace_edits(&self) -> usize {
		self.items.iter().filter(|item| matches!(item, DeferredWorkItem::ApplyWorkspaceEdit(_))).count()
	}
}

/// Per-frame runtime state.
///
/// Groups hot fields that are accessed every frame for better cache locality.
/// These fields change frequently during normal editor operation.
pub struct FrameState {
	/// Whether a redraw is needed.
	pub needs_redraw: bool,
	/// Whether a command requested the editor to quit.
	pub pending_quit: bool,
	/// Deferred runtime work consumed by [`crate::Editor::pump`].
	pub deferred_work: DeferredWorkQueue,
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
			deferred_work: DeferredWorkQueue::default(),
			last_tick: std::time::SystemTime::now(),
			dirty_buffers: HashSet::new(),
			sticky_views: HashSet::new(),
		}
	}
}
