//! Background syntax loading manager.
//!
//! Tracks pending parse tasks per [`DocumentId`], using version-based invalidation
//! to discard stale results when documents change during parsing.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

use ropey::Rope;
use tokio::task::JoinHandle;
use xeno_runtime_language::syntax::{Syntax, SyntaxError};
use xeno_runtime_language::{LanguageId, LanguageLoader};

use crate::buffer::DocumentId;

const PARSE_DEBOUNCE: Duration = Duration::from_millis(100);

/// Background syntax loading manager.
///
/// Spawns parse tasks via [`tokio::task::spawn_blocking`] and polls for completion
/// without blocking the render path.
pub struct SyntaxManager {
	pending: HashMap<DocumentId, PendingSyntaxTask>,
	last_edit_at: HashMap<DocumentId, Instant>,
}

struct PendingSyntaxTask {
	doc_version: u64,
	task: JoinHandle<Result<Syntax, SyntaxError>>,
}

/// Result of polling syntax state.
#[derive(Debug)]
pub enum SyntaxPollResult {
	/// Syntax is ready.
	Ready,
	/// Parse is pending in background.
	Pending,
	/// Parse was kicked off.
	Kicked,
	/// No language configured for this document.
	NoLanguage,
}

impl Default for SyntaxManager {
	fn default() -> Self {
		Self::new()
	}
}

impl SyntaxManager {
	pub fn new() -> Self {
		Self {
			pending: HashMap::new(),
			last_edit_at: HashMap::new(),
		}
	}

	/// Records an edit, canceling any stale pending parse.
	pub fn note_edit(&mut self, doc_id: DocumentId) {
		self.last_edit_at.insert(doc_id, Instant::now());
		if let Some(task) = self.pending.remove(&doc_id) {
			task.task.abort();
		}
	}

	/// Cleans up tracking state for a closed document.
	pub fn on_document_close(&mut self, doc_id: DocumentId) {
		if let Some(task) = self.pending.remove(&doc_id) {
			task.task.abort();
		}
		self.last_edit_at.remove(&doc_id);
	}

	/// Polls or kicks background syntax parsing.
	///
	/// Non-blocking: returns immediately if syntax ready, kicks a background
	/// parse if needed, or polls a pending parse and installs results.
	pub fn ensure_syntax(
		&mut self,
		doc_id: DocumentId,
		doc_version: u64,
		language_id: Option<LanguageId>,
		content: &Rope,
		current_syntax: &mut Option<Syntax>,
		syntax_dirty: &mut bool,
		loader: &Arc<LanguageLoader>,
	) -> SyntaxPollResult {
		let Some(lang_id) = language_id else {
			return SyntaxPollResult::NoLanguage;
		};

		if current_syntax.is_some() && !*syntax_dirty {
			return SyntaxPollResult::Ready;
		}

		if let Some(pending) = self.pending.get_mut(&doc_id) {
			if pending.doc_version != doc_version {
				self.pending.remove(&doc_id).unwrap().task.abort();
			} else if pending.task.is_finished() {
				let task = self.pending.remove(&doc_id).unwrap();
				match futures::executor::block_on(task.task) {
					Ok(Ok(syntax)) => {
						*current_syntax = Some(syntax);
						*syntax_dirty = false;
						return SyntaxPollResult::Ready;
					}
					Ok(Err(e)) => {
						tracing::warn!(doc_id = ?doc_id, error = %e, "Background syntax parse failed");
						return SyntaxPollResult::NoLanguage;
					}
					Err(e) => {
						tracing::warn!(doc_id = ?doc_id, error = %e, "Background syntax task panicked");
						return SyntaxPollResult::NoLanguage;
					}
				}
			} else {
				return SyntaxPollResult::Pending;
			}
		}

		if self
			.last_edit_at
			.get(&doc_id)
			.is_some_and(|t| t.elapsed() < PARSE_DEBOUNCE)
		{
			return SyntaxPollResult::Pending;
		}

		self.kick_parse(doc_id, doc_version, lang_id, content.clone(), loader);
		SyntaxPollResult::Kicked
	}

	fn kick_parse(
		&mut self,
		doc_id: DocumentId,
		doc_version: u64,
		language: LanguageId,
		content: Rope,
		loader: &Arc<LanguageLoader>,
	) {
		let loader = Arc::clone(loader);
		let task =
			tokio::task::spawn_blocking(move || Syntax::new(content.slice(..), language, &loader));
		self.pending
			.insert(doc_id, PendingSyntaxTask { doc_version, task });
	}

	pub fn has_pending(&self, doc_id: DocumentId) -> bool {
		self.pending.contains_key(&doc_id)
	}

	pub fn pending_count(&self) -> usize {
		self.pending.len()
	}
}
