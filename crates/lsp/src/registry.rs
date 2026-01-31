//! Language server registry.
//!
//! Manages the lifecycle of language server instances, mapping file types
//! to their corresponding servers.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tokio::sync::{Mutex, Notify};
use tracing::{info, warn};

use crate::Result;
use crate::client::transport::LspTransport;
use crate::client::{ClientHandle, LanguageServerId, ServerConfig};

/// Configuration for a language server.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LanguageServerConfig {
	/// Command to run the language server.
	pub command: String,
	/// Arguments to pass to the command.
	#[serde(default)]
	pub args: Vec<String>,
	/// Environment variables to set.
	#[serde(default)]
	pub env: HashMap<String, String>,
	/// Files/directories that mark the project root.
	/// The registry walks up from the file path to find these markers.
	#[serde(default)]
	pub root_markers: Vec<String>,
	/// Request timeout in seconds.
	#[serde(default = "default_timeout")]
	pub timeout_secs: u64,
	/// Server-specific initialization options.
	#[serde(default)]
	pub config: Option<Value>,
	/// Enable snippet support in completions.
	#[serde(default)]
	pub enable_snippets: bool,
}

/// Returns the default LSP request timeout in seconds.
fn default_timeout() -> u64 {
	30
}

impl Default for LanguageServerConfig {
	fn default() -> Self {
		Self {
			command: String::new(),
			args: Vec::new(),
			env: HashMap::new(),
			root_markers: Vec::new(),
			timeout_secs: default_timeout(),
			config: None,
			enable_snippets: true,
		}
	}
}

/// A running language server instance.
struct ServerInstance {
	/// Handle for communicating with the server.
	handle: ClientHandle,
}

/// Server metadata for handling server-initiated requests.
///
/// Captured during server startup and used to answer LSP requests like
/// `workspace/configuration` and `workspace/workspaceFolders` without
/// requiring additional context lookups.
#[derive(Debug, Clone)]
pub struct ServerMeta {
	/// Language identifier.
	pub language: String,
	/// Workspace root path resolved during server startup.
	pub root_path: PathBuf,
	/// Server-specific initialization options from [`LanguageServerConfig::config`].
	pub settings: Option<Value>,
}

/// Consolidated server state under a single lock for atomic operations.
///
/// All three indices must be updated atomically to maintain consistency across
/// server lifecycle operations (start, stop, crash recovery).
struct RegistryState {
	/// Active server instances keyed by `(language, root_path)`.
	servers: HashMap<(String, PathBuf), ServerInstance>,
	/// Server metadata for answering server-initiated requests.
	server_meta: HashMap<LanguageServerId, ServerMeta>,
	/// Reverse index for O(1) removal by server ID.
	id_index: HashMap<LanguageServerId, (String, PathBuf)>,
}

impl RegistryState {
	fn new() -> Self {
		Self {
			servers: HashMap::new(),
			server_meta: HashMap::new(),
			id_index: HashMap::new(),
		}
	}
}

/// Registry for managing language servers.
///
/// Thread-safe registry that ensures exactly one server instance per `(language, root_path)` key.
/// Uses singleflight pattern to prevent duplicate `transport.start()` calls under concurrent access.
///
/// # Concurrency
///
/// - `configs`: Protected by `RwLock` for read-heavy access to language server configurations
/// - `state`: Consolidated `RwLock` ensures atomic updates across all three server indices
/// - `inflight`: Async `Mutex` gate ensures only one transport start per key across all callers
pub struct Registry {
	configs: RwLock<HashMap<String, LanguageServerConfig>>,
	state: RwLock<RegistryState>,
	transport: Arc<dyn LspTransport>,
	inflight: Mutex<HashMap<(String, PathBuf), Arc<Notify>>>,
}

impl Registry {
	/// Create a new registry with the given transport.
	pub fn new(transport: Arc<dyn LspTransport>) -> Self {
		Self {
			configs: RwLock::new(HashMap::new()),
			state: RwLock::new(RegistryState::new()),
			transport,
			inflight: Mutex::new(HashMap::new()),
		}
	}

	/// Register a language server configuration for a language.
	pub fn register(&self, language: impl Into<String>, config: LanguageServerConfig) {
		let language = language.into();
		self.configs.write().insert(language, config);
	}

	/// Remove a language server configuration.
	pub fn unregister(&self, language: &str) {
		self.configs.write().remove(language);
	}

	/// Get the configuration for a language.
	pub fn get_config(&self, language: &str) -> Option<LanguageServerConfig> {
		self.configs.read().get(language).cloned()
	}

	/// List all registered languages.
	pub fn languages(&self) -> Vec<String> {
		self.configs.read().keys().cloned().collect()
	}

