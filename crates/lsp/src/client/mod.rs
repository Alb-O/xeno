//! LSP client wrapper for spawning and communicating with language servers.
//!
//! This module provides the [`ClientHandle`] type which wraps an LSP language server
//! process and provides methods for sending requests and notifications.
//!
//! # Architecture
//!
//! The client spawns a language server process and communicates via stdin/stdout.
//! It uses the [`crate::MainLoop`] to drive the LSP protocol, running in a
//! background task. The client provides a [`ServerSocket`] for sending requests
//! and notifications.
//!
//! # Event Handling
//!
//! Server-to-client notifications (diagnostics, progress, etc.) are delivered via
//! the [`LspEventHandler`] trait. Implement this trait to receive LSP events:
//!
//! ```ignore
//! use xeno_lsp::client::{LspEventHandler, LanguageServerId};
//!
//! struct MyHandler;
//!
//! impl LspEventHandler for MyHandler {
//!     fn on_diagnostics(&self, uri: Uri, diagnostics: Vec<Diagnostic>) {
//!         // Update UI with new diagnostics
//!     }
//! }
//! ```
//!
//! # Example
//!
//! ```ignore
//! use xeno_lsp::client::{Client, ServerConfig, LanguageServerId};
//!
//! let config = ServerConfig::new("rust-analyzer", "/path/to/project");
//! let client = Client::start(LanguageServerId(1), "rust-analyzer".into(), config)?;
//!
//! // Initialize the server
//! client.initialize(true).await?;
//!
//! // Use the client for LSP operations
//! let hover = client.hover(uri, position).await?;
//! ```

use std::ops::ControlFlow;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::Arc;
use std::time::Duration;

use futures::channel::oneshot;
use lsp_types::notification::Notification;
use lsp_types::request::Request;
use lsp_types::{
	ClientInfo, InitializeParams, InitializeResult, ServerCapabilities, Uri, WorkspaceFolder,
};
use serde_json::Value;
use tokio::process::Command;
use tokio::sync::{Notify, OnceCell, mpsc};
use tracing::{debug, error, info, warn};

mod capabilities;
mod config;
mod event_handler;

pub use capabilities::client_capabilities;
pub use config::{LanguageServerId, OffsetEncoding, ServerConfig};
pub use event_handler::{LogLevel, LspEventHandler, NoOpEventHandler, SharedEventHandler};

use crate::message::Message;
use crate::router::Router;
use crate::socket::MainLoopEvent;
use crate::types::{AnyNotification, AnyRequest, AnyResponse, RequestId};
use crate::{Error, MainLoop, Result, ServerSocket};

/// Handle to an LSP language server.
///
/// This provides a high-level API for communicating with a language server.
/// The actual I/O and main loop run in a separate task - this handle just
/// holds the socket for sending messages.
#[derive(Clone)]
pub struct ClientHandle {
	/// Unique identifier for this client.
	id: LanguageServerId,
	/// Human-readable name (usually the command name).
	name: String,
	/// Socket for communicating with the server.
	socket: ServerSocket,
	/// Server capabilities (set after initialization).
	capabilities: Arc<OnceCell<ServerCapabilities>>,
	/// Root path for the workspace.
	root_path: PathBuf,
	/// Root URI for the workspace.
	root_uri: Option<Uri>,
	/// Notification channel for initialization completion.
	initialize_notify: Arc<Notify>,
	/// Outbound message queue for serialized writes.
	outbound_tx: mpsc::Sender<OutboundMsg>,
	/// Per-request timeout.
	timeout: Duration,
}

impl std::fmt::Debug for ClientHandle {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		f.debug_struct("ClientHandle")
			.field("id", &self.id)
			.field("name", &self.name)
			.field("root_path", &self.root_path)
			.field("initialized", &self.capabilities.initialized())
			.finish_non_exhaustive()
	}
}

impl ClientHandle {
	/// Get the client's unique identifier.
	pub fn id(&self) -> LanguageServerId {
		self.id
	}

