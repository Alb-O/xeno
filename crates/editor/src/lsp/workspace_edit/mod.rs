//! LSP workspace edit planning and application.
//!
//! Provides utilities for translating complex [`WorkspaceEdit`] payloads into
//! validated, executable plans that can be applied to local editor buffers.

use std::collections::{HashMap, HashSet};
use std::ops::Range;
use std::path::PathBuf;

use thiserror::Error;
use xeno_lsp::lsp_types::{
	AnnotatedTextEdit, CreateFile, DeleteFile, DocumentChangeOperation, DocumentChanges, OneOf, RenameFile, ResourceOp, TextDocumentEdit, TextEdit, Uri,
	WorkspaceEdit,
};
use xeno_lsp::{OffsetEncoding, lsp_range_to_char_range};
use xeno_primitives::range::CharIdx;
use xeno_primitives::transaction::{Change, Tendril};
use xeno_primitives::{EditOrigin, SyntaxPolicy, Transaction, UndoPolicy};

use crate::Editor;
use crate::buffer::{ApplyPolicy, ViewId};
use crate::types::EditorUndoGroup;

/// A validated, ready-to-apply workspace edit plan.
pub struct WorkspaceEditPlan {
	/// List of individual buffer edit plans.
	pub per_buffer: Vec<BufferEditPlan>,
}

impl WorkspaceEditPlan {
	/// Returns all buffer identifiers affected by this plan.
	pub fn affected_buffer_ids(&self) -> Vec<ViewId> {
		self.per_buffer.iter().map(|p| p.buffer_id).collect()
	}
}

/// Execution plan for a single buffer.
pub struct BufferEditPlan {
	/// Target buffer identifier.
	pub buffer_id: ViewId,
	/// Set of non-overlapping text edits.
	pub edits: Vec<PlannedTextEdit>,
	/// Whether the buffer was opened specifically for this edit.
	/// Tracked separately in [`apply_workspace_edit`] for cleanup; retained
	/// here for diagnostics.
	#[allow(dead_code)]
	pub opened_temporarily: bool,
}

/// A single text replacement for a character range.
pub struct PlannedTextEdit {
	/// Character range to replace.
	pub range: Range<CharIdx>,
	/// New text content.
	pub replacement: Tendril,
}

/// Workspace edit failure with optional index of the first failed change.
#[derive(Debug)]
pub struct ApplyEditFailure {
	/// The underlying error.
	pub error: ApplyError,
	/// Zero-based index of the first failed operation in `documentChanges`.
	/// `None` for edits using the `changes` field (no indexed operations).
	pub failed_change: Option<u32>,
}

impl std::fmt::Display for ApplyEditFailure {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		if let Some(idx) = self.failed_change {
			write!(f, "change {}: {}", idx, self.error)
		} else {
			self.error.fmt(f)
		}
	}
}

/// Errors occurring during workspace edit planning or application.
#[derive(Debug, Error)]
pub enum ApplyError {
	/// The provided URI could not be normalized to a local path.
	#[error("invalid uri: {0}")]
	InvalidUri(String),
	/// The workspace edit contains an operation variant not supported.
	#[error("unsupported workspace edit operation")]
	UnsupportedOperation,
	/// The target document could not be found or opened.
	#[error("buffer not found for uri: {0}")]
	BufferNotFound(String),
	/// Range coordinates could not be mapped to character offsets.
	#[error("failed to convert text edit range for {0}")]
	RangeConversionFailed(String),
	/// Multiple edits target the same document region.
	#[error("overlapping edits for {0}")]
	OverlappingEdits(String),
	/// The target buffer is read-only or blocked by synchronization.
	#[error("read-only buffer for {0}")]
	ReadOnly(String),
	/// A `TextDocumentEdit` carries a version that doesn't match the
	/// client's tracked version for that URI. The entire workspace edit
	/// is rejected to prevent stale edits from corrupting buffer state.
	#[error("LSP edit arrived stale; ignoring (document changed). uri={uri} expected={expected} actual={actual}")]
	VersionMismatch { uri: String, expected: i32, actual: i32 },
	/// A `TextDocumentEdit` carries a version but the document isn't
	/// tracked by the LSP state manager. Rejected because the server
	/// expects version-consistent state that the client can't verify.
	#[error("LSP edit for unknown document ignored. uri={uri} version={version}")]
	UntrackedVersionedDocument { uri: String, version: i32 },
	/// A resource create operation failed.
	#[error("create file failed: {uri} — {reason}")]
	CreateFailed { uri: String, reason: String },
	/// A resource rename operation failed.
	#[error("rename file failed: {old_uri} → {new_uri} — {reason}")]
	RenameFailed { old_uri: String, new_uri: String, reason: String },
	/// A resource delete operation failed.
	#[error("delete file failed: {uri} — {reason}")]
	DeleteFailed { uri: String, reason: String },
	/// A resource rename would overwrite a modified open buffer.
	#[error("rename blocked: target buffer is modified — {uri}")]
	RenameBlockedModified { uri: String },
	/// A resource delete would discard a modified open buffer.
	#[error("delete blocked: buffer is modified — {uri}")]
	DeleteBlockedModified { uri: String },
	/// A temporary buffer's content could not be written to disk after a
	/// successful workspace edit. The buffer is kept alive so edits are
	/// not silently lost.
	#[error("failed to write workspace edit to disk: {path} — {error}")]
	IoWriteFailed { path: String, error: String },
	/// Multiple temp buffers resolve to the same canonical path with
	/// differing content. Indicates a malformed workspace edit or
	/// symlink/case-folding collision.
	#[error("conflicting temp buffer writes to same target: {path}")]
	ConflictingTempSave { path: String },
}