	/// Get or start a language server for a file path.
	///
	/// Returns an existing server handle if one is running for the resolved `(language, root_path)` key,
	/// otherwise starts a new server using singleflight pattern to prevent duplicate starts.
	///
	/// # Singleflight Protocol
	///
	/// 1. Fast path: check if server already running (read lock)
	/// 2. Leader election: first caller becomes leader, others become waiters
	/// 3. Leader calls `transport.start()`, then notifies waiters
	/// 4. Waiters retry from step 1 after notification
	/// 5. Atomic insertion under write lock handles pathological races
	///
	/// # Errors
	///
	/// Returns error if no configuration exists for the language or if transport start fails.
	pub async fn get_or_start(&self, language: &str, file_path: &Path) -> Result<ClientHandle> {
		let config = self.get_config(language).ok_or_else(|| {
			crate::Error::Protocol(format!("No server configured for {language}"))
		})?;

		let root_path = find_root_path(file_path, &config.root_markers);
		let key = (language.to_string(), root_path.clone());

		loop {
			if let Some(instance) = self.state.read().servers.get(&key) {
				return Ok(instance.handle.clone());
			}

			let (notify, is_leader) = {
				let mut inflight = self.inflight.lock().await;
				if let Some(n) = inflight.get(&key) {
					(n.clone(), false)
				} else {
					let n = Arc::new(Notify::new());
					inflight.insert(key.clone(), n.clone());
					(n, true)
				}
			};

			if !is_leader {
				notify.notified().await;
				continue;
			}

			info!(language = %language, command = %config.command, root = ?root_path, "Starting language server");

			let server_config = ServerConfig::new(&config.command, &root_path)
				.args(config.args.iter().cloned())
				.env(config.env.iter().map(|(k, v)| (k.clone(), v.clone())))
				.timeout(config.timeout_secs);

			tracing::trace!(
				language = %language,
				root = ?root_path,
				key = ?key,
				"Registry: before transport.start (leader)"
			);

			let started = self.transport.start(server_config).await;

			{
				let mut inflight = self.inflight.lock().await;
				inflight.remove(&key);
				notify.notify_waiters();
			}

			let started = started?;

			tracing::trace!(
				language = %language,
				root = ?root_path,
				server_id = started.id.0,
				"Registry: after transport.start (leader)"
			);

			let handle = {
				let mut state = self.state.write();
				if let Some(existing) = state.servers.get(&key) {
					tracing::trace!(
						language = %language,
						root = ?root_path,
						server_id = existing.handle.id().0,
						"Registry: lost pathological race, using existing handle"
					);
					existing.handle.clone()
				} else {
					tracing::trace!(
						language = %language,
						root = ?root_path,
						server_id = started.id.0,
						"Registry: inserting server and spawning init"
					);

					let handle = ClientHandle::new(
						started.id,
						config.command.clone(),
						root_path.clone(),
						self.transport.clone(),
					);

					state.server_meta.insert(
						started.id,
						ServerMeta {
							language: language.to_string(),
							root_path: root_path.clone(),
							settings: config.config.clone(),
						},
					);
					state
						.id_index
						.insert(started.id, (language.to_string(), root_path.clone()));
					state.servers.insert(
						key.clone(),
						ServerInstance {
							handle: handle.clone(),
						},
					);

					let init_handle = handle.clone();
					let enable_snippets = config.enable_snippets;
					let init_config = config.config.clone();

					tokio::spawn(async move {
						match tokio::time::timeout(
							Duration::from_secs(30),
							init_handle.initialize(enable_snippets, init_config),
						)
						.await
						{
							Ok(Ok(_)) => {}
							Ok(Err(e)) => {
								warn!(error = %e, "LSP initialize failed");
							}
							Err(_) => {
								warn!("LSP initialize timed out");
							}
						}
					});

					handle
				}
			};

			return Ok(handle);
		}
	}

	/// Get an active client for a language and file path, if one exists and is alive.
	pub fn get(&self, language: &str, file_path: &Path) -> Option<ClientHandle> {
		let config = self.get_config(language)?;
		let root_path = find_root_path(file_path, &config.root_markers);
		let key = (language.to_string(), root_path);

		let state = self.state.read();
		let instance = state.servers.get(&key)?;
		Some(instance.handle.clone())
	}

	/// Remove a server by its ID.
	///
	/// Atomically removes server from all three indices and returns its metadata.
	/// Typically called when a server crashes or stops to clean up registry state.
	pub fn remove_server(&self, server_id: LanguageServerId) -> Option<ServerMeta> {
		let mut state = self.state.write();
		let key = state.id_index.remove(&server_id)?;
		state.servers.remove(&key);
		state.server_meta.remove(&server_id)
	}

	/// Shutdown all servers.
	pub async fn shutdown_all(&self) {
		let mut state = self.state.write();
		state.servers.clear();
		state.server_meta.clear();
		state.id_index.clear();
	}

	/// Get the number of active servers.
	pub fn active_count(&self) -> usize {
		self.state.read().servers.len()
	}

	/// Get the underlying transport.
	pub fn transport(&self) -> Arc<dyn LspTransport> {
		self.transport.clone()
	}

	/// Check if any server is ready (initialized and accepting requests).
	pub fn any_server_ready(&self) -> bool {
		self.state
			.read()
			.servers
			.values()
			.any(|instance| instance.handle.is_ready())
	}

	/// Retrieve metadata for a server by its ID.
	///
	/// Returns `None` if the server has not been started or has been shut down.
	pub fn get_server_meta(&self, server_id: LanguageServerId) -> Option<ServerMeta> {
		self.state.read().server_meta.get(&server_id).cloned()
	}
}

/// Find the project root by walking up from the file path.
fn find_root_path(file_path: &Path, root_markers: &[String]) -> PathBuf {
	let abs_path = file_path
		.canonicalize()
		.unwrap_or_else(|_| std::env::current_dir().unwrap_or_default().join(file_path));

	let start_dir = if abs_path.is_file() {
		abs_path.parent().unwrap_or(&abs_path).to_path_buf()
	} else {
		abs_path.clone()
	};

	let mut current = start_dir.as_path();
	loop {
		for marker in root_markers {
			if current.join(marker).exists() {
				return current.to_path_buf();
			}
		}

		match current.parent() {
			Some(parent) => current = parent,
			None => break,
		}
	}

	start_dir
}
