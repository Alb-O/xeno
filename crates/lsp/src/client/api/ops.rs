use lsp_types::notification::Notification;
use lsp_types::{ClientInfo, InitializeParams, InitializeResult, Uri};
use serde_json::Value;
use tokio::sync::oneshot;

use super::super::capabilities::client_capabilities;
use super::super::handle::ClientHandle;
use super::types::workspace_folder_from_uri;
use crate::{AnyNotification, Result, uri_from_path};

impl ClientHandle {
	/// Initialize the language server.
	pub async fn initialize(&self, enable_snippets: bool, config: Option<Value>) -> Result<InitializeResult> {
		#[allow(deprecated, reason = "root_path field deprecated but required by some servers")]
		let params = InitializeParams {
			process_id: Some(std::process::id()),
			workspace_folders: Some(vec![workspace_folder_from_uri(
				self.root_uri.clone().unwrap_or_else(|| uri_from_path(&self.root_path).expect("valid path")),
			)]),
			root_path: self.root_path.to_str().map(String::from),
			root_uri: self.root_uri.clone(),
			initialization_options: config,
			capabilities: client_capabilities(enable_snippets),
			trace: None,
			client_info: Some(ClientInfo {
				name: String::from("xeno"),
				version: Some(String::from(env!("CARGO_PKG_VERSION"))),
			}),
			locale: None,
			work_done_progress_params: Default::default(),
		};

		let result = self.request::<lsp_types::request::Initialize>(params).await?;

		let _ = self.capabilities.set(result.capabilities.clone());
		self.initialize_notify.notify_waiters();

		let barrier_rx = self
			.transport
			.notify_with_barrier(
				self.id,
				AnyNotification::new(
					lsp_types::notification::Initialized::METHOD,
					serde_json::to_value(lsp_types::InitializedParams {}).expect("serialize"),
				),
			)
			.await?;

		// Await the barrier to ensure the 'initialized' notification is written
		// before we start sending requests.
		match barrier_rx.await {
			Ok(Ok(())) => {
				self.set_ready(true);
			}
			Ok(Err(e)) => {
				return Err(e);
			}
			Err(_) => {
				return Err(crate::Error::Protocol("barrier sender dropped".into()));
			}
		}

		Ok(result)
	}

	/// Shutdown the language server gracefully.
	pub async fn shutdown(&self) -> Result<()> {
		self.request::<lsp_types::request::Shutdown>(()).await
	}

	/// Send exit notification to the server.
	pub async fn exit(&self) -> Result<()> {
		self.notify::<lsp_types::notification::Exit>(()).await
	}

	/// Shutdown and exit the language server.
	pub async fn shutdown_and_exit(&self) -> Result<()> {
		self.shutdown().await?;
		self.exit().await
	}

	/// Notify the server that a document was opened.
	pub async fn text_document_did_open(&self, uri: Uri, language_id: String, version: i32, text: String) -> Result<()> {
		self.notify::<lsp_types::notification::DidOpenTextDocument>(lsp_types::DidOpenTextDocumentParams {
			text_document: lsp_types::TextDocumentItem {
				uri,
				language_id,
				version,
				text,
			},
		})
		.await
	}

	/// Notify the server that a document was changed (full sync).
	pub async fn text_document_did_change_full(&self, uri: Uri, version: i32, text: String) -> Result<()> {
		let notification = AnyNotification::new(
			lsp_types::notification::DidChangeTextDocument::METHOD,
			serde_json::to_value(lsp_types::DidChangeTextDocumentParams {
				text_document: lsp_types::VersionedTextDocumentIdentifier { uri, version },
				content_changes: vec![lsp_types::TextDocumentContentChangeEvent {
					range: None,
					range_length: None,
					text,
				}],
			})
			.expect("Failed to serialize"),
		);
		self.transport.notify(self.id, notification).await
	}

	/// Notify the server that a document was changed (full sync) with a write barrier.
	pub async fn text_document_did_change_full_with_barrier(&self, uri: Uri, version: i32, text: String) -> Result<oneshot::Receiver<Result<()>>> {
		let notification = AnyNotification::new(
			lsp_types::notification::DidChangeTextDocument::METHOD,
			serde_json::to_value(lsp_types::DidChangeTextDocumentParams {
				text_document: lsp_types::VersionedTextDocumentIdentifier { uri, version },
				content_changes: vec![lsp_types::TextDocumentContentChangeEvent {
					range: None,
					range_length: None,
					text,
				}],
			})
			.expect("Failed to serialize"),
		);
		self.transport.notify_with_barrier(self.id, notification).await
	}