impl Editor {
	/// Atomically applies a workspace edit across multiple buffers.
	///
	/// Temporary buffers opened during planning are always cleaned up,
	/// even if the edit fails partway through — no leaked buffer state.
	/// On success, temp buffer persistence uses two-phase commit: first
	/// all modified temps are atomically saved to disk, then (only if
	/// every save succeeds) all are closed. If any save fails, none are
	/// closed — the user can recover via the open buffer.
	///
	/// # Errors
	///
	/// Returns [`ApplyError`] if any part of the edit plan is invalid, if
	/// application to a buffer fails, or if a temp buffer cannot be saved.
	pub async fn apply_workspace_edit(&mut self, edit: WorkspaceEdit) -> Result<(), ApplyEditFailure> {
		// Route: `documentChanges: Operations` needs sequential processing for resource ops.
		if let Some(DocumentChanges::Operations(ref ops)) = edit.document_changes
			&& ops.iter().any(|op| matches!(op, DocumentChangeOperation::Op(_)))
		{
			return self.apply_workspace_edit_operations(edit).await;
		}

		// Existing plan-then-apply path (no resource ops, no failed_change index).
		let (plan_result, temp_buffers) = self.plan_workspace_edit(edit).await;
		let result = match plan_result {
			Err(e) => Err(ApplyEditFailure { error: e, failed_change: None }),
			Ok(plan) if plan.per_buffer.is_empty() => Ok(()),
			Ok(plan) => {
				self.begin_workspace_edit_group(&plan);
				let mut apply_result = Ok(());
				for buffer_plan in &plan.per_buffer {
					if let Err(e) = self.apply_buffer_edit_plan(buffer_plan) {
						apply_result = Err(ApplyEditFailure { error: e, failed_change: None });
						break;
					}
				}
				if apply_result.is_ok() {
					self.flush_lsp_sync_now(&plan.affected_buffer_ids());
				}
				apply_result
			}
		};
		if result.is_ok() {
			self.save_temp_buffers_atomic(&temp_buffers)
				.await
				.map_err(|e| ApplyEditFailure { error: e, failed_change: None })?;
		} else {
			for id in temp_buffers {
				self.close_headless_buffer(id).await;
			}
		}
		result
	}

	/// Validates and converts a [`WorkspaceEdit`] into an executable plan.
	///
	/// Returns `(plan_result, temp_buffer_ids)`. The caller must always
	/// close the temp buffers, even if planning fails, to avoid leaking
	/// buffers opened during URI resolution.
	async fn plan_workspace_edit(&mut self, edit: WorkspaceEdit) -> (Result<WorkspaceEditPlan, ApplyError>, Vec<ViewId>) {
		let mut temp_buffers = Vec::new();
		match self.plan_workspace_edit_inner(edit, &mut temp_buffers).await {
			Ok(plan) => (Ok(plan), temp_buffers),
			Err(e) => (Err(e), temp_buffers),
		}
	}

