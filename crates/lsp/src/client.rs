//! LSP client wrapper for spawning and communicating with language servers.
//!
//! This module provides the [`Client`] type which wraps an LSP language server
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

use tracing::{debug, error, info, warn};
use std::collections::HashMap;
use std::ops::ControlFlow;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::Arc;

use lsp_types::notification::Notification;
use lsp_types::request::Request;
use lsp_types::{
	ClientCapabilities, ClientInfo, CompletionClientCapabilities, CompletionItemCapability,
	CompletionItemCapabilityResolveSupport, DiagnosticClientCapabilities,
	GeneralClientCapabilities, HoverClientCapabilities, InitializeParams, InitializeResult,
	MarkupKind, PositionEncodingKind, PublishDiagnosticsClientCapabilities,
	RenameClientCapabilities, ServerCapabilities, SignatureHelpClientCapabilities,
	SignatureInformationSettings, TagSupport, TextDocumentClientCapabilities, Url,
	WindowClientCapabilities, WorkspaceClientCapabilities, WorkspaceFolder,
};
use parking_lot::Mutex;
use serde_json::Value;
use tokio::process::Command;
use tokio::sync::{Notify, OnceCell};

use crate::router::Router;
use crate::{MainLoop, Result, ServerSocket};

/// Unique identifier for a language server instance.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct LanguageServerId(pub u64);

impl std::fmt::Display for LanguageServerId {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		write!(f, "LSP#{}", self.0)
	}
}

/// Offset encoding for LSP positions.
///
/// LSP uses UTF-16 by default, but servers can negotiate different encodings.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum OffsetEncoding {
	/// UTF-8 byte offsets.
	Utf8,
	/// UTF-16 code unit offsets (LSP default).
	#[default]
	Utf16,
	/// UTF-32 / Unicode codepoint offsets.
	Utf32,
}

impl OffsetEncoding {
	/// Parse from LSP position encoding kind.
	pub fn from_lsp(kind: &PositionEncodingKind) -> Option<Self> {
		match kind.as_str() {
			"utf-8" => Some(Self::Utf8),
			"utf-16" => Some(Self::Utf16),
			"utf-32" => Some(Self::Utf32),
			_ => None,
		}
	}
}

/// Configuration for starting a language server.
#[derive(Debug, Clone)]
pub struct ServerConfig {
	/// Command to spawn the language server.
	pub command: String,
	/// Arguments to pass to the command.
	pub args: Vec<String>,
	/// Environment variables to set.
	pub env: HashMap<String, String>,
	/// Root path for the workspace.
	pub root_path: PathBuf,
	/// Request timeout in seconds.
	pub timeout_secs: u64,
	/// Optional server-specific configuration.
	pub config: Option<Value>,
}

impl ServerConfig {
	/// Create a new server configuration.
	pub fn new(command: impl Into<String>, root_path: impl Into<PathBuf>) -> Self {
		Self {
			command: command.into(),
			args: Vec::new(),
			env: HashMap::new(),
			root_path: root_path.into(),
			timeout_secs: 30,
			config: None,
		}
	}

	/// Add command line arguments.
	pub fn args(mut self, args: impl IntoIterator<Item = impl Into<String>>) -> Self {
		self.args = args.into_iter().map(Into::into).collect();
		self
	}

	/// Add environment variables.
	pub fn env(
		mut self,
		env: impl IntoIterator<Item = (impl Into<String>, impl Into<String>)>,
	) -> Self {
		self.env = env.into_iter().map(|(k, v)| (k.into(), v.into())).collect();
		self
	}

	/// Set request timeout.
	pub fn timeout(mut self, secs: u64) -> Self {
		self.timeout_secs = secs;
		self
	}

