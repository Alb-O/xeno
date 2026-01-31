use std::collections::HashMap;

use tracing::warn;
use xeno_primitives::{Selection, SyntaxPolicy, Transaction, UndoPolicy};
use xeno_registry::notifications::{Notification, keys};

use crate::buffer::{ApplyPolicy, DocumentId, ViewId};
use crate::impls::messaging::push_notification;
use crate::types::{Config, FrameState, UndoHost, ViewSnapshot};
use crate::view_manager::ViewManager;

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

		#[cfg(feature = "lsp")]
		let (encoding, doc_id) = {
			let buffer = self
				.buffers
				.get_buffer(buffer_id)
				.expect("buffer must exist");
			(
				self.lsp.incremental_encoding_for_buffer(buffer),
				buffer.document_id(),
			)
		};

		#[cfg(feature = "lsp")]
		let result = {
			let buffer = self
				.buffers
				.get_buffer_mut(buffer_id)
				.expect("buffer must exist");
			let result = if let Some(encoding) = encoding {
				let lsp_result =
					buffer.apply_with_lsp(tx, policy, &self.config.language_loader, encoding);

				if lsp_result.commit.applied {
					let prev_version = lsp_result.prev_version();
					let new_version = lsp_result.new_version();
					match lsp_result.lsp_changes {
						Some(changes) if !changes.is_empty() => {
							self.lsp.sync_manager_mut().on_doc_edit(
								doc_id,
								prev_version,
								new_version,
								changes,
								lsp_result.lsp_bytes,
							);
						}
						Some(_) => {}
						None => self.lsp.sync_manager_mut().escalate_full(doc_id),
					}
				}

				lsp_result.commit
			} else {
				let result = buffer.apply(tx, policy, &self.config.language_loader);
				// No incremental support - trigger full sync if edit applied
				if result.applied {
					self.lsp.sync_manager_mut().escalate_full(doc_id);
				}
				result
			};
			if result.applied
				&& let Some(selection) = new_selection
			{
				buffer.finalize_selection(selection);
			}
			result
		};

		#[cfg(not(feature = "lsp"))]
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

		if result.applied {
			let doc_id = self
				.buffers
				.get_buffer(buffer_id)
				.expect("buffer must exist")
				.document_id();
			self.syntax_manager.note_edit(doc_id);
			self.sync_sibling_selections(buffer_id, tx);
			self.frame.dirty_buffers.insert(buffer_id);
		}

		result
	}

	fn notify(&mut self, notification: impl Into<Notification>) {
		push_notification(self.config, self.notifications, notification.into());
	}

	fn mark_buffer_dirty_for_full_sync(&mut self, buffer_id: ViewId) {
		if let Some(buffer) = self.buffers.get_buffer_mut(buffer_id) {
			#[cfg(feature = "lsp")]
			let doc_id = buffer.document_id();

			buffer.with_doc_mut(|doc| {
				doc.increment_version();
			});

			#[cfg(feature = "lsp")]
			self.lsp.sync_manager_mut().escalate_full(doc_id);
		}
		self.frame.dirty_buffers.insert(buffer_id);
	}

	fn sync_sibling_selections(&mut self, buffer_id: ViewId, tx: &Transaction) {
		let doc_id = self
			.buffers
			.get_buffer(buffer_id)
			.expect("buffer must exist")
			.document_id();

		let sibling_ids: Vec<_> = self
			.buffers
			.buffer_ids()
			.filter(|&id| id != buffer_id)
			.filter(|&id| {
				self.buffers
					.get_buffer(id)
					.is_some_and(|b| b.document_id() == doc_id)
			})
			.collect();

		for sibling_id in sibling_ids {
			if let Some(sibling) = self.buffers.get_buffer_mut(sibling_id) {
				sibling.map_selection_through(tx);
				sibling.ensure_valid_selection();
				sibling.debug_assert_valid_state();
			}
		}
	}

	/// Clamps selections and cursors for all views of a document to valid bounds.
	///
	/// Call after any document mutation to ensure no view holds stale positions.
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
		let buffer_id = self
			.buffers
			.buffers()
			.find(|b| b.document_id() == doc_id)
			.map(|b| b.id);

		let Some(buffer_id) = buffer_id else {
			warn!(doc_id = ?doc_id, "Undo: no buffer for document");
			return false;
		};

		let ok = self
			.buffers
			.get_buffer_mut(buffer_id)
			.expect("buffer exists")
			.with_doc_mut(|doc| doc.undo(&self.config.language_loader));

		if ok {
			self.syntax_manager.note_edit(doc_id);
			self.mark_buffer_dirty_for_full_sync(buffer_id);
			self.normalize_all_views_for_doc(doc_id);
		}
		ok
	}

	fn redo_document(&mut self, doc_id: DocumentId) -> bool {
		let buffer_id = self
			.buffers
			.buffers()
			.find(|b| b.document_id() == doc_id)
			.map(|b| b.id);

		let Some(buffer_id) = buffer_id else {
			warn!(doc_id = ?doc_id, "Redo: no buffer for document");
			return false;
		};

		let ok = self
			.buffers
			.get_buffer_mut(buffer_id)
			.expect("buffer exists")
			.with_doc_mut(|doc| doc.redo(&self.config.language_loader));

		if ok {
			self.syntax_manager.note_edit(doc_id);
			self.mark_buffer_dirty_for_full_sync(buffer_id);
			self.normalize_all_views_for_doc(doc_id);
		}
		ok
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
			} else {
				buffer.ensure_valid_selection();
			}
		}
	}

	fn undo_documents(&mut self, doc_ids: &[DocumentId]) -> bool {
		let mut ok = true;
		for &doc_id in doc_ids {
			ok &= self.undo_document(doc_id);
		}
		ok
	}

	fn redo_documents(&mut self, doc_ids: &[DocumentId]) -> bool {
		let mut ok = true;
		for &doc_id in doc_ids {
			ok &= self.redo_document(doc_id);
		}
		ok
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