	async fn plan_workspace_edit_inner(&mut self, edit: WorkspaceEdit, temp_buffers: &mut Vec<ViewId>) -> Result<WorkspaceEditPlan, ApplyError> {
		let mut per_uri: HashMap<String, (Uri, Vec<TextEdit>)> = HashMap::new();
		if let Some(changes) = edit.changes {
			for (uri, edits) in changes {
				let key = uri.to_string();
				per_uri.entry(key).or_insert_with(|| (uri, Vec::new())).1.extend(edits);
			}
		}

		if let Some(document_changes) = edit.document_changes {
			match document_changes {
				DocumentChanges::Edits(edits) => {
					for edit in edits {
						self.collect_text_document_edit(edit, &mut per_uri)?;
					}
				}
				DocumentChanges::Operations(ops) => {
					for op in ops {
						match op {
							DocumentChangeOperation::Edit(edit) => {
								self.collect_text_document_edit(edit, &mut per_uri)?;
							}
							DocumentChangeOperation::Op(_) => {
								return Err(ApplyError::UnsupportedOperation);
							}
						}
					}
				}
			}
		}

		let mut per_buffer = Vec::new();
		for (_uri_string, (uri, edits)) in per_uri {
			let (buffer_id, opened_temporarily) = self.resolve_uri_to_buffer(&uri).await?;
			if opened_temporarily {
				temp_buffers.push(buffer_id);
			}
			let buffer = self
				.state
				.core
				.buffers
				.get_buffer(buffer_id)
				.ok_or_else(|| ApplyError::BufferNotFound(uri.to_string()))?;

			let encoding = self.state.integration.lsp.offset_encoding_for_buffer(buffer);
			let mut planned_edits = Vec::new();
			for edit in edits {
				let planned = buffer
					.with_doc(|doc| convert_text_edit(doc.content(), encoding, &edit))
					.ok_or_else(|| ApplyError::RangeConversionFailed(uri.to_string()))?;
				planned_edits.push(planned);
			}

			coalesce_and_validate(&mut planned_edits, &uri)?;
			per_buffer.push(BufferEditPlan {
				buffer_id,
				edits: planned_edits,
				opened_temporarily,
			});
		}

		Ok(WorkspaceEditPlan { per_buffer })
	}

	/// Collects edits from a [`TextDocumentEdit`] into the per-URI map.
	///
	/// If the edit carries a version (`Some(v)`), validates that it matches
	/// the client's tracked version for the document. Rejects the entire
	/// workspace edit (all-or-nothing) on version mismatch or when the
	/// document isn't tracked but the server expects version consistency.
	fn collect_text_document_edit(&self, edit: TextDocumentEdit, per_uri: &mut HashMap<String, (Uri, Vec<TextEdit>)>) -> Result<(), ApplyError> {
		let uri = edit.text_document.uri;
		if let Some(expected) = edit.text_document.version {
			match self.state.integration.lsp.documents().get_version(&uri) {
				Some(actual) if actual != expected => {
					return Err(ApplyError::VersionMismatch {
						uri: uri.to_string(),
						expected,
						actual,
					});
				}
				None => {
					return Err(ApplyError::UntrackedVersionedDocument {
						uri: uri.to_string(),
						version: expected,
					});
				}
				_ => {}
			}
		}
		let key = uri.to_string();
		let edits = normalize_text_document_edits(edit.edits);
		per_uri.entry(key).or_insert_with(|| (uri, Vec::new())).1.extend(edits);
		Ok(())
	}

	/// Resolves a URI to an existing buffer or opens the file.
	///
	/// Returns `(view_id, opened_temporarily)`. If the buffer is already
	/// open, returns its existing view_id without closing/reopening —
	/// workspace edits must not invalidate existing buffer identities.
	async fn resolve_uri_to_buffer(&mut self, uri: &Uri) -> Result<(ViewId, bool), ApplyError> {
		let path = xeno_lsp::path_from_uri(uri).ok_or_else(|| ApplyError::InvalidUri(uri.to_string()))?;
		if let Some(buffer_id) = self.state.core.editor.buffers.find_by_path(&path) {
			return Ok((buffer_id, false));
		}

		let buffer_id = self.open_file(path.clone()).await.map_err(|_| ApplyError::BufferNotFound(uri.to_string()))?;
		Ok((buffer_id, true))
	}

	/// Prepares undo grouping for a workspace edit affecting multiple buffers.
	///
	/// Collects view snapshots and creates a single [`EditorUndoGroup`] for all
	/// affected documents. Document-level undo is recorded by each buffer's
	/// `apply()` call in [`apply_buffer_edit_plan`].
	fn begin_workspace_edit_group(&mut self, plan: &WorkspaceEditPlan) {
		let mut seen_docs = HashSet::new();
		let mut affected_docs = Vec::new();
		let mut all_view_snapshots = HashMap::new();

		for buffer_plan in &plan.per_buffer {
			let Some(buffer) = self.state.core.editor.buffers.get_buffer(buffer_plan.buffer_id) else {
				continue;
			};
			let doc_id = buffer.document_id();

			if !seen_docs.insert(doc_id) {
				continue;
			}
			affected_docs.push(doc_id);

			let snapshots: HashMap<_, _> = self
				.state
				.core
				.buffers
				.buffers()
				.filter(|b| b.document_id() == doc_id)
				.map(|b| (b.id, b.snapshot_view()))
				.collect();
			all_view_snapshots.extend(snapshots);
		}

		self.state.core.editor.undo_manager.push_group(EditorUndoGroup {
			affected_docs,
			view_snapshots: all_view_snapshots,
			origin: EditOrigin::Lsp,
		});
	}

