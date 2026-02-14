//! Undo/redo history types.
//!
//! This module defines the types used for undo/redo history at both the document
//! and editor levels. Document history stores only document state (text content),
//! while editor-level undo groups store view state (selections, cursors, scroll).
//!
//! # History Layers
//!
//! * [`DocumentHistoryEntry`]: Document-only state (text content and version).
//! * [`ViewSnapshot`]: Per-buffer view state (cursor, selection, scroll position).
//! * [`EditorUndoGroup`]: Editor-level grouping that combines affected documents
//!   with their corresponding view snapshots.

use std::collections::HashMap;

use xeno_primitives::range::CharIdx;
use xeno_primitives::{EditOrigin, Rope, Selection};

use crate::buffer::{DocumentId, ViewId};

/// Document-level undo entry storing only document state.
///
/// This replaces the old `HistoryEntry` which incorrectly stored per-buffer
/// selections. Document history should be view-agnostic - the same document
/// can be viewed in multiple buffers with different selections.
#[derive(Clone)]
pub struct DocumentHistoryEntry {
	/// Document content at this point in history.
	pub rope: Rope,
	/// Document version at this point in history.
	pub version: u64,
}

/// Snapshot of a buffer's view state for undo/redo restoration.
///
/// Captured at undo group boundaries and restored when undoing/redoing.
/// This enables restoring the exact cursor position, selection, and scroll
/// position that existed before an edit operation.
#[derive(Debug, Clone)]
pub struct ViewSnapshot {
	/// Primary cursor position (char index).
	pub cursor: CharIdx,
	/// Multi-cursor selection state.
	pub selection: Selection,
	/// First visible line.
	pub scroll_line: usize,
	/// First visible segment within the line (for wrapped lines).
	pub scroll_segment: usize,
}

/// Editor-level undo group bundling affected documents with view state.
///
/// When an edit operation affects one or more documents, the editor creates
/// an undo group that tracks:
/// * Which documents were modified
/// * The view state of each buffer before the edit
/// * The origin of the edit (for debugging and telemetry)
///
/// On undo, the editor calls each document's undo method and then restores
/// the view snapshots to their corresponding buffers.
#[derive(Clone, Debug)]
pub struct EditorUndoGroup {
	/// Documents affected by this undo group.
	pub affected_docs: Vec<DocumentId>,
	/// View snapshots for each buffer at the time of the edit.
	pub view_snapshots: HashMap<ViewId, ViewSnapshot>,
	/// Origin of this edit group.
	pub origin: EditOrigin,
}
