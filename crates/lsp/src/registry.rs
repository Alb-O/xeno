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

/// Registry for managing language servers.
///
/// Thread-safe; can be shared across async tasks via `Arc<Registry>`.
pub struct Registry {
	/// Configurations by language name.
	configs: RwLock<HashMap<String, LanguageServerConfig>>,
	/// Active server instances by (language, root_path).
	servers: RwLock<HashMap<(String, PathBuf), ServerInstance>>,
	/// Underlying transport.
	transport: Arc<dyn LspTransport>,
	/// Server metadata indexed by server ID for answering server-initiated requests.
	server_meta: RwLock<HashMap<LanguageServerId, ServerMeta>>,
}

impl Registry {
	/// Create a new registry with the given transport.
	pub fn new(transport: Arc<dyn LspTransport>) -> Self {
		Self {
			configs: RwLock::new(HashMap::new()),
			servers: RwLock::new(HashMap::new()),
			transport,
			server_meta: RwLock::new(HashMap::new()),
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
	pub async fn get_or_start(&self, language: &str, file_path: &Path) -> Result<ClientHandle> {
		let config = self.get_config(language).ok_or_else(|| {
			crate::Error::Protocol(format!("No server configured for {language}"))
		})?;

		let root_path = find_root_path(file_path, &config.root_markers);
		let key = (language.to_string(), root_path.clone());

		{
			let servers = self.servers.read();
			if let Some(instance) = servers.get(&key) {
				return Ok(instance.handle.clone());
			}
		}

		info!(language = %language, command = %config.command, root = ?root_path, "Starting language server");

		let server_config = ServerConfig::new(&config.command, &root_path)
			.args(config.args.iter().cloned())
			.env(config.env.iter().map(|(k, v)| (k.clone(), v.clone())))
			.timeout(config.timeout_secs);

		let started = self.transport.start(server_config).await?;

		self.server_meta.write().insert(
			started.id,
			ServerMeta {
				language: language.to_string(),
				root_path: root_path.clone(),
				settings: config.config.clone(),
			},
		);

		let handle = ClientHandle::new(
			started.id,
			config.command.clone(),
			root_path,
			self.transport.clone(),
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

		self.servers.write().insert(
			key,
			ServerInstance {
				handle: handle.clone(),
			},
		);

		Ok(handle)
	}

	/// Get an active client for a language and file path, if one exists and is alive.
	pub fn get(&self, language: &str, file_path: &Path) -> Option<ClientHandle> {
		let config = self.get_config(language)?;
		let root_path = find_root_path(file_path, &config.root_markers);
		let key = (language.to_string(), root_path);

		let servers = self.servers.read();
		let instance = servers.get(&key)?;
		Some(instance.handle.clone())
	}

	/// Shutdown all servers.
	pub async fn shutdown_all(&self) {
		self.servers.write().clear();
		self.server_meta.write().clear();
	}

	/// Get the number of active servers.
	pub fn active_count(&self) -> usize {
		self.servers.read().len()
	}

	/// Get the underlying transport.
	pub fn transport(&self) -> Arc<dyn LspTransport> {
		self.transport.clone()
	}

	/// Check if any server is ready (initialized and accepting requests).
	pub fn any_server_ready(&self) -> bool {
		self.servers
			.read()
			.values()
			.any(|instance| instance.handle.is_ready())
	}

	/// Retrieve metadata for a server by its ID.
	///
	/// Returns `None` if the server has not been started or has been shut down.
	pub fn get_server_meta(&self, server_id: LanguageServerId) -> Option<ServerMeta> {
		self.server_meta.read().get(&server_id).cloned()
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