	/// Executes the edit plan for a specific buffer.
	pub(crate) fn apply_buffer_edit_plan(&mut self, plan: &BufferEditPlan) -> Result<Transaction, ApplyError> {
		let buffer_id = plan.buffer_id;
		let doc_id = {
			let buffer = self
				.state
				.core
				.buffers
				.get_buffer(buffer_id)
				.ok_or_else(|| ApplyError::BufferNotFound(buffer_id.0.to_string()))?;
			buffer.document_id()
		};

		let changes: Vec<Change> = plan
			.edits
			.iter()
			.map(|edit| Change {
				start: edit.range.start,
				end: edit.range.end,
				replacement: if edit.replacement.is_empty() { None } else { Some(edit.replacement.clone()) },
			})
			.collect();

		let tx = {
			let buffer = self
				.state
				.core
				.buffers
				.get_buffer(buffer_id)
				.ok_or_else(|| ApplyError::BufferNotFound(buffer_id.0.to_string()))?;
			buffer.with_doc(|doc| Transaction::change(doc.content().slice(..), changes))
		};

		let before_rope = {
			let buffer = self
				.state
				.core
				.buffers
				.get_buffer(buffer_id)
				.ok_or_else(|| ApplyError::BufferNotFound(buffer_id.0.to_string()))?;
			buffer.with_doc(|doc| doc.content().clone())
		};

		let result = {
			let buffer = self
				.state
				.core
				.buffers
				.get_buffer_mut(buffer_id)
				.ok_or_else(|| ApplyError::BufferNotFound(buffer_id.0.to_string()))?;
			let policy = ApplyPolicy {
				undo: UndoPolicy::Record,
				syntax: SyntaxPolicy::IncrementalOrDirty,
			};
			buffer.apply(&tx, policy)
		};

		if result.applied {
			let (after_rope, version) = self
				.state
				.core
				.buffers
				.get_buffer(buffer_id)
				.ok_or_else(|| ApplyError::BufferNotFound(buffer_id.0.to_string()))?
				.with_doc(|doc| (doc.content().clone(), doc.version()));
			self.state.integration.syntax_manager.note_edit_incremental(
				doc_id,
				version,
				&before_rope,
				&after_rope,
				tx.changes(),
				&self.state.config.config.language_loader,
				xeno_syntax::EditSource::History,
			);
			self.state.integration.lsp.sync_manager_mut().escalate_full(doc_id);
		}

		if !result.applied {
			return Err(ApplyError::ReadOnly(buffer_id.0.to_string()));
		}

		for buffer in self.state.core.editor.buffers.buffers_mut() {
			if buffer.document_id() == doc_id {
				buffer.map_selection_through(&tx);
			}
		}

		self.state.core.frame.dirty_buffers.insert(buffer_id);
		Ok(tx)
	}

	/// Two-phase atomic persistence for temporary workspace edit buffers.
	///
	/// Phase 1: collect content from all modified temp buffers and write
	/// each to disk using [`crate::io::write_atomic`] (temp-file + rename,
	/// crash-safe). If any write fails, none of the buffers are closed so
	/// the user can recover via the still-open buffer.
	///
	/// Phase 2 (only on full success): close all temp buffers.
	async fn save_temp_buffers_atomic(&mut self, temps: &[ViewId]) -> Result<(), ApplyError> {
		use std::collections::BTreeMap;
		use std::path::PathBuf;

		// Phase 1: collect save plans, canonicalize paths, and deduplicate.
		// BTreeMap gives deterministic (lexicographic) write order.
		let mut plans: BTreeMap<PathBuf, Vec<u8>> = BTreeMap::new();
		for &id in temps {
			let Some(buffer) = self.state.core.editor.buffers.get_buffer(id) else {
				continue;
			};
			if !buffer.modified() {
				continue;
			}
			let Some(raw_path) = buffer.path().map(|p| p.to_path_buf()) else {
				continue;
			};
			let canonical = std::fs::canonicalize(&raw_path).unwrap_or(raw_path);
			let bytes = crate::io::serialize_buffer(buffer);
			if let Some(existing) = plans.get(&canonical) {
				if existing != &bytes {
					return Err(ApplyError::ConflictingTempSave {
						path: canonical.display().to_string(),
					});
				}
				// Identical bytes — deduplicate (skip).
				continue;
			}
			plans.insert(canonical, bytes);
		}

		// Write all plans in deterministic order.
		for (path, bytes) in &plans {
			let write_path = path.clone();
			let write_bytes = bytes.clone();
			let result = self
				.state
				.async_state
				.worker_runtime
				.spawn_blocking(xeno_worker::TaskClass::IoBlocking, move || crate::io::write_atomic(&write_path, &write_bytes))
				.await;
			let write_result = match result {
				Ok(r) => r,
				Err(e) => Err(std::io::Error::other(e.to_string())),
			};
			if let Err(e) = write_result {
				tracing::error!(path = %path.display(), error = %e, "Failed to atomically save workspace edit to disk");
				return Err(ApplyError::IoWriteFailed {
					path: path.display().to_string(),
					error: e.to_string(),
				});
			}
		}

		// Phase 2: all saves succeeded — close all temp buffers.
		for &id in temps {
			self.close_headless_buffer(id).await;
		}
		Ok(())
	}