	/// Get the client's name.
	pub fn name(&self) -> &str {
		&self.name
	}

	/// Check if the server has been initialized.
	pub fn is_initialized(&self) -> bool {
		self.capabilities.initialized()
	}

	/// Get the server's capabilities.
	///
	/// # Panics
	///
	/// Panics if called before initialization completes.
	pub fn capabilities(&self) -> &ServerCapabilities {
		self.capabilities
			.get()
			.expect("language server not yet initialized")
	}

	/// Get the server's capabilities if initialized.
	pub fn try_capabilities(&self) -> Option<&ServerCapabilities> {
		self.capabilities.get()
	}

	/// Check if the server supports hover.
	pub fn supports_hover(&self) -> bool {
		self.try_capabilities()
			.is_some_and(|c| c.hover_provider.is_some())
	}

	/// Check if the server supports completion.
	pub fn supports_completion(&self) -> bool {
		self.try_capabilities()
			.is_some_and(|c| c.completion_provider.is_some())
	}

	/// Check if the server supports formatting.
	pub fn supports_formatting(&self) -> bool {
		self.try_capabilities()
			.is_some_and(|c| c.document_formatting_provider.is_some())
	}

	/// Check if the server supports go to definition.
	pub fn supports_definition(&self) -> bool {
		self.try_capabilities()
			.is_some_and(|c| c.definition_provider.is_some())
	}

	/// Check if the server supports find references.
	pub fn supports_references(&self) -> bool {
		self.try_capabilities()
			.is_some_and(|c| c.references_provider.is_some())
	}

	/// Check if the server supports document symbols.
	pub fn supports_document_symbol(&self) -> bool {
		self.try_capabilities()
			.is_some_and(|c| c.document_symbol_provider.is_some())
	}

	/// Check if the server supports code actions.
	pub fn supports_code_action(&self) -> bool {
		self.try_capabilities()
			.is_some_and(|c| c.code_action_provider.is_some())
	}

	/// Check if the server supports signature help.
	pub fn supports_signature_help(&self) -> bool {
		self.try_capabilities()
			.is_some_and(|c| c.signature_help_provider.is_some())
	}

	/// Check if the server supports rename.
	pub fn supports_rename(&self) -> bool {
		self.try_capabilities()
			.is_some_and(|c| c.rename_provider.is_some())
	}

	/// Check if the server supports execute command.
	pub fn supports_execute_command(&self) -> bool {
		self.try_capabilities()
			.is_some_and(|c| c.execute_command_provider.is_some())
	}

	/// Get the offset encoding negotiated with the server.
	pub fn offset_encoding(&self) -> OffsetEncoding {
		self.capabilities()
			.position_encoding
			.as_ref()
			.and_then(OffsetEncoding::from_lsp)
			.unwrap_or_default()
	}

	/// Get the root path.
	pub fn root_path(&self) -> &Path {
		&self.root_path
	}

	/// Get the root URI.
	pub fn root_uri(&self) -> Option<&Uri> {
		self.root_uri.as_ref()
	}

	/// Wait for initialization to complete.
	pub async fn wait_initialized(&self) {
		if self.is_initialized() {
			return;
		}
		self.initialize_notify.notified().await;
	}

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
					.unwrap_or_else(|| crate::uri_from_path(&self.root_path).expect("valid path")),
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

