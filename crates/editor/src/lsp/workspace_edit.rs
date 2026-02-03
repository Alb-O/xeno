//! LSP workspace edit planning and application.

use std::collections::{HashMap, HashSet};
use std::ops::Range;

use thiserror::Error;
use xeno_lsp::lsp_types::{
	AnnotatedTextEdit, DocumentChangeOperation, DocumentChanges, OneOf, TextDocumentEdit, TextEdit,
	Uri, WorkspaceEdit,
};
use xeno_lsp::{OffsetEncoding, lsp_range_to_char_range};
use xeno_primitives::range::CharIdx;
use xeno_primitives::transaction::{Change, Tendril};
use xeno_primitives::{EditOrigin, SyntaxPolicy, Transaction, UndoPolicy};

use crate::buffer::{ApplyPolicy, ViewId};
use crate::impls::{Editor, EditorUndoGroup};

/// A validated, ready-to-apply workspace edit plan.
pub struct WorkspaceEditPlan {
	pub per_buffer: Vec<BufferEditPlan>,
}

impl WorkspaceEditPlan {
	pub fn affected_buffer_ids(&self) -> Vec<ViewId> {
		self.per_buffer.iter().map(|p| p.buffer_id).collect()
	}
}

pub struct BufferEditPlan {
	pub buffer_id: ViewId,
	pub edits: Vec<PlannedTextEdit>,
	pub opened_temporarily: bool,
}

pub struct PlannedTextEdit {
	pub range: Range<CharIdx>,
	pub replacement: Tendril,
}

#[derive(Debug, Error)]
pub enum ApplyError {
	#[error("invalid uri: {0}")]
	InvalidUri(String),
	#[error("unsupported workspace edit operation")]
	UnsupportedOperation,
	#[error("buffer not found for uri: {0}")]
	BufferNotFound(String),
	#[error("failed to convert text edit range for {0}")]
	RangeConversionFailed(String),
	#[error("overlapping edits for {0}")]
	OverlappingEdits(String),
	#[error("read-only buffer for {0}")]
	ReadOnly(String),
}

impl Editor {
	pub async fn apply_workspace_edit(&mut self, edit: WorkspaceEdit) -> Result<(), ApplyError> {
		let plan = self.plan_workspace_edit(edit).await?;
		if plan.per_buffer.is_empty() {
			return Ok(());
		}

		self.begin_workspace_edit_group(&plan);

		for buffer_plan in &plan.per_buffer {
			let _ = self.apply_buffer_edit_plan(buffer_plan)?;
		}

		self.flush_lsp_sync_now(&plan.affected_buffer_ids());
		self.close_temporary_buffers(&plan);
		Ok(())
	}