	/// Closes a buffer and its LSP identity inline.
	///
	/// Used for temp buffers opened during workspace edit planning and for
	/// resource op cleanup. LSP close is awaited inline to prevent
	/// out-of-order didClose/didOpen interleaving with subsequent operations.
	async fn close_headless_buffer(&mut self, buffer_id: ViewId) {
		let Some(buffer) = self.state.core.editor.buffers.get_buffer(buffer_id) else {
			return;
		};
		if let (Some(path), Some(lang)) = (buffer.path().map(|p| p.to_path_buf()), buffer.file_type().map(|s| s.to_string())) {
			if let Err(e) = self.state.integration.lsp.sync().close_document(&path, &lang).await {
				tracing::warn!(error = %e, "LSP buffer close failed");
			}
		}

		self.finalize_buffer_removal(buffer_id);
	}

	/// Applies a workspace edit containing resource operations sequentially.
	///
	/// Each operation in `documentChanges` is processed in order. On failure,
	/// best-effort rollback is attempted for resource operations already applied
	/// in this edit. Returns the index of the first failed operation.
	async fn apply_workspace_edit_operations(&mut self, edit: WorkspaceEdit) -> Result<(), ApplyEditFailure> {
		let ops = match edit.document_changes {
			Some(DocumentChanges::Operations(ops)) => ops,
			_ => return Ok(()),
		};

		let mut rollback_log: Vec<ResourceRollbackEntry> = Vec::new();
		let mut temp_buffers: Vec<ViewId> = Vec::new();

		for (idx, op) in ops.into_iter().enumerate() {
			let result = match op {
				DocumentChangeOperation::Op(resource_op) => self.apply_resource_op(resource_op, &mut rollback_log).await,
				DocumentChangeOperation::Edit(text_edit) => self.apply_single_text_document_edit(text_edit, &mut temp_buffers).await,
			};

			if let Err(error) = result {
				// Rollback resource ops applied so far in reverse order.
				self.rollback_resource_ops(&mut rollback_log).await;
				for id in temp_buffers {
					self.close_headless_buffer(id).await;
				}
				return Err(ApplyEditFailure {
					error,
					failed_change: Some(idx as u32),
				});
			}
		}

		// All operations succeeded; save temp buffers.
		if let Err(error) = self.save_temp_buffers_atomic(&temp_buffers).await {
			return Err(ApplyEditFailure { error, failed_change: None });
		}
		Ok(())
	}

	/// Applies a single resource operation (create/rename/delete).
	async fn apply_resource_op(&mut self, op: ResourceOp, rollback_log: &mut Vec<ResourceRollbackEntry>) -> Result<(), ApplyError> {
		match op {
			ResourceOp::Create(create) => self.apply_resource_create(create, rollback_log).await,
			ResourceOp::Rename(rename) => self.apply_resource_rename(rename, rollback_log).await,
			ResourceOp::Delete(delete) => self.apply_resource_delete(delete, rollback_log).await,
		}
	}

	/// Creates a file on disk. Respects `overwrite` and `ignoreIfExists` options.
	async fn apply_resource_create(&mut self, create: CreateFile, rollback_log: &mut Vec<ResourceRollbackEntry>) -> Result<(), ApplyError> {
		let path = xeno_lsp::path_from_uri(&create.uri).ok_or_else(|| ApplyError::InvalidUri(create.uri.to_string()))?;

		let overwrite = create.options.as_ref().is_some_and(|o| o.overwrite == Some(true));
		let ignore_if_exists = create.options.as_ref().is_some_and(|o| o.ignore_if_exists == Some(true));

		if path.exists() {
			if ignore_if_exists {
				return Ok(());
			}
			if !overwrite {
				// Check if the file is open and modified — reject to prevent data loss.
				if let Some(buf_id) = self.state.core.editor.buffers.find_by_path(&path) {
					let buffer = self.state.core.editor.buffers.get_buffer(buf_id);
					if buffer.is_some_and(|b| b.modified()) {
						return Err(ApplyError::CreateFailed {
							uri: create.uri.to_string(),
							reason: "file exists and buffer is modified".to_string(),
						});
					}
				}
			}
		}

		// Snapshot existing content for rollback.
		let had_previous = path.exists();
		let previous_bytes = if had_previous { std::fs::read(&path).ok() } else { None };

		// Create parent directories if needed.
		if let Some(parent) = path.parent()
			&& !parent.exists()
		{
			std::fs::create_dir_all(parent).map_err(|e| ApplyError::CreateFailed {
				uri: create.uri.to_string(),
				reason: e.to_string(),
			})?;
		}

		// Create empty file (or truncate if overwriting).
		std::fs::write(&path, b"").map_err(|e| ApplyError::CreateFailed {
			uri: create.uri.to_string(),
			reason: e.to_string(),
		})?;

		rollback_log.push(ResourceRollbackEntry::Created {
			path: path.clone(),
			had_previous,
			previous_bytes,
		});

		Ok(())
	}

