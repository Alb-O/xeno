use lsp_types::notification::Notification;
use lsp_types::{ClientInfo, InitializeParams, InitializeResult, Uri};
use serde_json::Value;
use tokio::sync::oneshot;

use super::super::capabilities::client_capabilities;
use super::super::handle::ClientHandle;
use super::types::workspace_folder_from_uri;
use crate::types::AnyNotification;
use crate::{Result, uri_from_path};

impl ClientHandle {
	/// Initialize the language server.
	pub async fn initialize(
		&self,
		enable_snippets: bool,
		config: Option<Value>,
	) -> Result<InitializeResult> {
		#[allow(
			deprecated,
			reason = "root_path field deprecated but required by some servers"
		)]
		let params = InitializeParams {
			process_id: Some(std::process::id()),
			workspace_folders: Some(vec![workspace_folder_from_uri(
				self.root_uri
					.clone()
					.unwrap_or_else(|| uri_from_path(&self.root_path).expect("valid path")),
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

		let result = self
			.request::<lsp_types::request::Initialize>(params)
			.await?;

		let _ = self.capabilities.set(result.capabilities.clone());
		self.initialize_notify.notify_waiters();

		let barrier_rx = self
			.transport
			.notify_with_barrier(
				self.id,
				AnyNotification {
					method: lsp_types::notification::Initialized::METHOD.into(),
					params: serde_json::to_value(lsp_types::InitializedParams {})
						.expect("serialize"),
				},
			)
			.await?;

		let _ = barrier_rx.await;
		self.set_ready(true);

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
	pub async fn text_document_did_open(
		&self,
		uri: Uri,
		language_id: String,
		version: i32,
		text: String,
	) -> Result<()> {
		self.notify::<lsp_types::notification::DidOpenTextDocument>(
			lsp_types::DidOpenTextDocumentParams {
				text_document: lsp_types::TextDocumentItem {
					uri,
					language_id,
					version,
					text,
				},
			},
		)
		.await
	}

	/// Notify the server that a document was changed (full sync).
	pub async fn text_document_did_change_full(
		&self,
		uri: Uri,
		version: i32,
		text: String,
	) -> Result<()> {
		let notification = AnyNotification {
			method: lsp_types::notification::DidChangeTextDocument::METHOD.into(),
			params: serde_json::to_value(lsp_types::DidChangeTextDocumentParams {
				text_document: lsp_types::VersionedTextDocumentIdentifier { uri, version },
				content_changes: vec![lsp_types::TextDocumentContentChangeEvent {
					range: None,
					range_length: None,
					text,
				}],
			})
			.expect("Failed to serialize"),
		};
		self.transport.notify(self.id, notification).await
	}

	/// Notify the server that a document was changed (full sync) with a write barrier.
	pub async fn text_document_did_change_full_with_barrier(
		&self,
		uri: Uri,
		version: i32,
		text: String,
	) -> Result<oneshot::Receiver<()>> {
		let notification = AnyNotification {
			method: lsp_types::notification::DidChangeTextDocument::METHOD.into(),
			params: serde_json::to_value(lsp_types::DidChangeTextDocumentParams {
				text_document: lsp_types::VersionedTextDocumentIdentifier { uri, version },
				content_changes: vec![lsp_types::TextDocumentContentChangeEvent {
					range: None,
					range_length: None,
					text,
				}],
			})
			.expect("Failed to serialize"),
		};
		self.transport
			.notify_with_barrier(self.id, notification)
			.await
	}

	/// Notify the server that a document was changed (incremental sync).
	pub async fn text_document_did_change(
		&self,
		uri: Uri,
		version: i32,
		changes: Vec<lsp_types::TextDocumentContentChangeEvent>,
	) -> Result<()> {
		let notification = AnyNotification {
			method: lsp_types::notification::DidChangeTextDocument::METHOD.into(),
			params: serde_json::to_value(lsp_types::DidChangeTextDocumentParams {
				text_document: lsp_types::VersionedTextDocumentIdentifier { uri, version },
				content_changes: changes,
			})
			.expect("Failed to serialize"),
		};
		self.transport.notify(self.id, notification).await
	}

	/// Notify the server that a document was changed (incremental sync) with a write barrier.
	pub async fn text_document_did_change_with_barrier(
		&self,
		uri: Uri,
		version: i32,
		changes: Vec<lsp_types::TextDocumentContentChangeEvent>,
	) -> Result<oneshot::Receiver<()>> {
		let notification = AnyNotification {
			method: lsp_types::notification::DidChangeTextDocument::METHOD.into(),
			params: serde_json::to_value(lsp_types::DidChangeTextDocumentParams {
				text_document: lsp_types::VersionedTextDocumentIdentifier { uri, version },
				content_changes: changes,
			})
			.expect("Failed to serialize"),
		};
		self.transport
			.notify_with_barrier(self.id, notification)
			.await
	}

	/// Notify the server that a document will be saved.
	pub async fn text_document_will_save(
		&self,
		uri: Uri,
		reason: lsp_types::TextDocumentSaveReason,
	) -> Result<()> {
		self.notify::<lsp_types::notification::WillSaveTextDocument>(
			lsp_types::WillSaveTextDocumentParams {
				text_document: lsp_types::TextDocumentIdentifier { uri },
				reason,
			},
		)
		.await
	}

	/// Notify the server that a document was saved.
	pub async fn text_document_did_save(&self, uri: Uri, text: Option<String>) -> Result<()> {
		self.notify::<lsp_types::notification::DidSaveTextDocument>(
			lsp_types::DidSaveTextDocumentParams {
				text_document: lsp_types::TextDocumentIdentifier { uri },
				text,
			},
		)
		.await
	}

	/// Notify the server that a document was closed.
	pub async fn text_document_did_close(&self, uri: Uri) -> Result<()> {
		self.notify::<lsp_types::notification::DidCloseTextDocument>(
			lsp_types::DidCloseTextDocumentParams {
				text_document: lsp_types::TextDocumentIdentifier { uri },
			},
		)
		.await
	}

	/// Request hover information.
	pub async fn hover(
		&self,
		uri: Uri,
		position: lsp_types::Position,
	) -> Result<Option<lsp_types::Hover>> {
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
	pub async fn goto_definition(
		&self,
		uri: Uri,
		position: lsp_types::Position,
	) -> Result<Option<lsp_types::GotoDefinitionResponse>> {
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
	pub async fn references(
		&self,
		uri: Uri,
		position: lsp_types::Position,
		include_declaration: bool,
	) -> Result<Option<Vec<lsp_types::Location>>> {
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
			context: lsp_types::ReferenceContext {
				include_declaration,
			},
		})
		.await
	}

	/// Request document symbols.
	pub async fn document_symbol(
		&self,
		uri: Uri,
	) -> Result<Option<lsp_types::DocumentSymbolResponse>> {
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
	pub async fn formatting(
		&self,
		uri: Uri,
		options: lsp_types::FormattingOptions,
	) -> Result<Option<Vec<lsp_types::TextEdit>>> {
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
	pub async fn code_action(
		&self,
		uri: Uri,
		range: lsp_types::Range,
		context: lsp_types::CodeActionContext,
	) -> Result<Option<lsp_types::CodeActionResponse>> {
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

	/// Request signature help.
	pub async fn signature_help(
		&self,
		uri: Uri,
		position: lsp_types::Position,
	) -> Result<Option<lsp_types::SignatureHelp>> {
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

	/// Request rename.
	pub async fn rename(
		&self,
		uri: Uri,
		position: lsp_types::Position,
		new_name: String,
	) -> Result<Option<lsp_types::WorkspaceEdit>> {
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

	/// Execute a command on the server.
	pub async fn execute_command(
		&self,
		command: String,
		arguments: Option<Vec<Value>>,
	) -> Result<Option<Value>> {
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
