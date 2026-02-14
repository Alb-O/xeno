//! Undo host adapter for the editor.
//!
//! Provides a bridge between the generic [`UndoManager`] and the concrete
//! [`Editor`] implementation, enabling view state restoration and document
//! history coordination.

use std::collections::HashMap;

use tracing::{trace, trace_span, warn};
use xeno_primitives::transaction::Operation;
use xeno_primitives::{ChangeSet, Selection, SyntaxPolicy, Transaction, UndoPolicy};
use xeno_registry::notifications::{Notification, keys};

use crate::buffer::{ApplyPolicy, DocumentId, ViewId};
use crate::impls::messaging::push_notification;
#[cfg(feature = "lsp")]
use crate::lsp::LspSystem;
use crate::types::{Config, FrameState, UndoHost, ViewSnapshot};
use crate::view_manager::ViewManager;

/// Concrete implementation of [`UndoHost`] for the editor.
pub(super) struct EditorUndoHost<'a> {
	pub buffers: &'a mut ViewManager,
	pub focused_view: ViewId,
	pub config: &'a Config,
	pub frame: &'a mut FrameState,
	pub notifications: &'a mut crate::notifications::NotificationCenter,
	pub syntax_manager: &'a mut crate::syntax_manager::SyntaxManager,
	#[cfg(feature = "lsp")]
	pub lsp: &'a mut LspSystem,
}

fn summarize_operations(ops: &[Operation]) -> (usize, usize, usize, usize, usize, i64) {
	let mut op_count = 0usize;
	let mut retain_chars = 0usize;
	let mut deleted_chars = 0usize;
	let mut inserted_bytes = 0usize;
	let mut inserted_chars = 0usize;
	for op in ops {
		op_count += 1;
		match op {
			Operation::Retain(n) => {
				retain_chars += *n;
			}
			Operation::Delete(n) => {
				deleted_chars += *n;
			}
			Operation::Insert(ins) => {
				inserted_bytes += ins.byte_len();
				inserted_chars += ins.char_len();
			}
		}
	}
	let net_char_delta = inserted_chars as i64 - deleted_chars as i64;
	(op_count, retain_chars, deleted_chars, inserted_bytes, inserted_chars, net_char_delta)
}

fn summarize_txs(txs: &[Transaction]) -> (usize, usize, usize, usize, usize, usize, i64) {
	let mut op_count = 0usize;
	let mut retain_chars = 0usize;
	let mut deleted_chars = 0usize;
	let mut inserted_bytes = 0usize;
	let mut inserted_chars = 0usize;
	for tx in txs {
		let (ops, retain, deleted, inserted_b, inserted_c, _delta) = summarize_operations(tx.operations());
		op_count += ops;
		retain_chars += retain;
		deleted_chars += deleted;
		inserted_bytes += inserted_b;
		inserted_chars += inserted_c;
	}
	let net_char_delta = inserted_chars as i64 - deleted_chars as i64;
	(txs.len(), op_count, retain_chars, deleted_chars, inserted_bytes, inserted_chars, net_char_delta)
}