	/// Renames a file on disk. Updates open buffers if the source is open.
	async fn apply_resource_rename(&mut self, rename: RenameFile, rollback_log: &mut Vec<ResourceRollbackEntry>) -> Result<(), ApplyError> {
		let old_path = xeno_lsp::path_from_uri(&rename.old_uri).ok_or_else(|| ApplyError::InvalidUri(rename.old_uri.to_string()))?;
		let new_path = xeno_lsp::path_from_uri(&rename.new_uri).ok_or_else(|| ApplyError::InvalidUri(rename.new_uri.to_string()))?;

		let overwrite = rename.options.as_ref().is_some_and(|o| o.overwrite == Some(true));
		let ignore_if_exists = rename.options.as_ref().is_some_and(|o| o.ignore_if_exists == Some(true));

		// Block if old file is open + modified.
		if let Some(buf_id) = self.state.core.editor.buffers.find_by_path(&old_path) {
			let buffer = self.state.core.editor.buffers.get_buffer(buf_id);
			if buffer.is_some_and(|b| b.modified()) {
				return Err(ApplyError::RenameBlockedModified {
					uri: rename.old_uri.to_string(),
				});
			}
		}

		// Check target.
		if new_path.exists() {
			if ignore_if_exists {
				return Ok(());
			}
			if !overwrite {
				return Err(ApplyError::RenameFailed {
					old_uri: rename.old_uri.to_string(),
					new_uri: rename.new_uri.to_string(),
					reason: "target exists".to_string(),
				});
			}
		}

		// Create parent directories for target.
		if let Some(parent) = new_path.parent()
			&& !parent.exists()
		{
			std::fs::create_dir_all(parent).map_err(|e| ApplyError::RenameFailed {
				old_uri: rename.old_uri.to_string(),
				new_uri: rename.new_uri.to_string(),
				reason: e.to_string(),
			})?;
		}

		std::fs::rename(&old_path, &new_path).map_err(|e| ApplyError::RenameFailed {
			old_uri: rename.old_uri.to_string(),
			new_uri: rename.new_uri.to_string(),
			reason: e.to_string(),
		})?;

		rollback_log.push(ResourceRollbackEntry::Renamed {
			from: old_path.clone(),
			to: new_path.clone(),
		});

		// Update open buffer path and notify LSP of the identity change.
		if let Some(buf_id) = self.state.core.editor.buffers.find_by_path(&old_path) {
			// Capture old language and buffer text before changing the path.
			let old_lang = self
				.state
				.core
				.editor
				.buffers
				.get_buffer(buf_id)
				.and_then(|b| b.file_type().map(|s| s.to_string()));
			let text = self
				.state
				.core
				.editor
				.buffers
				.get_buffer(buf_id)
				.map(|b| b.with_doc(|doc| doc.content().to_string()));

			if let Some(buffer) = self.state.core.editor.buffers.get_buffer_mut(buf_id) {
				buffer.set_path(Some(new_path.clone()), Some(&self.state.config.config.language_loader));
			}

			// Get new language after path update (may differ, e.g. .js → .ts).
			let new_lang = self
				.state
				.core
				.editor
				.buffers
				.get_buffer(buf_id)
				.and_then(|b| b.file_type().map(|s| s.to_string()));

			let sync = self.state.integration.lsp.sync();
			match (old_lang, new_lang, text) {
				(Some(old_lang), Some(new_lang), Some(text)) => {
					if let Err(e) = sync.reopen_document(&old_path, &old_lang, &new_path, &new_lang, text).await {
						tracing::warn!(error = %e, "LSP reopen after rename failed");
					}
				}
				(Some(old_lang), None, _) => {
					if let Err(e) = sync.close_document(&old_path, &old_lang).await {
						tracing::warn!(error = %e, "LSP close after rename (lang removed) failed");
					}
				}
				(None, Some(new_lang), Some(text)) => {
					if let Err(e) = sync.ensure_open_text(&new_path, &new_lang, text).await {
						tracing::warn!(error = %e, "LSP open after rename (lang added) failed");
					}
				}
				(Some(old_lang), _, None) => {
					tracing::warn!("buffer text unavailable during rename; closing stale LSP identity");
					if let Err(e) = sync.close_document(&old_path, &old_lang).await {
						tracing::warn!(error = %e, "LSP close (text unavailable) failed");
					}
				}
				_ => {}
			}

			// Update sync manager's tracked config to reflect the new path/language.
			// Without this, didChange notifications would reference the old URI.
			self.maybe_track_lsp_for_buffer(buf_id, true);
		}

		Ok(())
	}