		// Send initialized notification
		self.notify::<lsp_types::notification::Initialized>(lsp_types::InitializedParams {})?;

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
		self.send_outbound(OutboundMsg::DidChange {
			notification,
			ack: None,
		})
	}

	/// Notify the server that a document was changed (full sync) with an ack.
	pub fn text_document_did_change_full_with_ack(
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
		self.send_outbound(OutboundMsg::DidChange {
			notification,
			ack: Some(tx),
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
		self.send_outbound(OutboundMsg::DidChange {
			notification,
			ack: None,
		})
	}

	/// Notify the server that a document was changed (incremental sync) with an ack.
	pub fn text_document_did_change_with_ack(
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
		self.send_outbound(OutboundMsg::DidChange {
			notification,
			ack: Some(tx),
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
		self.request::<lsp_types::request::SignatureHelpRequest>(
			lsp_types::SignatureHelpParams {
				text_document_position_params: lsp_types::TextDocumentPositionParams {
					text_document: lsp_types::TextDocumentIdentifier { uri },
					position,
				},
				work_done_progress_params: Default::default(),
				context: None,
			},
		)
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
		self.request::<lsp_types::request::ExecuteCommand>(
			lsp_types::ExecuteCommandParams {
				command,
				arguments,
				work_done_progress_params: Default::default(),
			},
		)
		.await
	}

	/// Send a request to the language server.
	pub async fn request<R: Request>(&self, params: R::Params) -> Result<R::Result> {
		let req = AnyRequest {
			id: RequestId::Number(0),
			method: R::METHOD.into(),
			params: serde_json::to_value(params).expect("Failed to serialize"),
		};
		let (tx, rx) = oneshot::channel();
		self.outbound_tx
			.send(OutboundMsg::Request {
				request: req,
				response_tx: tx,
			})
			.await
			.map_err(|_| Error::ServiceStopped)?;
		let resp = if self.timeout == Duration::ZERO {
			rx.await.map_err(|_| Error::ServiceStopped)?
		} else {
			match tokio::time::timeout(self.timeout, rx).await {
				Ok(resp) => resp.map_err(|_| Error::ServiceStopped)?,
				Err(_) => return Err(Error::RequestTimeout(R::METHOD.into())),
			}
		};
		match resp.error {
			None => Ok(serde_json::from_value(resp.result.unwrap_or_default())?),
			Some(err) => Err(Error::Response(err)),
		}
	}

	/// Send a notification to the language server.
	pub fn notify<N: Notification>(&self, params: N::Params) -> Result<()> {
		let notif = AnyNotification {
			method: N::METHOD.into(),
			params: serde_json::to_value(params).expect("Failed to serialize"),
		};
		self.send_outbound(OutboundMsg::Notification { notification: notif })
	}

	fn send_outbound(&self, msg: OutboundMsg) -> Result<()> {
		self.outbound_tx.try_send(msg).map_err(|err| match err {
			tokio::sync::mpsc::error::TrySendError::Closed(_) => Error::ServiceStopped,
			tokio::sync::mpsc::error::TrySendError::Full(_) => {
				Error::Protocol("Outbound LSP queue is full".into())
			}
		})
	}
}

const OUTBOUND_QUEUE_LEN: usize = 256;

enum OutboundMsg {
	Notification { notification: AnyNotification },
	Request {
		request: AnyRequest,
		response_tx: oneshot::Sender<AnyResponse>,
	},
	DidChange {
		notification: AnyNotification,
		ack: Option<oneshot::Sender<()>>,
	},
}

async fn outbound_dispatcher(
	mut rx: mpsc::Receiver<OutboundMsg>,
	socket: ServerSocket,
) {
	while let Some(msg) = rx.recv().await {
		let result = match msg {
			OutboundMsg::Notification { notification } => socket
				.0
				.send(MainLoopEvent::Outgoing(Message::Notification(notification))),
			OutboundMsg::Request {
				request,
				response_tx,
			} => socket
				.0
				.send(MainLoopEvent::OutgoingRequest(request, response_tx)),
			OutboundMsg::DidChange { notification, ack } => match ack {
				Some(ack) => socket.0.send(MainLoopEvent::OutgoingWithAck(
					Message::Notification(notification),
					ack,
				)),
				None => socket
					.0
					.send(MainLoopEvent::Outgoing(Message::Notification(notification))),
			},
		};

		if let Err(err) = result {
			warn!(error = %err, "Failed to queue LSP outbound message");
		}
	}
}

/// State for the LSP client service.
///
/// This handles incoming server->client notifications and requests.
struct ClientState {
	/// Server ID for this client.
	server_id: LanguageServerId,
	/// Event handler for LSP events.
	event_handler: SharedEventHandler,
}

impl ClientState {
	/// Creates a new client state with the given event handler.
	fn new(server_id: LanguageServerId, event_handler: SharedEventHandler) -> Self {
		Self {
			server_id,
			event_handler,
		}
	}
}

/// Start a language server process and return a handle to communicate with it.
///
/// This spawns the server process and starts the main loop in a background task.
/// Returns a [`ClientHandle`] that can be used to send requests and notifications.
///
/// # Arguments
///
/// * `id` - Unique identifier for this server instance
/// * `name` - Human-readable name for the server
/// * `config` - Server configuration (command, args, root path, etc.)
/// * `event_handler` - Optional handler for server-to-client events (diagnostics, etc.)
///
/// # Returns
///
/// A tuple of:
/// * `ClientHandle` - Handle for communicating with the server
/// * `JoinHandle` - Handle to the background task running the main loop
pub fn start_server(
	id: LanguageServerId,
	name: String,
	config: ServerConfig,
	event_handler: Option<SharedEventHandler>,
) -> Result<(ClientHandle, tokio::task::JoinHandle<Result<()>>)> {
	let root_uri = crate::uri_from_path(&config.root_path);

	let mut cmd = Command::new(&config.command);
	cmd.args(&config.args)
		.envs(&config.env)
		.stdin(Stdio::piped())
		.stdout(Stdio::piped())
		.stderr(Stdio::piped())
		.current_dir(&config.root_path)
		.kill_on_drop(true);

	// Detach from controlling TTY to prevent LSP from writing directly to terminal
	#[cfg(unix)]
	cmd.process_group(0);

	let mut process = cmd.spawn().map_err(crate::Error::Io)?;

	let stdin = process.stdin.take().expect("Failed to open stdin");
	let stdout = process.stdout.take().expect("Failed to open stdout");
	let stderr = process.stderr.take().expect("Failed to open stderr");

	// Log stderr from the LSP server
	let stderr_id = id;
	tokio::spawn(async move {
		use tokio::io::AsyncBufReadExt;
		let reader = tokio::io::BufReader::new(stderr);
		let mut lines = reader.lines();
		while let Ok(Some(line)) = lines.next_line().await {
			warn!(server_id = stderr_id.0, stderr = %line, "LSP server stderr");
		}
	});

	let capabilities = Arc::new(OnceCell::new());
	let initialize_notify = Arc::new(Notify::new());

	// Use provided event handler or a no-op default
	let handler: SharedEventHandler = event_handler.unwrap_or_else(|| Arc::new(NoOpEventHandler));
	let state = Arc::new(ClientState::new(id, handler));

	// Build the router for handling server->client messages
	let (main_loop, socket) = MainLoop::new_client(|_socket| {
		let mut router = Router::new(state.clone());
		router
			.notification::<lsp_types::notification::PublishDiagnostics>(|state, params| {
				debug!(
					target: "lsp",
					server_id = state.server_id.0,
					uri = params.uri.as_str(),
					count = params.diagnostics.len(),
					"Received diagnostics"
				);
				state
					.event_handler
					.on_diagnostics(state.server_id, params.uri, params.diagnostics);
				ControlFlow::Continue(())
			})
			.notification::<lsp_types::notification::Progress>(|state, params| {
				state.event_handler.on_progress(state.server_id, params);
				ControlFlow::Continue(())
			})
			.notification::<lsp_types::notification::LogMessage>(|state, params| {
				let level = LogLevel::from(params.typ);
				state
					.event_handler
					.on_log_message(state.server_id, level, &params.message);
				// Also log to tracing for debugging
				match params.typ {
					lsp_types::MessageType::ERROR => {
						error!(target: "lsp", message = %params.message, "Server log")
					}
					lsp_types::MessageType::WARNING => {
						warn!(target: "lsp", message = %params.message, "Server log")
					}
					lsp_types::MessageType::INFO => {
						info!(target: "lsp", message = %params.message, "Server log")
					}
					lsp_types::MessageType::LOG => {
						debug!(target: "lsp", message = %params.message, "Server log")
					}
					_ => {}
				}
				ControlFlow::Continue(())
			})
			.notification::<lsp_types::notification::ShowMessage>(|state, params| {
				let level = LogLevel::from(params.typ);
				state
					.event_handler
					.on_show_message(state.server_id, level, &params.message);
				// Also log to tracing
				match params.typ {
					lsp_types::MessageType::ERROR => {
						error!(target: "lsp", message = %params.message, "Server message")
					}
					lsp_types::MessageType::WARNING => {
						warn!(target: "lsp", message = %params.message, "Server message")
					}
					_ => {
						info!(target: "lsp", message = %params.message, "Server message")
					}
				}
				ControlFlow::Continue(())
			})
			// Server->client requests
			.request::<lsp_types::request::WorkspaceConfiguration, _>(|_state, params| {
				// Return empty config object for each requested item
				let result: Vec<serde_json::Value> =
					params.items.iter().map(|_| serde_json::json!({})).collect();
				async move { Ok(result) }
			})
			.request::<lsp_types::request::WorkDoneProgressCreate, _>(|_state, _params| {
				// Acknowledge work done progress creation
				async move { Ok(()) }
			})
			// Catch-all for unhandled notifications
			.unhandled_notification(|_state, notif| {
				debug!(target: "lsp", method = %notif.method, "Unhandled notification");
				ControlFlow::Continue(())
			});
		router
	});

	let (outbound_tx, outbound_rx) = mpsc::channel(OUTBOUND_QUEUE_LEN);
	let outbound_socket = socket.clone();
	tokio::spawn(outbound_dispatcher(outbound_rx, outbound_socket));

	let handle = ClientHandle {
		id,
		name,
		socket,
		capabilities,
		root_path: config.root_path,
		root_uri,
		initialize_notify,
		outbound_tx,
		timeout: Duration::from_secs(config.timeout_secs),
	};

	let server_id = id;
	let join_handle = tokio::spawn(async move {
		// Convert tokio I/O to futures I/O
		let stdin = tokio_util::compat::TokioAsyncWriteCompatExt::compat_write(stdin);
		let stdout = tokio_util::compat::TokioAsyncReadCompatExt::compat(stdout);

		let result = main_loop.run_buffered(stdout, stdin).await;
		if let Err(ref e) = result {
			error!(server_id = server_id.0, error = %e, "LSP main loop error");
		} else {
			info!(server_id = server_id.0, "LSP main loop exited normally");
		}

		drop(process);
		result
	});

	Ok((handle, join_handle))
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

#[cfg(test)]
mod tests {
	use futures::StreamExt;

	use super::*;
	use crate::socket::PeerSocket;

	#[tokio::test]
	async fn outbound_dispatcher_preserves_fifo_order() {
		let (peer_tx, mut peer_rx) = futures::channel::mpsc::unbounded();
		let socket = ServerSocket(PeerSocket { tx: peer_tx });

		let (outbound_tx, outbound_rx) = mpsc::channel(4);
		tokio::spawn(outbound_dispatcher(outbound_rx, socket));

		outbound_tx
			.send(OutboundMsg::Notification {
				notification: AnyNotification {
					method: "first".into(),
					params: serde_json::Value::Null,
				},
			})
			.await
			.unwrap();
		outbound_tx
			.send(OutboundMsg::Notification {
				notification: AnyNotification {
					method: "second".into(),
					params: serde_json::Value::Null,
				},
			})
			.await
			.unwrap();

		let first = peer_rx.next().await.expect("first event");
		let second = peer_rx.next().await.expect("second event");

		match first {
			MainLoopEvent::Outgoing(Message::Notification(notif)) => {
				assert_eq!(notif.method, "first");
			}
			other => panic!("unexpected first event: {:?}", other),
		}

		match second {
			MainLoopEvent::Outgoing(Message::Notification(notif)) => {
				assert_eq!(notif.method, "second");
			}
			other => panic!("unexpected second event: {:?}", other),
		}
	}
}