	/// Notify the server that a document was changed (incremental sync).
	pub async fn text_document_did_change(&self, uri: Uri, version: i32, changes: Vec<lsp_types::TextDocumentContentChangeEvent>) -> Result<()> {
		let notification = AnyNotification::new(
			lsp_types::notification::DidChangeTextDocument::METHOD,
			serde_json::to_value(lsp_types::DidChangeTextDocumentParams {
				text_document: lsp_types::VersionedTextDocumentIdentifier { uri, version },
				content_changes: changes,
			})
			.expect("Failed to serialize"),
		);
		self.transport.notify(self.id, notification).await
	}

	/// Notify the server that a document was changed (incremental sync) with a write barrier.
	pub async fn text_document_did_change_with_barrier(
		&self,
		uri: Uri,
		version: i32,
		changes: Vec<lsp_types::TextDocumentContentChangeEvent>,
	) -> Result<oneshot::Receiver<Result<()>>> {
		let notification = AnyNotification::new(
			lsp_types::notification::DidChangeTextDocument::METHOD,
			serde_json::to_value(lsp_types::DidChangeTextDocumentParams {
				text_document: lsp_types::VersionedTextDocumentIdentifier { uri, version },
				content_changes: changes,
			})
			.expect("Failed to serialize"),
		);
		self.transport.notify_with_barrier(self.id, notification).await
	}

	/// Notify the server that a document will be saved.
	pub async fn text_document_will_save(&self, uri: Uri, reason: lsp_types::TextDocumentSaveReason) -> Result<()> {
		self.notify::<lsp_types::notification::WillSaveTextDocument>(lsp_types::WillSaveTextDocumentParams {
			text_document: lsp_types::TextDocumentIdentifier { uri },
			reason,
		})
		.await
	}

	/// Notify the server that a document was saved.
	pub async fn text_document_did_save(&self, uri: Uri, text: Option<String>) -> Result<()> {
		self.notify::<lsp_types::notification::DidSaveTextDocument>(lsp_types::DidSaveTextDocumentParams {
			text_document: lsp_types::TextDocumentIdentifier { uri },
			text,
		})
		.await
	}

	/// Notify the server that a document was closed.
	pub async fn text_document_did_close(&self, uri: Uri) -> Result<()> {
		self.notify::<lsp_types::notification::DidCloseTextDocument>(lsp_types::DidCloseTextDocumentParams {
			text_document: lsp_types::TextDocumentIdentifier { uri },
		})
		.await
	}

	/// Notify the server of configuration changes.
	pub async fn did_change_configuration(&self, settings: Value) -> Result<()> {
		self.notify::<lsp_types::notification::DidChangeConfiguration>(lsp_types::DidChangeConfigurationParams { settings })
			.await
	}

	/// Request hover information.
	pub async fn hover(&self, uri: Uri, position: lsp_types::Position) -> Result<Option<lsp_types::Hover>> {
		if !self.supports_hover() {
			return Ok(None);
		}
		self.request::<lsp_types::request::HoverRequest>(lsp_types::HoverParams {
			text_document_position_params: lsp_types::TextDocumentPositionParams {
				text_document: lsp_types::TextDocumentIdentifier { uri },
				position,
			},
			work_done_progress_params: Default::default(),
		})
		.await
	}

	/// Request completions.
	pub async fn completion(
		&self,
		uri: Uri,
		position: lsp_types::Position,
		context: Option<lsp_types::CompletionContext>,
	) -> Result<Option<lsp_types::CompletionResponse>> {
		if !self.supports_completion() {
			return Ok(None);
		}
		self.request::<lsp_types::request::Completion>(lsp_types::CompletionParams {
			text_document_position: lsp_types::TextDocumentPositionParams {
				text_document: lsp_types::TextDocumentIdentifier { uri },
				position,
			},
			work_done_progress_params: Default::default(),
			partial_result_params: Default::default(),
			context,
		})
		.await
	}

	/// Request go to definition.
	pub async fn goto_definition(&self, uri: Uri, position: lsp_types::Position) -> Result<Option<lsp_types::GotoDefinitionResponse>> {
		if !self.supports_definition() {
			return Ok(None);
		}
		self.request::<lsp_types::request::GotoDefinition>(lsp_types::GotoDefinitionParams {
			text_document_position_params: lsp_types::TextDocumentPositionParams {
				text_document: lsp_types::TextDocumentIdentifier { uri },
				position,
			},
			work_done_progress_params: Default::default(),
			partial_result_params: Default::default(),
		})
		.await
	}