	/// Set server-specific configuration.
	pub fn config(mut self, config: Value) -> Self {
		self.config = Some(config);
		self
	}
}

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
		#[allow(deprecated)]
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

		// Store capabilities
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

	// Create the client state
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

	// Create the client handle
	let handle = ClientHandle {
		id,
		name,
		socket,
		capabilities,
		root_path: config.root_path,
		root_uri,
		initialize_notify,
	};

	// Spawn the main loop task
	let join_handle = tokio::spawn(async move {
		// Convert tokio I/O to futures I/O
		let stdin = tokio_util::compat::TokioAsyncWriteCompatExt::compat_write(stdin);
		let stdout = tokio_util::compat::TokioAsyncReadCompatExt::compat(stdout);

		main_loop.run_buffered(stdout, stdin).await?;

		// Keep process handle alive
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

/// Build client capabilities for initialization.
fn client_capabilities(enable_snippets: bool) -> ClientCapabilities {
	ClientCapabilities {
		workspace: Some(WorkspaceClientCapabilities {
			configuration: Some(true),
			did_change_configuration: Some(lsp_types::DynamicRegistrationClientCapabilities {
				dynamic_registration: Some(false),
			}),
			workspace_folders: Some(true),
			apply_edit: Some(true),
			symbol: Some(lsp_types::WorkspaceSymbolClientCapabilities {
				dynamic_registration: Some(false),
				..Default::default()
			}),
			execute_command: Some(lsp_types::DynamicRegistrationClientCapabilities {
				dynamic_registration: Some(false),
			}),
			inlay_hint: Some(lsp_types::InlayHintWorkspaceClientCapabilities {
				refresh_support: Some(false),
			}),
			workspace_edit: Some(lsp_types::WorkspaceEditClientCapabilities {
				document_changes: Some(true),
				resource_operations: Some(vec![
					lsp_types::ResourceOperationKind::Create,
					lsp_types::ResourceOperationKind::Rename,
					lsp_types::ResourceOperationKind::Delete,
				]),
				failure_handling: Some(lsp_types::FailureHandlingKind::Abort),
				normalizes_line_endings: Some(false),
				change_annotation_support: None,
			}),
			did_change_watched_files: Some(lsp_types::DidChangeWatchedFilesClientCapabilities {
				dynamic_registration: Some(true),
				relative_pattern_support: Some(false),
			}),
			file_operations: Some(lsp_types::WorkspaceFileOperationsClientCapabilities {
				will_rename: Some(true),
				did_rename: Some(true),
				..Default::default()
			}),
			diagnostic: Some(lsp_types::DiagnosticWorkspaceClientCapabilities {
				refresh_support: Some(true),
			}),
			..Default::default()
		}),
		text_document: Some(TextDocumentClientCapabilities {
			completion: Some(CompletionClientCapabilities {
				completion_item: Some(CompletionItemCapability {
					snippet_support: Some(enable_snippets),
					resolve_support: Some(CompletionItemCapabilityResolveSupport {
						properties: vec![
							String::from("documentation"),
							String::from("detail"),
							String::from("additionalTextEdits"),
						],
					}),
					insert_replace_support: Some(true),
					deprecated_support: Some(true),
					tag_support: Some(TagSupport {
						value_set: vec![lsp_types::CompletionItemTag::DEPRECATED],
					}),
					..Default::default()
				}),
				completion_item_kind: Some(lsp_types::CompletionItemKindCapability {
					..Default::default()
				}),
				context_support: None,
				..Default::default()
			}),
			hover: Some(HoverClientCapabilities {
				content_format: Some(vec![MarkupKind::Markdown]),
				..Default::default()
			}),
			signature_help: Some(SignatureHelpClientCapabilities {
				signature_information: Some(SignatureInformationSettings {
					documentation_format: Some(vec![MarkupKind::Markdown]),
					parameter_information: Some(lsp_types::ParameterInformationSettings {
						label_offset_support: Some(true),
					}),
					active_parameter_support: Some(true),
				}),
				..Default::default()
			}),
			rename: Some(RenameClientCapabilities {
				dynamic_registration: Some(false),
				prepare_support: Some(true),
				prepare_support_default_behavior: None,
				honors_change_annotations: Some(false),
			}),
			formatting: Some(lsp_types::DocumentFormattingClientCapabilities {
				dynamic_registration: Some(false),
			}),
			code_action: Some(lsp_types::CodeActionClientCapabilities {
				code_action_literal_support: Some(lsp_types::CodeActionLiteralSupport {
					code_action_kind: lsp_types::CodeActionKindLiteralSupport {
						value_set: [
							lsp_types::CodeActionKind::EMPTY,
							lsp_types::CodeActionKind::QUICKFIX,
							lsp_types::CodeActionKind::REFACTOR,
							lsp_types::CodeActionKind::REFACTOR_EXTRACT,
							lsp_types::CodeActionKind::REFACTOR_INLINE,
							lsp_types::CodeActionKind::REFACTOR_REWRITE,
							lsp_types::CodeActionKind::SOURCE,
							lsp_types::CodeActionKind::SOURCE_ORGANIZE_IMPORTS,
							lsp_types::CodeActionKind::SOURCE_FIX_ALL,
						]
						.iter()
						.map(|kind| kind.as_str().to_string())
						.collect(),
					},
				}),
				is_preferred_support: Some(true),
				disabled_support: Some(true),
				data_support: Some(true),
				resolve_support: Some(lsp_types::CodeActionCapabilityResolveSupport {
					properties: vec!["edit".to_owned(), "command".to_owned()],
				}),
				..Default::default()
			}),
			diagnostic: Some(DiagnosticClientCapabilities {
				dynamic_registration: Some(false),
				related_document_support: Some(true),
			}),
			publish_diagnostics: Some(PublishDiagnosticsClientCapabilities {
				version_support: Some(true),
				tag_support: Some(TagSupport {
					value_set: vec![
						lsp_types::DiagnosticTag::UNNECESSARY,
						lsp_types::DiagnosticTag::DEPRECATED,
					],
				}),
				..Default::default()
			}),
			inlay_hint: Some(lsp_types::InlayHintClientCapabilities {
				dynamic_registration: Some(false),
				resolve_support: None,
			}),
			..Default::default()
		}),
		window: Some(WindowClientCapabilities {
			work_done_progress: Some(true),
			show_document: Some(lsp_types::ShowDocumentClientCapabilities { support: true }),
			..Default::default()
		}),
		general: Some(GeneralClientCapabilities {
			position_encodings: Some(vec![
				PositionEncodingKind::UTF8,
				PositionEncodingKind::UTF32,
				PositionEncodingKind::UTF16,
			]),
			..Default::default()
		}),
		..Default::default()
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn test_offset_encoding_from_lsp() {
		assert_eq!(
			OffsetEncoding::from_lsp(&PositionEncodingKind::UTF8),
			Some(OffsetEncoding::Utf8)
		);
		assert_eq!(
			OffsetEncoding::from_lsp(&PositionEncodingKind::UTF16),
			Some(OffsetEncoding::Utf16)
		);
		assert_eq!(
			OffsetEncoding::from_lsp(&PositionEncodingKind::UTF32),
			Some(OffsetEncoding::Utf32)
		);
	}

	#[test]
	fn test_server_config_builder() {
		let config = ServerConfig::new("rust-analyzer", "/home/user/project")
			.args(["--log-file", "/tmp/ra.log"])
			.timeout(60)
			.config(serde_json::json!({"checkOnSave": true}));

		assert_eq!(config.command, "rust-analyzer");
		assert_eq!(config.args, vec!["--log-file", "/tmp/ra.log"]);
		assert_eq!(config.timeout_secs, 60);
		assert!(config.config.is_some());
	}
}
