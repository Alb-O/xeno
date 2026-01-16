use std::collections::HashMap;

use tracing::warn;
use xeno_primitives::{Selection, SyntaxPolicy, Transaction, UndoPolicy};
use xeno_registry_notifications::keys;
use xeno_registry_notifications::Notification;

use crate::buffer::{ApplyPolicy, BufferId, DocumentId};
use crate::buffer_manager::BufferManager;
use crate::impls::messaging::push_notification;
use crate::types::{Config, FrameState, UndoHost, ViewSnapshot};

pub(super) struct EditorUndoHost<'a> {
	pub buffers: &'a mut BufferManager,
	pub config: &'a Config,
	pub frame: &'a mut FrameState,
	pub notifications: &'a mut xeno_tui::widgets::notifications::ToastManager,
	#[cfg(feature = "lsp")]
	pub lsp: &'a mut crate::lsp::LspManager,
}

impl EditorUndoHost<'_> {
	pub(super) fn apply_transaction_inner(
		&mut self,
		buffer_id: BufferId,
		tx: &Transaction,
		new_selection: Option<Selection>,
		undo: UndoPolicy,
	) -> bool {
		let policy = ApplyPolicy {
			undo,
			syntax: SyntaxPolicy::IncrementalOrDirty,
		};

		#[cfg(feature = "lsp")]
		let encoding = {
			let buffer = self
				.buffers
				.get_buffer(buffer_id)
				.expect("focused buffer must exist");
			self.lsp.incremental_encoding_for_buffer(buffer)
		};

		#[cfg(feature = "lsp")]
		let applied = {
			let buffer = self
				.buffers
				.get_buffer_mut(buffer_id)
				.expect("focused buffer must exist");
			let applied = if let Some(encoding) = encoding {
				buffer.apply_with_lsp(tx, policy, &self.config.language_loader, encoding)
			} else {
				buffer.apply(tx, policy, &self.config.language_loader)
			};
			if applied && let Some(selection) = new_selection {
				buffer.finalize_selection(selection);
			}
			applied
		};

		#[cfg(not(feature = "lsp"))]
		let applied = {
			let buffer = self
				.buffers
				.get_buffer_mut(buffer_id)
				.expect("focused buffer must exist");
			let applied = buffer.apply(tx, policy, &self.config.language_loader);
			if applied && let Some(selection) = new_selection {
				buffer.finalize_selection(selection);
			}
			applied
		};

		if applied {
			self.sync_sibling_selections(buffer_id, tx);
			self.frame.dirty_buffers.insert(buffer_id);
		}

		applied
	}

	fn notify(&mut self, notification: impl Into<Notification>) {
		push_notification(self.config, self.notifications, notification.into());
	}

	fn mark_buffer_dirty_for_full_sync(&mut self, buffer_id: BufferId) {
		if let Some(buffer) = self.buffers.get_buffer_mut(buffer_id) {
			buffer.with_doc_mut(|doc| {
				doc.increment_version();
				#[cfg(feature = "lsp")]
				doc.mark_for_full_lsp_sync();
			});
		}
		self.frame.dirty_buffers.insert(buffer_id);
	}

	fn sync_sibling_selections(&mut self, buffer_id: BufferId, tx: &Transaction) {
		let doc_id = self
			.buffers
			.get_buffer(buffer_id)
			.expect("focused buffer must exist")
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
			self.mark_buffer_dirty_for_full_sync(buffer_id);
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
			self.mark_buffer_dirty_for_full_sync(buffer_id);
		}
		ok
	}
}

impl UndoHost for EditorUndoHost<'_> {
	fn guard_readonly(&mut self) -> bool {
		if self.buffers.focused_buffer().is_readonly() {
			self.notify(keys::buffer_readonly);
			return false;
		}
		true
	}

	fn doc_id_for_buffer(&self, buffer_id: BufferId) -> DocumentId {
		self.buffers
			.get_buffer(buffer_id)
			.expect("buffer must exist")
			.document_id()
	}

	fn collect_view_snapshots(&self, doc_id: DocumentId) -> HashMap<BufferId, ViewSnapshot> {
		self.buffers
			.buffers()
			.filter(|b| b.document_id() == doc_id)
			.map(|b| (b.id, b.snapshot_view()))
			.collect()
	}

	fn capture_current_view_snapshots(
		&self,
		doc_ids: &[DocumentId],
	) -> HashMap<BufferId, ViewSnapshot> {
		self.buffers
			.buffers()
			.filter(|b| doc_ids.contains(&b.document_id()))
			.map(|b| (b.id, b.snapshot_view()))
			.collect()
	}

	fn restore_view_snapshots(&mut self, snapshots: &HashMap<BufferId, ViewSnapshot>) {
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

	fn doc_insert_undo_active(&self, buffer_id: BufferId) -> bool {
		self.buffers
			.get_buffer(buffer_id)
			.map(|b| b.with_doc(|doc| doc.insert_undo_active()))
			.unwrap_or(false)
	}

	fn notify_undo(&mut self) {
		self.notify(keys::undo);
	}

	fn notify_redo(&mut self) {
		self.notify(keys::redo);
	}

	fn notify_nothing_to_undo(&mut self) {
		self.notify(keys::nothing_to_undo);
	}

	fn notify_nothing_to_redo(&mut self) {
		self.notify(keys::nothing_to_redo);
	}
}
