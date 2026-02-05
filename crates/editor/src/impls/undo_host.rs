//! Undo host adapter for the editor.
//!
//! Provides a bridge between the generic [`UndoManager`] and the concrete
//! [`Editor`] implementation, enabling view state restoration and document
//! history coordination.

use std::collections::HashMap;

use tracing::warn;
use xeno_primitives::{Selection, SyntaxPolicy, Transaction, UndoPolicy};
use xeno_registry::notifications::{Notification, keys};

use crate::buffer::{ApplyPolicy, DocumentId, ViewId};
use crate::impls::messaging::push_notification;
use crate::types::{Config, FrameState, UndoHost, ViewSnapshot};
use crate::view_manager::ViewManager;

/// Concrete implementation of [`UndoHost`] for the editor.
pub(super) struct EditorUndoHost<'a> {
	pub buffers: &'a mut ViewManager,
	pub focused_view: ViewId,
	pub config: &'a Config,
	pub frame: &'a mut FrameState,
	pub notifications: &'a mut xeno_tui::widgets::notifications::ToastManager,
	pub syntax_manager: &'a mut crate::syntax_manager::SyntaxManager,
	#[cfg(feature = "lsp")]
	pub lsp: &'a mut crate::LspSystem,
}

impl EditorUndoHost<'_> {
	/// Applies a transaction to a specific buffer with full LSP and selection sync.
	pub(super) fn apply_transaction_inner(
		&mut self,
		buffer_id: ViewId,
		tx: &Transaction,
		new_selection: Option<Selection>,
		undo: UndoPolicy,
	) -> xeno_primitives::CommitResult {
		let policy = ApplyPolicy {
			undo,
			syntax: SyntaxPolicy::IncrementalOrDirty,
		};

		let before_rope = {
			let buffer = self
				.buffers
				.get_buffer(buffer_id)
				.expect("buffer must exist");
			buffer.with_doc(|doc| doc.content().clone())
		};

		let result = {
			let buffer = self
				.buffers
				.get_buffer_mut(buffer_id)
				.expect("buffer must exist");
			let result = buffer.apply(tx, policy, &self.config.language_loader);
			if result.applied
				&& let Some(selection) = new_selection
			{
				buffer.finalize_selection(selection);
			}
			result
		};

		#[cfg(feature = "lsp")]
		{
			let buffer = self
				.buffers
				.get_buffer(buffer_id)
				.expect("buffer must exist");
			self.lsp
				.on_local_edit(buffer, Some(before_rope.clone()), tx, &result);
		}

		if result.applied {
			let buffer = self
				.buffers
				.get_buffer(buffer_id)
				.expect("buffer must exist");
			let doc_id = buffer.document_id();
			let (after_rope, version) =
				buffer.with_doc(|doc| (doc.content().clone(), doc.version()));
			self.syntax_manager.note_edit_incremental(
				doc_id,
				version,
				&before_rope,
				&after_rope,
				tx.changes(),
				&self.config.language_loader,
			);
			self.sync_all_view_selections_for_doc(
				doc_id,
				std::slice::from_ref(tx),
				Some(buffer_id),
			);
			for id in self.buffers.buffer_ids() {
				if self
					.buffers
					.get_buffer(id)
					.is_some_and(|b| b.document_id() == doc_id)
				{
					self.frame.dirty_buffers.insert(id);
				}
			}
			self.frame.dirty_buffers.insert(buffer_id);
		}

		result
	}

	fn notify(&mut self, notification: impl Into<Notification>) {
		push_notification(self.config, self.notifications, notification.into());
	}

	fn mark_buffer_dirty_for_full_sync(&mut self, buffer_id: ViewId) {
		if let Some(_buffer) = self.buffers.get_buffer_mut(buffer_id) {
			#[cfg(feature = "lsp")]
			{
				let doc_id = _buffer.document_id();
				self.lsp.sync_manager_mut().escalate_full(doc_id);
			}
		}
		self.frame.dirty_buffers.insert(buffer_id);
	}

	/// Synchronizes selections of all sibling buffers viewing the same document.
	/// Synchronizes selections of all views viewing the same document.
	fn sync_all_view_selections_for_doc(
		&mut self,
		doc_id: DocumentId,
		txs: &[Transaction],
		exclude_view: Option<ViewId>,
	) {
		let view_ids: Vec<_> = self
			.buffers
			.buffer_ids()
			.filter(|&id| Some(id) != exclude_view)
			.filter(|&id| {
				self.buffers
					.get_buffer(id)
					.is_some_and(|b| b.document_id() == doc_id)
			})
			.collect();

		for view_id in view_ids {
			if let Some(view) = self.buffers.get_buffer_mut(view_id) {
				for tx in txs {
					view.map_selection_through(tx);
				}
				view.ensure_valid_selection();
				view.debug_assert_valid_state();
			}
		}
	}

	/// Clamps selections and cursors for all views of a document to valid bounds.
	fn normalize_all_views_for_doc(&mut self, doc_id: DocumentId) {
		let view_ids: Vec<_> = self
			.buffers
			.buffer_ids()
			.filter(|&id| {
				self.buffers
					.get_buffer(id)
					.is_some_and(|b| b.document_id() == doc_id)
			})
			.collect();

		for view_id in view_ids {
			if let Some(buffer) = self.buffers.get_buffer_mut(view_id) {
				buffer.ensure_valid_selection();
				buffer.debug_assert_valid_state();
			}
		}
	}

	fn undo_document(&mut self, doc_id: DocumentId) -> bool {
		self.apply_history_op(doc_id, |doc| doc.undo())
	}

	fn redo_document(&mut self, doc_id: DocumentId) -> bool {
		self.apply_history_op(doc_id, |doc| doc.redo())
	}

	/// Shared implementation for history operations.
	///
	/// Applies the operation to the document and synchronizes all associated views,
	/// including incremental syntax updates and selection mapping.
	fn apply_history_op(
		&mut self,
		doc_id: DocumentId,
		op: impl FnOnce(&mut crate::buffer::Document) -> Option<Vec<Transaction>>,
	) -> bool {
		let buffer_id = self
			.buffers
			.buffers()
			.find(|b| b.document_id() == doc_id)
			.map(|b| b.id);

		let Some(buffer_id) = buffer_id else {
			warn!(doc_id = ?doc_id, "History op: no buffer for document");
			return false;
		};

		let before_rope = self
			.buffers
			.get_buffer(buffer_id)
			.expect("buffer exists")
			.with_doc(|doc| doc.content().clone());

		let txs = self
			.buffers
			.get_buffer_mut(buffer_id)
			.expect("buffer exists")
			.with_doc_mut(op);

		let Some(txs) = txs else {
			return false;
		};

		let (after_rope, version) = self
			.buffers
			.get_buffer(buffer_id)
			.expect("buffer exists")
			.with_doc(|doc| (doc.content().clone(), doc.version()));

		// Compose changes for incremental syntax update
		if !txs.is_empty() {
			let mut net_changes = txs[0].changes().clone();
			for tx in &txs[1..] {
				net_changes = net_changes.compose(tx.changes().clone());
			}

			self.syntax_manager.note_edit_incremental(
				doc_id,
				version,
				&before_rope,
				&after_rope,
				&net_changes,
				&self.config.language_loader,
			);
		}

		self.sync_all_view_selections_for_doc(doc_id, &txs, None);
		for id in self.buffers.buffer_ids() {
			if self
				.buffers
				.get_buffer(id)
				.is_some_and(|b| b.document_id() == doc_id)
			{
				self.frame.dirty_buffers.insert(id);
			}
		}
		self.mark_buffer_dirty_for_full_sync(buffer_id);
		self.normalize_all_views_for_doc(doc_id);
		true
	}
}