fn summarize_changeset(changeset: &ChangeSet) -> (usize, usize, usize, usize, usize, i64) {
	summarize_operations(changeset.changes())
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
			let buffer = self.buffers.get_buffer(buffer_id).expect("buffer must exist");
			buffer.with_doc(|doc| doc.content().clone())
		};

		let result = {
			let buffer = self.buffers.get_buffer_mut(buffer_id).expect("buffer must exist");
			let result = buffer.apply(tx, policy);
			if result.applied
				&& let Some(selection) = new_selection
			{
				buffer.finalize_selection(selection);
			}
			result
		};

		#[cfg(feature = "lsp")]
		{
			let buffer = self.buffers.get_buffer(buffer_id).expect("buffer must exist");
			self.lsp.on_local_edit(buffer, Some(before_rope.clone()), tx, &result);
		}

		if result.applied {
			let buffer = self.buffers.get_buffer(buffer_id).expect("buffer must exist");
			let doc_id = buffer.document_id();
			let (after_rope, version) = buffer.with_doc(|doc| (doc.content().clone(), doc.version()));
			self.syntax_manager.note_edit_incremental(
				doc_id,
				version,
				&before_rope,
				&after_rope,
				tx.changes(),
				&self.config.language_loader,
				crate::syntax_manager::EditSource::Typing,
			);
			self.sync_all_view_selections_for_doc(doc_id, std::slice::from_ref(tx), Some(buffer_id));
			for id in self.buffers.buffer_ids() {
				if self.buffers.get_buffer(id).is_some_and(|b| b.document_id() == doc_id) {
					self.frame.dirty_buffers.insert(id);
				}
			}
			self.frame.dirty_buffers.insert(buffer_id);
		}

		result
	}

	fn notify(&mut self, notification: impl Into<Notification>) {
		push_notification(self.notifications, notification.into());
		self.frame.needs_redraw = true;
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
	fn sync_all_view_selections_for_doc(&mut self, doc_id: DocumentId, txs: &[Transaction], exclude_view: Option<ViewId>) {
		let view_ids: Vec<_> = self
			.buffers
			.buffer_ids()
			.filter(|&id| Some(id) != exclude_view)
			.filter(|&id| self.buffers.get_buffer(id).is_some_and(|b| b.document_id() == doc_id))
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
			.filter(|&id| self.buffers.get_buffer(id).is_some_and(|b| b.document_id() == doc_id))
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
	fn apply_history_op(&mut self, doc_id: DocumentId, op: impl FnOnce(&mut crate::buffer::Document) -> Option<Vec<Transaction>>) -> bool {
		let span = trace_span!(target: "xeno_undo_trace", "undo_host.apply_history_op", ?doc_id);
		let _span_guard = span.enter();
		let buffer_id = self.buffers.buffers().find(|b| b.document_id() == doc_id).map(|b| b.id);

		let Some(buffer_id) = buffer_id else {
			warn!(doc_id = ?doc_id, "History op: no buffer for document");
			trace!(target: "xeno_undo_trace", result = "no_buffer");
			return false;
		};

		let (before_rope, before_version) = self
			.buffers
			.get_buffer(buffer_id)
			.expect("buffer exists")
			.with_doc(|doc| (doc.content().clone(), doc.version()));
		trace!(
			target: "xeno_undo_trace",
			?buffer_id,
			before_version,
			before_bytes = before_rope.len_bytes(),
			"undo_host.history.before"
		);

		let txs = self.buffers.get_buffer_mut(buffer_id).expect("buffer exists").with_doc_mut(op);

		let Some(txs) = txs else {
			trace!(target: "xeno_undo_trace", result = "no_transactions");
			return false;
		};
		let (tx_count, op_count, retain_chars, deleted_chars, inserted_bytes, inserted_chars, net_char_delta) = summarize_txs(&txs);
		trace!(
			target: "xeno_undo_trace",
			tx_count,
			op_count,
			retain_chars,
			deleted_chars,
			inserted_chars,
			inserted_bytes,
			net_char_delta,
			"undo_host.history.transactions"
		);

		let (after_rope, version) = self
			.buffers
			.get_buffer(buffer_id)
			.expect("buffer exists")
			.with_doc(|doc| (doc.content().clone(), doc.version()));
		trace!(
			target: "xeno_undo_trace",
			after_version = version,
			after_bytes = after_rope.len_bytes(),
			version_delta = version as i64 - before_version as i64,
			byte_delta = after_rope.len_bytes() as i64 - before_rope.len_bytes() as i64,
			"undo_host.history.after"
		);

		// Compose changes for incremental syntax update
		if !txs.is_empty() {
			let mut net_changes = txs[0].changes().clone();
			for tx in &txs[1..] {
				net_changes = net_changes.compose(tx.changes().clone());
			}
			let (net_ops, net_retain, net_deleted, net_inserted_bytes, net_inserted_chars, net_delta_chars) = summarize_changeset(&net_changes);
			trace!(
				target: "xeno_undo_trace",
				net_ops,
				net_retain,
				net_deleted,
				net_inserted_chars,
				net_inserted_bytes,
				net_delta_chars,
				"undo_host.history.net_changes"
			);

			self.syntax_manager.note_edit_incremental(
				doc_id,
				version,
				&before_rope,
				&after_rope,
				&net_changes,
				&self.config.language_loader,
				crate::syntax_manager::EditSource::History,
			);
		}

		self.sync_all_view_selections_for_doc(doc_id, &txs, None);
		for id in self.buffers.buffer_ids() {
			if self.buffers.get_buffer(id).is_some_and(|b| b.document_id() == doc_id) {
				self.frame.dirty_buffers.insert(id);
			}
		}
		self.mark_buffer_dirty_for_full_sync(buffer_id);
		self.normalize_all_views_for_doc(doc_id);
		trace!(target: "xeno_undo_trace", result = "ok");
		true
	}
}

impl UndoHost for EditorUndoHost<'_> {
	fn guard_readonly(&mut self) -> bool {
		let buffer = self.buffers.get_buffer(self.focused_view).expect("focused buffer must exist");
		if buffer.is_readonly() {
			self.notify(keys::BUFFER_READONLY);
			return false;
		}
		true
	}

	fn doc_id_for_buffer(&self, buffer_id: ViewId) -> DocumentId {
		self.buffers.get_buffer(buffer_id).expect("buffer must exist").document_id()
	}

	fn collect_view_snapshots(&self, doc_id: DocumentId) -> HashMap<ViewId, ViewSnapshot> {
		self.buffers
			.buffers()
			.filter(|b| b.document_id() == doc_id)
			.map(|b| (b.id, b.snapshot_view()))
			.collect()
	}

	fn capture_current_view_snapshots(&self, doc_ids: &[DocumentId]) -> HashMap<ViewId, ViewSnapshot> {
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