	/// Request references.
	pub async fn references(&self, uri: Uri, position: lsp_types::Position, include_declaration: bool) -> Result<Option<Vec<lsp_types::Location>>> {
		if !self.supports_references() {
			return Ok(None);
		}
		self.request::<lsp_types::request::References>(lsp_types::ReferenceParams {
			text_document_position: lsp_types::TextDocumentPositionParams {
				text_document: lsp_types::TextDocumentIdentifier { uri },
				position,
			},
			work_done_progress_params: Default::default(),
			partial_result_params: Default::default(),
			context: lsp_types::ReferenceContext { include_declaration },
		})
		.await
	}

	/// Request document symbols.
	pub async fn document_symbol(&self, uri: Uri) -> Result<Option<lsp_types::DocumentSymbolResponse>> {
		if !self.supports_document_symbol() {
			return Ok(None);
		}
		self.request::<lsp_types::request::DocumentSymbolRequest>(lsp_types::DocumentSymbolParams {
			text_document: lsp_types::TextDocumentIdentifier { uri },
			work_done_progress_params: Default::default(),
			partial_result_params: Default::default(),
		})
		.await
	}

	/// Request formatting.
	pub async fn formatting(&self, uri: Uri, options: lsp_types::FormattingOptions) -> Result<Option<Vec<lsp_types::TextEdit>>> {
		if !self.supports_formatting() {
			return Ok(None);
		}
		self.request::<lsp_types::request::Formatting>(lsp_types::DocumentFormattingParams {
			text_document: lsp_types::TextDocumentIdentifier { uri },
			options,
			work_done_progress_params: Default::default(),
		})
		.await
	}

	/// Request code actions.
	pub async fn code_action(&self, uri: Uri, range: lsp_types::Range, context: lsp_types::CodeActionContext) -> Result<Option<lsp_types::CodeActionResponse>> {
		if !self.supports_code_action() {
			return Ok(None);
		}
		self.request::<lsp_types::request::CodeActionRequest>(lsp_types::CodeActionParams {
			text_document: lsp_types::TextDocumentIdentifier { uri },
			range,
			context,
			work_done_progress_params: Default::default(),
			partial_result_params: Default::default(),
		})
		.await
	}

	/// Resolve additional details for a code action (edit, command, etc.).
	///
	/// Only sends the request if the server advertises `resolve_provider`. Returns
	/// the original action unchanged if resolve is not supported.
	pub async fn code_action_resolve(&self, action: lsp_types::CodeAction) -> Result<lsp_types::CodeAction> {
		if !self.supports_code_action_resolve() {
			return Ok(action);
		}
		self.request::<lsp_types::request::CodeActionResolveRequest>(action).await
	}

	/// Request signature help.
	pub async fn signature_help(&self, uri: Uri, position: lsp_types::Position) -> Result<Option<lsp_types::SignatureHelp>> {
		if !self.supports_signature_help() {
			return Ok(None);
		}
		self.request::<lsp_types::request::SignatureHelpRequest>(lsp_types::SignatureHelpParams {
			text_document_position_params: lsp_types::TextDocumentPositionParams {
				text_document: lsp_types::TextDocumentIdentifier { uri },
				position,
			},
			work_done_progress_params: Default::default(),
			context: None,
		})
		.await
	}

	/// Request prepare rename to validate a rename operation and get the target range/placeholder.
	pub async fn prepare_rename(&self, uri: Uri, position: lsp_types::Position) -> Result<Option<lsp_types::PrepareRenameResponse>> {
		if !self.supports_prepare_rename() {
			return Ok(None);
		}
		self.request::<lsp_types::request::PrepareRenameRequest>(lsp_types::TextDocumentPositionParams {
			text_document: lsp_types::TextDocumentIdentifier { uri },
			position,
		})
		.await
	}

	/// Request rename.
	pub async fn rename(&self, uri: Uri, position: lsp_types::Position, new_name: String) -> Result<Option<lsp_types::WorkspaceEdit>> {
		if !self.supports_rename() {
			return Ok(None);
		}
		self.request::<lsp_types::request::Rename>(lsp_types::RenameParams {
			text_document_position: lsp_types::TextDocumentPositionParams {
				text_document: lsp_types::TextDocumentIdentifier { uri },
				position,
			},
			new_name,
			work_done_progress_params: Default::default(),
		})
		.await
	}