impl UndoHost for EditorUndoHost<'_> {
	fn guard_readonly(&mut self) -> bool {
		let buffer = self
			.buffers
			.get_buffer(self.focused_view)
			.expect("focused buffer must exist");
		if buffer.is_readonly() {
			self.notify(keys::BUFFER_READONLY);
			return false;
		}
		true
	}

	fn doc_id_for_buffer(&self, buffer_id: ViewId) -> DocumentId {
		self.buffers
			.get_buffer(buffer_id)
			.expect("buffer must exist")
			.document_id()
	}

	fn collect_view_snapshots(&self, doc_id: DocumentId) -> HashMap<ViewId, ViewSnapshot> {
		self.buffers
			.buffers()
			.filter(|b| b.document_id() == doc_id)
			.map(|b| (b.id, b.snapshot_view()))
			.collect()
	}

	fn capture_current_view_snapshots(
		&self,
		doc_ids: &[DocumentId],
	) -> HashMap<ViewId, ViewSnapshot> {
		self.buffers
			.buffers()
			.filter(|b| doc_ids.contains(&b.document_id()))
			.map(|b| (b.id, b.snapshot_view()))
			.collect()
	}

	fn restore_view_snapshots(&mut self, snapshots: &HashMap<ViewId, ViewSnapshot>) {
		for buffer in self.buffers.buffers_mut() {
			if let Some(snapshot) = snapshots.get(&buffer.id) {
				buffer.restore_view(snapshot);
			}
		}
	}

	fn undo_documents(&mut self, doc_ids: &[DocumentId]) -> bool {
		doc_ids.iter().all(|&id| self.undo_document(id))
	}

	fn redo_documents(&mut self, doc_ids: &[DocumentId]) -> bool {
		doc_ids.iter().all(|&id| self.redo_document(id))
	}

	fn notify_undo(&mut self) {
		self.notify(keys::UNDO);
	}

	fn notify_redo(&mut self) {
		self.notify(keys::REDO);
	}

	fn notify_nothing_to_undo(&mut self) {
		self.notify(keys::NOTHING_TO_UNDO);
	}

	fn notify_nothing_to_redo(&mut self) {
		self.notify(keys::NOTHING_TO_REDO);
	}
}
