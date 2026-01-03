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
//! # Example
//!
//! ```ignore
//! use evildoer_lsp::client::{Client, ServerConfig, LanguageServerId};
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

use std::collections::HashMap;
use std::ops::ControlFlow;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::Arc;

use lsp_types::notification::Notification;
use lsp_types::request::Request;
use lsp_types::{
	ClientInfo, InitializeParams, InitializeResult, ServerCapabilities, Url, WorkspaceFolder,
};
use parking_lot::Mutex;
use serde_json::Value;
use tokio::process::Command;
use tokio::sync::{Notify, OnceCell};
use tracing::{debug, error, info, warn};

mod capabilities;
mod config;

pub use capabilities::client_capabilities;
pub use config::{LanguageServerId, OffsetEncoding, ServerConfig};

use crate::router::Router;
use crate::{MainLoop, Result, ServerSocket};

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
	root_uri: Option<Url>,
	/// Notification channel for initialization completion.
	initialize_notify: Arc<Notify>,
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
	pub fn root_uri(&self) -> Option<&Url> {
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
					.unwrap_or_else(|| Url::from_file_path(&self.root_path).expect("valid path")),
			)]),
			root_path: self.root_path.to_str().map(String::from),
			root_uri: self.root_uri.clone(),
			initialization_options: config,
			capabilities: client_capabilities(enable_snippets),
			trace: None,
			client_info: Some(ClientInfo {
				name: String::from("evildoer"),
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
		uri: Url,
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
		uri: Url,
		version: i32,
		text: String,
	) -> Result<()> {
		self.notify::<lsp_types::notification::DidChangeTextDocument>(
			lsp_types::DidChangeTextDocumentParams {
				text_document: lsp_types::VersionedTextDocumentIdentifier { uri, version },
				content_changes: vec![lsp_types::TextDocumentContentChangeEvent {
					range: None,
					range_length: None,
					text,
				}],
			},
		)
	}

	/// Notify the server that a document was changed (incremental sync).
	pub fn text_document_did_change(
		&self,
		uri: Url,
		version: i32,
		changes: Vec<lsp_types::TextDocumentContentChangeEvent>,
	) -> Result<()> {
		self.notify::<lsp_types::notification::DidChangeTextDocument>(
			lsp_types::DidChangeTextDocumentParams {
				text_document: lsp_types::VersionedTextDocumentIdentifier { uri, version },
				content_changes: changes,
			},
		)
	}

	/// Notify the server that a document will be saved.
	pub fn text_document_will_save(
		&self,
		uri: Url,
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
	pub fn text_document_did_save(&self, uri: Url, text: Option<String>) -> Result<()> {
		self.notify::<lsp_types::notification::DidSaveTextDocument>(
			lsp_types::DidSaveTextDocumentParams {
				text_document: lsp_types::TextDocumentIdentifier { uri },
				text,
			},
		)
	}

	/// Notify the server that a document was closed.
	pub fn text_document_did_close(&self, uri: Url) -> Result<()> {
		self.notify::<lsp_types::notification::DidCloseTextDocument>(
			lsp_types::DidCloseTextDocumentParams {
				text_document: lsp_types::TextDocumentIdentifier { uri },
			},
		)
	}

	/// Request hover information.
	pub async fn hover(
		&self,
		uri: Url,
		position: lsp_types::Position,
	) -> Result<Option<lsp_types::Hover>> {
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
		uri: Url,
		position: lsp_types::Position,
		context: Option<lsp_types::CompletionContext>,
	) -> Result<Option<lsp_types::CompletionResponse>> {
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
		uri: Url,
		position: lsp_types::Position,
	) -> Result<Option<lsp_types::GotoDefinitionResponse>> {
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
		uri: Url,
		position: lsp_types::Position,
		include_declaration: bool,
	) -> Result<Option<Vec<lsp_types::Location>>> {
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
		uri: Url,
	) -> Result<Option<lsp_types::DocumentSymbolResponse>> {
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
		uri: Url,
		options: lsp_types::FormattingOptions,
	) -> Result<Option<Vec<lsp_types::TextEdit>>> {
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
		uri: Url,
		range: lsp_types::Range,
		context: lsp_types::CodeActionContext,
	) -> Result<Option<lsp_types::CodeActionResponse>> {
		self.request::<lsp_types::request::CodeActionRequest>(lsp_types::CodeActionParams {
			text_document: lsp_types::TextDocumentIdentifier { uri },
			range,
			context,
			work_done_progress_params: Default::default(),
			partial_result_params: Default::default(),
		})
		.await
	}

	/// Request rename.
	pub async fn rename(
		&self,
		uri: Url,
		position: lsp_types::Position,
		new_name: String,
	) -> Result<Option<lsp_types::WorkspaceEdit>> {
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

	/// Send a request to the language server.
	pub async fn request<R: Request>(&self, params: R::Params) -> Result<R::Result> {
		self.socket.request::<R>(params).await
	}

	/// Send a notification to the language server.
	pub fn notify<N: Notification>(&self, params: N::Params) -> Result<()> {
		self.socket.notify::<N>(params)
	}
}

/// State for the LSP client service.
///
/// This handles incoming server->client notifications and requests.
struct ClientState {
	/// Diagnostics received from the server, keyed by document URI.
	diagnostics: Mutex<HashMap<Url, Vec<lsp_types::Diagnostic>>>,
}

impl ClientState {
	/// Creates a new empty client state.
	fn new() -> Self {
		Self {
			diagnostics: Mutex::new(HashMap::new()),
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
) -> Result<(ClientHandle, tokio::task::JoinHandle<Result<()>>)> {
	let root_uri = Url::from_file_path(&config.root_path).ok();

	let mut cmd = Command::new(&config.command);
	cmd.args(&config.args)
		.envs(&config.env)
		.stdin(Stdio::piped())
		.stdout(Stdio::piped())
		.stderr(Stdio::piped())
		.current_dir(&config.root_path)
		.kill_on_drop(true);

	let mut process = cmd.spawn().map_err(crate::Error::Io)?;

	let stdin = process.stdin.take().expect("Failed to open stdin");
	let stdout = process.stdout.take().expect("Failed to open stdout");
	let _stderr = process.stderr.take().expect("Failed to open stderr");

	let capabilities = Arc::new(OnceCell::new());
	let initialize_notify = Arc::new(Notify::new());

	let state = Arc::new(ClientState::new());

	// Build the router for handling server->client messages
	let (main_loop, socket) = MainLoop::new_client(|_socket| {
		let mut router = Router::new(state.clone());
		router
			.notification::<lsp_types::notification::PublishDiagnostics>(|state, params| {
				let mut diagnostics = state.diagnostics.lock();
				diagnostics.insert(params.uri, params.diagnostics);
				ControlFlow::Continue(())
			})
			.notification::<lsp_types::notification::LogMessage>(|_state, params| {
				// Log messages from the server
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
			.notification::<lsp_types::notification::ShowMessage>(|_state, params| {
				// Show message notifications
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
			// Catch-all for unhandled notifications
			.unhandled_notification(|_state, notif| {
				debug!(target: "lsp", method = %notif.method, "Unhandled notification");
				ControlFlow::Continue(())
			});
		router
	});

	let handle = ClientHandle {
		id,
		name,
		socket,
		capabilities,
		root_path: config.root_path,
		root_uri,
		initialize_notify,
	};

	let join_handle = tokio::spawn(async move {
		// Convert tokio I/O to futures I/O
		let stdin = tokio_util::compat::TokioAsyncWriteCompatExt::compat_write(stdin);
		let stdout = tokio_util::compat::TokioAsyncReadCompatExt::compat(stdout);

		main_loop.run_buffered(stdout, stdin).await?;

		drop(process);
		Ok(())
	});

	Ok((handle, join_handle))
}

/// Create a workspace folder from a URI.
fn workspace_folder_from_uri(uri: Url) -> WorkspaceFolder {
	WorkspaceFolder {
		name: uri
			.path_segments()
			.and_then(|mut segments| segments.next_back())
			.map(String::from)
			.unwrap_or_default(),
		uri,
	}
}