	/// Request go to declaration.
	pub async fn goto_declaration(&self, uri: Uri, position: lsp_types::Position) -> Result<Option<lsp_types::GotoDefinitionResponse>> {
		if !self.supports_declaration() {
			return Ok(None);
		}
		self.request::<lsp_types::request::GotoDeclaration>(lsp_types::GotoDefinitionParams {
			text_document_position_params: lsp_types::TextDocumentPositionParams {
				text_document: lsp_types::TextDocumentIdentifier { uri },
				position,
			},
			work_done_progress_params: Default::default(),
			partial_result_params: Default::default(),
		})
		.await
	}

	/// Request go to implementation.
	pub async fn goto_implementation(&self, uri: Uri, position: lsp_types::Position) -> Result<Option<lsp_types::GotoDefinitionResponse>> {
		if !self.supports_implementation() {
			return Ok(None);
		}
		self.request::<lsp_types::request::GotoImplementation>(lsp_types::GotoDefinitionParams {
			text_document_position_params: lsp_types::TextDocumentPositionParams {
				text_document: lsp_types::TextDocumentIdentifier { uri },
				position,
			},
			work_done_progress_params: Default::default(),
			partial_result_params: Default::default(),
		})
		.await
	}

	/// Request go to type definition.
	pub async fn goto_type_definition(&self, uri: Uri, position: lsp_types::Position) -> Result<Option<lsp_types::GotoDefinitionResponse>> {
		if !self.supports_type_definition() {
			return Ok(None);
		}
		self.request::<lsp_types::request::GotoTypeDefinition>(lsp_types::GotoDefinitionParams {
			text_document_position_params: lsp_types::TextDocumentPositionParams {
				text_document: lsp_types::TextDocumentIdentifier { uri },
				position,
			},
			work_done_progress_params: Default::default(),
			partial_result_params: Default::default(),
		})
		.await
	}

	/// Request range formatting.
	pub async fn range_formatting(&self, uri: Uri, range: lsp_types::Range, options: lsp_types::FormattingOptions) -> Result<Option<Vec<lsp_types::TextEdit>>> {
		if !self.supports_range_formatting() {
			return Ok(None);
		}
		self.request::<lsp_types::request::RangeFormatting>(lsp_types::DocumentRangeFormattingParams {
			text_document: lsp_types::TextDocumentIdentifier { uri },
			range,
			options,
			work_done_progress_params: Default::default(),
		})
		.await
	}

	/// Request workspace symbols.
	pub async fn workspace_symbol(&self, query: String) -> Result<Option<lsp_types::WorkspaceSymbolResponse>> {
		if !self.supports_workspace_symbol() {
			return Ok(None);
		}
		self.request::<lsp_types::request::WorkspaceSymbolRequest>(lsp_types::WorkspaceSymbolParams {
			query,
			work_done_progress_params: Default::default(),
			partial_result_params: Default::default(),
		})
		.await
	}

	/// Notify the server that files will be renamed, allowing it to return edits
	/// (e.g. import path updates) to apply before the rename.
	pub async fn will_rename_files(&self, files: Vec<lsp_types::FileRename>) -> Result<Option<lsp_types::WorkspaceEdit>> {
		if !self.supports_will_rename_files() {
			return Ok(None);
		}
		self.request::<lsp_types::request::WillRenameFiles>(lsp_types::RenameFilesParams { files })
			.await
	}

	/// Notify the server that files were renamed.
	pub async fn did_rename_files(&self, files: Vec<lsp_types::FileRename>) -> Result<()> {
		if !self.supports_did_rename_files() {
			return Ok(());
		}
		self.notify::<lsp_types::notification::DidRenameFiles>(lsp_types::RenameFilesParams { files })
			.await
	}

	/// Notify the server that files will be created, allowing it to return edits
	/// to apply before the creation.
	pub async fn will_create_files(&self, files: Vec<lsp_types::FileCreate>) -> Result<Option<lsp_types::WorkspaceEdit>> {
		if !self.supports_will_create_files() {
			return Ok(None);
		}
		self.request::<lsp_types::request::WillCreateFiles>(lsp_types::CreateFilesParams { files })
			.await
	}

