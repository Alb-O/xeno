//! High-level typed LSP API methods.
//!
//! This module contains the convenience methods on `ClientHandle` for
//! common LSP operations like hover, completion, formatting, etc.

use futures::channel::oneshot;
use lsp_types::notification::Notification;
use lsp_types::{ClientInfo, InitializeParams, InitializeResult, Uri, WorkspaceFolder};
use serde_json::Value;

use crate::types::AnyNotification;
use crate::{Result, uri_from_path};

use super::capabilities::client_capabilities;
use super::handle::ClientHandle;
use super::outbox::OutboundMsg;
use super::state::ServerState;

impl ClientHandle {
	/// Initialize the language server.
	///
	/// This sends the `initialize` request and waits for the response.
	/// After initialization, sends `initialized` notification.
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

		let (barrier_tx, barrier_rx) = oneshot::channel();
		self.send_outbound(OutboundMsg::Notification {
			notification: AnyNotification {
				method: lsp_types::notification::Initialized::METHOD.into(),
				params: serde_json::to_value(lsp_types::InitializedParams {}).expect("serialize"),
			},
			barrier: Some(barrier_tx),
		})?;
		let _ = barrier_rx.await;
		self.set_state(ServerState::Ready);

		Ok(result)
	}

	/// Shutdown the language server gracefully.
	pub async fn shutdown(&self) -> Result<()> {
		self.request::<lsp_types::request::Shutdown>(()).await
	}

	/// Send exit notification to the server.
	pub fn exit(&self) -> Result<()> {
		self.notify::<lsp_types::notification::Exit>(())
	}

	/// Shutdown and exit the language server.
	pub async fn shutdown_and_exit(&self) -> Result<()> {
		self.shutdown().await?;
		self.exit()
	}

	/// Notify the server that a document was opened.
	pub fn text_document_did_open(
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
	}

	/// Notify the server that a document was changed (full sync).
	pub fn text_document_did_change_full(
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
		self.send_outbound(OutboundMsg::Notification {
			notification,
			barrier: None,
		})
	}

	/// Notify the server that a document was changed (full sync) with a write barrier.
	pub fn text_document_did_change_full_with_barrier(
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
		let (tx, rx) = oneshot::channel();
		self.send_outbound(OutboundMsg::Notification {
			notification,
			barrier: Some(tx),
		})?;
		Ok(rx)
	}

	/// Notify the server that a document was changed (incremental sync).
	pub fn text_document_did_change(
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
		self.send_outbound(OutboundMsg::Notification {
			notification,
			barrier: None,
		})
	}

	/// Notify the server that a document was changed (incremental sync) with a write barrier.
	pub fn text_document_did_change_with_barrier(
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
		let (tx, rx) = oneshot::channel();
		self.send_outbound(OutboundMsg::Notification {
			notification,
			barrier: Some(tx),
		})?;
		Ok(rx)
	}

	/// Notify the server that a document will be saved.
	pub fn text_document_will_save(
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
	}

	/// Notify the server that a document was saved.
	pub fn text_document_did_save(&self, uri: Uri, text: Option<String>) -> Result<()> {
		self.notify::<lsp_types::notification::DidSaveTextDocument>(
			lsp_types::DidSaveTextDocumentParams {
				text_document: lsp_types::TextDocumentIdentifier { uri },
				text,
			},
		)
	}

	/// Notify the server that a document was closed.
	pub fn text_document_did_close(&self, uri: Uri) -> Result<()> {
		self.notify::<lsp_types::notification::DidCloseTextDocument>(
			lsp_types::DidCloseTextDocumentParams {
				text_document: lsp_types::TextDocumentIdentifier { uri },
			},
		)
	}

	/// Request hover information.
	///
	/// Returns `Ok(None)` if the server doesn't support hover.
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
	///
	/// Returns `Ok(None)` if the server doesn't support completion.
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
	///
	/// Returns `Ok(None)` if the server doesn't support definition.
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
	///
	/// Returns `Ok(None)` if the server doesn't support references.
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
	///
	/// Returns `Ok(None)` if the server doesn't support document symbols.
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
	///
	/// Returns `Ok(None)` if the server doesn't support formatting.
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
	///
	/// Returns `Ok(None)` if the server doesn't support code actions.
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
	///
	/// Returns `Ok(None)` if the server doesn't support signature help.
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
	///
	/// Returns `Ok(None)` if the server doesn't support execute command.
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

/// Create a workspace folder from a URI.
fn workspace_folder_from_uri(uri: Uri) -> WorkspaceFolder {
	let name = uri
		.as_str()
		.rsplit('/')
		.next()
		.filter(|s| !s.is_empty())
		.unwrap_or_default()
		.to_string();
	WorkspaceFolder { name, uri }
}