	async fn plan_workspace_edit(
		&mut self,
		edit: WorkspaceEdit,
	) -> Result<WorkspaceEditPlan, ApplyError> {
		// Use String keys to avoid clippy::mutable_key_type warning for Uri
		let mut per_uri: HashMap<String, (Uri, Vec<TextEdit>)> = HashMap::new();
		if let Some(changes) = edit.changes {
			for (uri, edits) in changes {
				let key = uri.to_string();
				per_uri
					.entry(key)
					.or_insert_with(|| (uri, Vec::new()))
					.1
					.extend(edits);
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
			let buffer = self
				.state
				.core
				.buffers
				.get_buffer(buffer_id)
				.ok_or_else(|| ApplyError::BufferNotFound(uri.to_string()))?;
			let encoding = self.state.lsp.offset_encoding_for_buffer(buffer);
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

	fn collect_text_document_edit(
		&self,
		edit: TextDocumentEdit,
		per_uri: &mut HashMap<String, (Uri, Vec<TextEdit>)>,
	) -> Result<(), ApplyError> {
		let uri = edit.text_document.uri;
		let key = uri.to_string();
		let edits = normalize_text_document_edits(edit.edits);
		per_uri
			.entry(key)
			.or_insert_with(|| (uri, Vec::new()))
			.1
			.extend(edits);
		Ok(())
	}

	async fn resolve_uri_to_buffer(&mut self, uri: &Uri) -> Result<(ViewId, bool), ApplyError> {
		let path =
			xeno_lsp::path_from_uri(uri).ok_or_else(|| ApplyError::InvalidUri(uri.to_string()))?;
		if let Some(buffer_id) = self.state.core.buffers.find_by_path(&path) {
			self.finalize_buffer_removal(buffer_id);
		}

		let buffer_id = self
			.open_file(path.clone())
			.await
			.map_err(|_| ApplyError::BufferNotFound(uri.to_string()))?;
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
		let mut all_view_snapshots = std::collections::HashMap::new();

		for buffer_plan in &plan.per_buffer {
			let Some(buffer) = self.state.core.buffers.get_buffer(buffer_plan.buffer_id) else {
				continue;
			};
			let doc_id = buffer.document_id();

			if !seen_docs.insert(doc_id) {
				continue;
			}
			affected_docs.push(doc_id);

			let snapshots: std::collections::HashMap<_, _> = self
				.state
				.core
				.buffers
				.buffers()
				.filter(|b| b.document_id() == doc_id)
				.map(|b| (b.id, b.snapshot_view()))
				.collect();
			all_view_snapshots.extend(snapshots);
		}

		self.state.core.undo_manager.push_group(EditorUndoGroup {
			affected_docs,
			view_snapshots: all_view_snapshots,
			origin: EditOrigin::Lsp,
		});
	}

	pub(crate) fn apply_buffer_edit_plan(
		&mut self,
		plan: &BufferEditPlan,
	) -> Result<Transaction, ApplyError> {
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
				replacement: if edit.replacement.is_empty() {
					None
				} else {
					Some(edit.replacement.clone())
				},
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
			buffer.apply(&tx, policy, &self.state.config.language_loader)
		};

		if result.applied {
			self.state.syntax_manager.note_edit(doc_id);
			self.state.lsp.sync_manager_mut().escalate_full(doc_id);

			if let Some(uri) = self
				.state
				.shared_state
				.uri_for_doc_id(doc_id)
				.map(str::to_string)
				&& let Some(payload) = self.state.shared_state.prepare_edit(&uri, &tx)
			{
				let _ = self.state.lsp.shared_state_out_tx().send(payload);
			}
		}

		if !result.applied {
			return Err(ApplyError::ReadOnly(buffer_id.0.to_string()));
		}

		for buffer in self.state.core.buffers.buffers_mut() {
			if buffer.document_id() == doc_id {
				buffer.map_selection_through(&tx);
			}
		}

		self.state.frame.dirty_buffers.insert(buffer_id);
		Ok(tx)
	}

	fn close_temporary_buffers(&mut self, plan: &WorkspaceEditPlan) {
		let buffer_ids: Vec<_> = plan
			.per_buffer
			.iter()
			.filter(|p| p.opened_temporarily)
			.map(|p| p.buffer_id)
			.collect();
		for buffer_id in buffer_ids {
			self.close_headless_buffer(buffer_id);
		}
	}

	fn close_headless_buffer(&mut self, buffer_id: ViewId) {
		let Some(buffer) = self.state.core.buffers.get_buffer(buffer_id) else {
			return;
		};
		if let (Some(path), Some(lang)) = (
			buffer.path().map(|p| p.to_path_buf()),
			buffer.file_type().map(|s| s.to_string()),
		) {
			let lsp_handle = self.state.lsp.handle();
			tokio::spawn(async move {
				if let Err(e) = lsp_handle.close_document(path, lang).await {
					tracing::warn!(error = %e, "LSP buffer close failed");
				}
			});
		}

		self.finalize_buffer_removal(buffer_id);
	}
}

pub(crate) fn convert_text_edit(
	rope: &xeno_primitives::Rope,
	encoding: OffsetEncoding,
	edit: &TextEdit,
) -> Option<PlannedTextEdit> {
	let (start, end) = lsp_range_to_char_range(rope, edit.range, encoding)?;
	Some(PlannedTextEdit {
		range: start..end,
		replacement: edit.new_text.clone(),
	})
}

pub(crate) fn coalesce_and_validate(
	edits: &mut Vec<PlannedTextEdit>,
	uri: &Uri,
) -> Result<(), ApplyError> {
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
mod tests {
	use xeno_lsp::lsp_types;

	use super::*;

	#[test]
	fn workspace_edit_plan_manual_construct() {
		let plan = WorkspaceEditPlan {
			per_buffer: Vec::new(),
		};
		assert!(plan.affected_buffer_ids().is_empty());
	}

	#[test]
	fn coalesce_rejects_overlap() {
		let mut edits = vec![
			PlannedTextEdit {
				range: 0..2,
				replacement: "a".into(),
			},
			PlannedTextEdit {
				range: 1..3,
				replacement: "b".into(),
			},
		];
		let uri: Uri = "file:///tmp/test.rs".parse().unwrap();
		let err = coalesce_and_validate(&mut edits, &uri).unwrap_err();
		assert!(matches!(err, ApplyError::OverlappingEdits(_)));
	}

	#[test]
	fn convert_text_edit_utf16() {
		let rope = xeno_primitives::Rope::from("a😀b\n");
		let edit = TextEdit {
			range: lsp_types::Range {
				start: lsp_types::Position {
					line: 0,
					character: 1,
				},
				end: lsp_types::Position {
					line: 0,
					character: 3,
				},
			},
			new_text: "X".into(),
		};
		let planned = convert_text_edit(&rope, OffsetEncoding::Utf16, &edit).unwrap();
		assert_eq!(planned.range.start, 1);
		assert_eq!(planned.range.end, 2);
	}
}
