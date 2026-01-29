//! Buffer storage, ID generation, and focus tracking.
//!
//! [`ViewManager`] centralizes ownership of text buffers.

use std::collections::HashMap;
use std::path::PathBuf;

use smallvec::SmallVec;
use xeno_runtime_language::LanguageLoader;

use crate::buffer::{Buffer, DocumentId, ViewId};

/// Owns text buffers and tracks document associations.
///
/// Maintains a reverse index from `DocumentId` to `ViewId` for O(1) lookup
/// when acquiring snapshots for LSP sync.
pub struct ViewManager {
	buffers: HashMap<ViewId, Buffer>,
	doc_to_views: HashMap<DocumentId, SmallVec<[ViewId; 2]>>,
	next_buffer_id: u64,
}

impl ViewManager {
	/// Creates a manager with an initial buffer (ID 1).
	pub fn new(content: String, path: Option<PathBuf>, language_loader: &LanguageLoader) -> Self {
		let buffer_id = ViewId(1);
		let buffer = Buffer::new(buffer_id, content, path);
		buffer.init_syntax(language_loader);
		let doc_id = buffer.document_id();

		let mut buffers = HashMap::new();
		buffers.insert(buffer_id, buffer);

		let mut doc_to_views: HashMap<DocumentId, SmallVec<[ViewId; 2]>> = HashMap::new();
		doc_to_views.insert(doc_id, smallvec::smallvec![buffer_id]);

		Self {
			buffers,
			doc_to_views,
			next_buffer_id: 2,
		}
	}

	/// Creates a manager with an existing buffer.
	pub fn with_buffer(buffer: Buffer) -> Self {
		let buffer_id = buffer.id;
		let doc_id = buffer.document_id();

		let mut buffers = HashMap::new();
		buffers.insert(buffer_id, buffer);

		let mut doc_to_views: HashMap<DocumentId, SmallVec<[ViewId; 2]>> = HashMap::new();
		doc_to_views.insert(doc_id, smallvec::smallvec![buffer_id]);

		Self {
			buffers,
			doc_to_views,
			next_buffer_id: buffer_id.0 + 1,
		}
	}

	/// Creates a new buffer with syntax highlighting.
	pub fn create_buffer(
		&mut self,
		content: String,
		path: Option<PathBuf>,
		language_loader: &LanguageLoader,
		window_width: Option<u16>,
	) -> ViewId {
		let buffer_id = ViewId(self.next_buffer_id);
		self.next_buffer_id += 1;

		let mut buffer = Buffer::new(buffer_id, content, path);
		buffer.init_syntax(language_loader);

		if let Some(width) = window_width {
			buffer.text_width = width.saturating_sub(buffer.gutter_width()) as usize;
		}

		let doc_id = buffer.document_id();
		self.buffers.insert(buffer_id, buffer);
		self.index_add(doc_id, buffer_id);
		buffer_id
	}

	/// Creates an empty scratch buffer without syntax highlighting.
	pub fn create_scratch(&mut self) -> ViewId {
		let buffer_id = ViewId(self.next_buffer_id);
		self.next_buffer_id += 1;

		let buffer = Buffer::new(buffer_id, String::new(), None);
		let doc_id = buffer.document_id();
		self.buffers.insert(buffer_id, buffer);
		self.index_add(doc_id, buffer_id);
		buffer_id
	}

	pub(crate) fn next_buffer_id(&mut self) -> u64 {
		let id = self.next_buffer_id;
		self.next_buffer_id += 1;
		id
	}

	/// Creates a new buffer that shares the same document as the specified buffer.
	///
	/// The new buffer has independent cursor/selection/scroll state but
	/// edits in either buffer affect both (they share the same Document).
	pub fn clone_buffer_for_split(&mut self, source_id: ViewId) -> Option<ViewId> {
		let source_buffer = self.buffers.get(&source_id)?;
		let new_id = ViewId(self.next_buffer_id);
		self.next_buffer_id += 1;

		let new_buffer = source_buffer.clone_for_split(new_id);
		let doc_id = new_buffer.document_id();
		debug_assert_eq!(
			doc_id,
			source_buffer.document_id(),
			"split buffer must share document"
		);
		self.buffers.insert(new_id, new_buffer);
		self.index_add(doc_id, new_id);
		Some(new_id)
	}

	/// Inserts a buffer into the manager and updates the reverse index.
	pub(crate) fn insert_buffer(&mut self, id: ViewId, buffer: Buffer) {
		let doc_id = buffer.document_id();
		self.buffers.insert(id, buffer);
		self.index_add(doc_id, id);
	}

	/// Removes a buffer without performing document-level cleanup.
	///
	/// Internal use only. Callers should typically use [`Editor::finalize_buffer_removal`]
	/// to ensure document-level cleanup (cache invalidation, LSP sync).
	///
	/// [`Editor::finalize_buffer_removal`]: crate::impls::Editor::finalize_buffer_removal
	pub(crate) fn remove_buffer_raw(&mut self, id: ViewId) -> Option<Buffer> {
		let removed = self.buffers.remove(&id);
		if let Some(ref buffer) = removed {
			self.index_remove(buffer.document_id(), id);
		}
		removed
	}

	/// Returns a buffer by ID.
	pub fn get_buffer(&self, id: ViewId) -> Option<&Buffer> {
		self.buffers.get(&id)
	}

	/// Returns a buffer mutably by ID.
	pub fn get_buffer_mut(&mut self, id: ViewId) -> Option<&mut Buffer> {
		self.buffers.get_mut(&id)
	}

	/// Returns an iterator over all buffer IDs.
	pub fn buffer_ids(&self) -> impl Iterator<Item = ViewId> + '_ {
		self.buffers.keys().copied()
	}

	/// Returns the number of open text buffers.
	pub fn buffer_count(&self) -> usize {
		self.buffers.len()
	}

	/// Returns an iterator over all buffers.
	pub fn buffers(&self) -> impl Iterator<Item = &Buffer> {
		self.buffers.values()
	}

	/// Returns a mutable iterator over all buffers.
	pub fn buffers_mut(&mut self) -> impl Iterator<Item = &mut Buffer> {
		self.buffers.values_mut()
	}

	/// Finds a buffer by its file path.
	pub fn find_by_path(&self, path: &std::path::Path) -> Option<ViewId> {
		self.buffers
			.values()
			.find(|b| b.path().as_deref() == Some(path))
			.map(|b| b.id)
	}

	/// Returns any view ID associated with the given document.
	pub fn any_buffer_for_doc(&self, doc_id: DocumentId) -> Option<ViewId> {
		self.doc_to_views
			.get(&doc_id)
			.and_then(|views| views.first().copied())
	}

	/// Adds a view to the reverse index for a document.
	fn index_add(&mut self, doc_id: DocumentId, view_id: ViewId) {
		self.doc_to_views.entry(doc_id).or_default().push(view_id);
	}

	/// Removes a view from the reverse index for a document.
	fn index_remove(&mut self, doc_id: DocumentId, view_id: ViewId) {
		if let Some(views) = self.doc_to_views.get_mut(&doc_id) {
			views.retain(|v| *v != view_id);
			if views.is_empty() {
				self.doc_to_views.remove(&doc_id);
			}
		}
	}
}