	/// Notify the server that files were created.
	pub async fn did_create_files(&self, files: Vec<lsp_types::FileCreate>) -> Result<()> {
		if !self.supports_did_create_files() {
			return Ok(());
		}
		self.notify::<lsp_types::notification::DidCreateFiles>(lsp_types::CreateFilesParams { files })
			.await
	}

	/// Notify the server that files will be deleted, allowing it to return edits
	/// to apply before the deletion.
	pub async fn will_delete_files(&self, files: Vec<lsp_types::FileDelete>) -> Result<Option<lsp_types::WorkspaceEdit>> {
		if !self.supports_will_delete_files() {
			return Ok(None);
		}
		self.request::<lsp_types::request::WillDeleteFiles>(lsp_types::DeleteFilesParams { files })
			.await
	}

	/// Notify the server that files were deleted.
	pub async fn did_delete_files(&self, files: Vec<lsp_types::FileDelete>) -> Result<()> {
		if !self.supports_did_delete_files() {
			return Ok(());
		}
		self.notify::<lsp_types::notification::DidDeleteFiles>(lsp_types::DeleteFilesParams { files })
			.await
	}

	/// Request pull diagnostics for a document.
	pub async fn pull_diagnostics(&self, uri: Uri, previous_result_id: Option<String>) -> Result<Option<lsp_types::DocumentDiagnosticReportResult>> {
		if !self.supports_pull_diagnostics() {
			return Ok(None);
		}
		let result: lsp_types::DocumentDiagnosticReportResult = self
			.request::<lsp_types::request::DocumentDiagnosticRequest>(lsp_types::DocumentDiagnosticParams {
				text_document: lsp_types::TextDocumentIdentifier { uri },
				identifier: None,
				previous_result_id,
				work_done_progress_params: Default::default(),
				partial_result_params: Default::default(),
			})
			.await?;
		Ok(Some(result))
	}

	/// Request inlay hints for a range.
	pub async fn inlay_hints(&self, uri: Uri, range: lsp_types::Range) -> Result<Option<Vec<lsp_types::InlayHint>>> {
		if !self.supports_inlay_hint() {
			return Ok(None);
		}
		self.request::<lsp_types::request::InlayHintRequest>(lsp_types::InlayHintParams {
			text_document: lsp_types::TextDocumentIdentifier { uri },
			range,
			work_done_progress_params: Default::default(),
		})
		.await
	}

	/// Request semantic tokens for an entire document.
	pub async fn semantic_tokens_full(&self, uri: Uri) -> Result<Option<lsp_types::SemanticTokensResult>> {
		if !self.supports_semantic_tokens_full() {
			return Ok(None);
		}
		let result = self
			.request::<lsp_types::request::SemanticTokensFullRequest>(lsp_types::SemanticTokensParams {
				text_document: lsp_types::TextDocumentIdentifier { uri },
				work_done_progress_params: Default::default(),
				partial_result_params: Default::default(),
			})
			.await?;
		Ok(result)
	}

	/// Request semantic tokens for a range within a document.
	pub async fn semantic_tokens_range(&self, uri: Uri, range: lsp_types::Range) -> Result<Option<lsp_types::SemanticTokensRangeResult>> {
		if !self.supports_semantic_tokens_range() {
			return Ok(None);
		}
		let result = self
			.request::<lsp_types::request::SemanticTokensRangeRequest>(lsp_types::SemanticTokensRangeParams {
				text_document: lsp_types::TextDocumentIdentifier { uri },
				range,
				work_done_progress_params: Default::default(),
				partial_result_params: Default::default(),
			})
			.await?;
		Ok(result)
	}

	/// Resolve additional details for an inlay hint.
	pub async fn inlay_hint_resolve(&self, hint: lsp_types::InlayHint) -> Result<lsp_types::InlayHint> {
		if !self.supports_inlay_hint_resolve() {
			return Ok(hint);
		}
		self.request::<lsp_types::request::InlayHintResolveRequest>(hint).await
	}

	/// Execute a command on the server.
	pub async fn execute_command(&self, command: String, arguments: Option<Vec<Value>>) -> Result<Option<Value>> {
		if !self.supports_execute_command() {
			return Ok(None);
		}
		let arguments = arguments.unwrap_or_default();
		self.request::<lsp_types::request::ExecuteCommand>(lsp_types::ExecuteCommandParams {
			command,
			arguments,
			work_done_progress_params: Default::default(),
		})
		.await
	}
}