	/// Deletes a file from disk. Closes open buffers for the deleted file.
	async fn apply_resource_delete(&mut self, delete: DeleteFile, rollback_log: &mut Vec<ResourceRollbackEntry>) -> Result<(), ApplyError> {
		let path = xeno_lsp::path_from_uri(&delete.uri).ok_or_else(|| ApplyError::InvalidUri(delete.uri.to_string()))?;

		let ignore_if_not_exists = delete.options.as_ref().is_some_and(|o| o.ignore_if_not_exists == Some(true));

		if !path.exists() {
			if ignore_if_not_exists {
				return Ok(());
			}
			return Err(ApplyError::DeleteFailed {
				uri: delete.uri.to_string(),
				reason: "file does not exist".to_string(),
			});
		}

		// Block if open + modified.
		if let Some(buf_id) = self.state.core.editor.buffers.find_by_path(&path) {
			let buffer = self.state.core.editor.buffers.get_buffer(buf_id);
			if buffer.is_some_and(|b| b.modified()) {
				return Err(ApplyError::DeleteBlockedModified { uri: delete.uri.to_string() });
			}
		}

		// Snapshot for rollback.
		let bytes = std::fs::read(&path).ok();

		if path.is_dir() {
			let recursive = delete.options.as_ref().is_some_and(|o| o.recursive == Some(true));
			if recursive {
				std::fs::remove_dir_all(&path)
			} else {
				std::fs::remove_dir(&path)
			}
		} else {
			std::fs::remove_file(&path)
		}
		.map_err(|e| ApplyError::DeleteFailed {
			uri: delete.uri.to_string(),
			reason: e.to_string(),
		})?;

		rollback_log.push(ResourceRollbackEntry::Deleted { path: path.clone(), bytes });

		// Close open buffer for deleted file.
		if let Some(buf_id) = self.state.core.editor.buffers.find_by_path(&path) {
			self.close_headless_buffer(buf_id).await;
		}

		Ok(())
	}

	/// Applies a single `TextDocumentEdit` as part of a sequential operations flow.
	///
	/// This opens the document if needed (tracking temp buffers), plans the edit,
	/// and applies it immediately.
	async fn apply_single_text_document_edit(&mut self, edit: TextDocumentEdit, temp_buffers: &mut Vec<ViewId>) -> Result<(), ApplyError> {
		let uri = edit.text_document.uri.clone();

		// Version check.
		if let Some(expected) = edit.text_document.version {
			match self.state.integration.lsp.documents().get_version(&uri) {
				Some(actual) if actual != expected => {
					return Err(ApplyError::VersionMismatch {
						uri: uri.to_string(),
						expected,
						actual,
					});
				}
				None => {
					return Err(ApplyError::UntrackedVersionedDocument {
						uri: uri.to_string(),
						version: expected,
					});
				}
				_ => {}
			}
		}

		let text_edits = normalize_text_document_edits(edit.edits);
		if text_edits.is_empty() {
			return Ok(());
		}

		let (buffer_id, opened_temporarily) = self.resolve_uri_to_buffer(&uri).await?;
		if opened_temporarily {
			temp_buffers.push(buffer_id);
		}

		let buffer = self
			.state
			.core
			.buffers
			.get_buffer(buffer_id)
			.ok_or_else(|| ApplyError::BufferNotFound(uri.to_string()))?;
		let encoding = self.state.integration.lsp.offset_encoding_for_buffer(buffer);
		let mut planned_edits: Vec<PlannedTextEdit> = Vec::new();
		for te in &text_edits {
			let planned = buffer
				.with_doc(|doc| convert_text_edit(doc.content(), encoding, te))
				.ok_or_else(|| ApplyError::RangeConversionFailed(uri.to_string()))?;
			planned_edits.push(planned);
		}
		coalesce_and_validate(&mut planned_edits, &uri)?;

		let plan = BufferEditPlan {
			buffer_id,
			edits: planned_edits,
			opened_temporarily,
		};

		// Apply immediately.
		let workspace_plan = WorkspaceEditPlan { per_buffer: vec![plan] };
		self.begin_workspace_edit_group(&workspace_plan);
		self.apply_buffer_edit_plan(&workspace_plan.per_buffer[0])?;
		self.flush_lsp_sync_now(&[buffer_id]);

		Ok(())
	}

	/// Best-effort rollback of resource operations applied in this edit.
	async fn rollback_resource_ops(&mut self, log: &mut Vec<ResourceRollbackEntry>) {
		while let Some(entry) = log.pop() {
			match entry {
				ResourceRollbackEntry::Created {
					path,
					had_previous,
					previous_bytes,
				} => {
					if had_previous {
						if let Some(bytes) = previous_bytes {
							let _ = std::fs::write(&path, &bytes);
						}
					} else {
						let _ = std::fs::remove_file(&path);
					}
				}
				ResourceRollbackEntry::Renamed { from, to } => {
					let _ = std::fs::rename(&to, &from);
					// Restore buffer path and LSP identity.
					if let Some(buf_id) = self.state.core.editor.buffers.find_by_path(&to) {
						let old_lang = self
							.state
							.core
							.editor
							.buffers
							.get_buffer(buf_id)
							.and_then(|b| b.file_type().map(|s| s.to_string()));
						let text = self
							.state
							.core
							.editor
							.buffers
							.get_buffer(buf_id)
							.map(|b| b.with_doc(|doc| doc.content().to_string()));

						if let Some(buffer) = self.state.core.editor.buffers.get_buffer_mut(buf_id) {
							buffer.set_path(Some(from.clone()), Some(&self.state.config.config.language_loader));
						}

						let new_lang = self
							.state
							.core
							.editor
							.buffers
							.get_buffer(buf_id)
							.and_then(|b| b.file_type().map(|s| s.to_string()));

						let sync = self.state.integration.lsp.sync();
						match (old_lang, new_lang, text) {
							(Some(old_lang), Some(new_lang), Some(text)) => {
								if let Err(e) = sync.reopen_document(&to, &old_lang, &from, &new_lang, text).await {
									tracing::warn!(error = %e, "LSP reopen after rollback rename failed");
								}
							}
							(Some(old_lang), None, _) => {
								if let Err(e) = sync.close_document(&to, &old_lang).await {
									tracing::warn!(error = %e, "LSP close after rollback rename (lang removed) failed");
								}
							}
							(None, Some(new_lang), Some(text)) => {
								if let Err(e) = sync.ensure_open_text(&from, &new_lang, text).await {
									tracing::warn!(error = %e, "LSP open after rollback rename (lang added) failed");
								}
							}
							(Some(old_lang), _, None) => {
								tracing::warn!("buffer text unavailable during rollback rename; closing stale LSP identity");
								if let Err(e) = sync.close_document(&to, &old_lang).await {
									tracing::warn!(error = %e, "LSP close (text unavailable, rollback) failed");
								}
							}
							_ => {}
						}

						// Update sync manager's tracked config to reflect the restored path.
						self.maybe_track_lsp_for_buffer(buf_id, true);
					}
				}
				ResourceRollbackEntry::Deleted { path, bytes } => {
					if let Some(bytes) = bytes {
						let _ = std::fs::write(&path, &bytes);
					}
				}
			}
		}
	}
}

/// Rollback log entry for a resource operation.
enum ResourceRollbackEntry {
	Created {
		path: PathBuf,
		had_previous: bool,
		previous_bytes: Option<Vec<u8>>,
	},
	Renamed {
		from: PathBuf,
		to: PathBuf,
	},
	Deleted {
		path: PathBuf,
		bytes: Option<Vec<u8>>,
	},
}

/// Converts an LSP [`TextEdit`] into a character-offset based [`PlannedTextEdit`].
///
/// Returns `None` if position conversion fails (OOB line) or the resulting
/// character range is reversed (`start > end`), which indicates the server
/// sent an invalid range.
pub(crate) fn convert_text_edit(rope: &xeno_primitives::Rope, encoding: OffsetEncoding, edit: &TextEdit) -> Option<PlannedTextEdit> {
	let (start, end) = lsp_range_to_char_range(rope, edit.range, encoding)?;
	if start > end {
		return None;
	}
	Some(PlannedTextEdit {
		range: start..end,
		replacement: edit.new_text.clone(),
	})
}

/// Sorts, coalesces adjacent, and validates that edits do not overlap.
///
/// # Errors
///
/// Returns [`ApplyError::OverlappingEdits`] if any edits target intersecting regions.
pub(crate) fn coalesce_and_validate(edits: &mut Vec<PlannedTextEdit>, uri: &Uri) -> Result<(), ApplyError> {
	edits.sort_by_key(|edit| (edit.range.start, edit.range.end));
	let mut out: Vec<PlannedTextEdit> = Vec::with_capacity(edits.len());
	for edit in edits.drain(..) {
		if let Some(last) = out.last_mut() {
			if edit.range.start < last.range.end {
				return Err(ApplyError::OverlappingEdits(uri.to_string()));
			}
			if edit.range.start == last.range.end {
				last.range.end = edit.range.end;
				last.replacement.push_str(&edit.replacement);
				continue;
			}
		}
		out.push(edit);
	}
	*edits = out;
	Ok(())
}

fn normalize_text_document_edits(edits: Vec<OneOf<TextEdit, AnnotatedTextEdit>>) -> Vec<TextEdit> {
	edits
		.into_iter()
		.map(|edit| match edit {
			OneOf::Left(edit) => edit,
			OneOf::Right(AnnotatedTextEdit { text_edit, .. }) => text_edit,
		})
		.collect()
}

#[cfg(test)]
mod tests;
