//! Language server registry.
//!
//! Manages the lifecycle of language server instances, mapping file types
//! to their corresponding servers.
//!
//! # Overview
//!
//! The registry maintains:
//! - A map of server configurations by language/file type
//! - Active server instances
//! - Pending initialization state
//!
//! # Example
//!
//! ```ignore
//! use xeno_lsp::registry::{Registry, LanguageServerConfig};
//!
//! let mut registry = Registry::new();
//!
//! // Register rust-analyzer for Rust files
//! registry.register("rust", LanguageServerConfig {
//!     command: "rust-analyzer".into(),
//!     args: vec![],
//!     root_markers: vec!["Cargo.toml".into()],
//!     ..Default::default()
//! });
//!
//! // Get or start a server for a Rust file
//! let client = registry.get_or_start("rust", "/path/to/project").await?;
//! ```

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tokio::task::JoinHandle;
use tracing::{info, warn};

use crate::Result;
use crate::client::{
	ClientHandle, LanguageServerId, ServerConfig, SharedEventHandler, start_server,
};

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
	/// Task running the main loop.
	task: JoinHandle<Result<()>>,
}

impl ServerInstance {
	/// Check if the server task is still running.
	fn is_alive(&self) -> bool {
		!self.task.is_finished()
	}
}

/// Registry for managing language servers.
///
/// Thread-safe: can be shared across async tasks using `Arc<Registry>`.
pub struct Registry {
	/// Configurations by language name.
	configs: RwLock<HashMap<String, LanguageServerConfig>>,
	/// Active server instances by (language, root_path).
	servers: RwLock<HashMap<(String, PathBuf), ServerInstance>>,
	/// Counter for generating unique server IDs.
	next_id: AtomicU64,
	/// Event handler for LSP events (diagnostics, progress, etc.).
	event_handler: Option<SharedEventHandler>,
}

impl Default for Registry {
	fn default() -> Self {
		Self::new()
	}
}

impl Registry {
	/// Create a new empty registry.
	pub fn new() -> Self {
		Self {
			configs: RwLock::new(HashMap::new()),
			servers: RwLock::new(HashMap::new()),
			next_id: AtomicU64::new(1),
			event_handler: None,
		}
	}

	/// Create a new registry with an event handler for LSP events.
	///
	/// The event handler receives notifications from all language servers
	/// managed by this registry (diagnostics, progress, messages, etc.).
	pub fn with_event_handler(event_handler: SharedEventHandler) -> Self {
		Self {
			configs: RwLock::new(HashMap::new()),
			servers: RwLock::new(HashMap::new()),
			next_id: AtomicU64::new(1),
			event_handler: Some(event_handler),
		}
	}

	/// Set the event handler for LSP events.
	///
	/// This only affects servers started after this call.
	pub fn set_event_handler(&mut self, handler: SharedEventHandler) {
		self.event_handler = Some(handler);
	}

	/// Register a language server configuration for a language.
	pub fn register(&self, language: impl Into<String>, config: LanguageServerConfig) {
		let language = language.into();
		info!(
			language = %language,
			command = %config.command,
			root_markers = ?config.root_markers,
			"Registry::register: configured language server"
		);
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

	/// Get an active client for a language and file path, starting one if needed.
	///
	/// This finds the project root based on the configured root markers,
	/// then returns an existing server for that root or starts a new one.
	/// If an existing server has crashed, it will be cleaned up and restarted.
	pub async fn get_or_start(&self, language: &str, file_path: &Path) -> Result<ClientHandle> {
		let config = self.get_config(language).ok_or_else(|| {
			crate::Error::Protocol(format!("No server configured for {language}"))
		})?;

		let root_path = find_root_path(file_path, &config.root_markers);
		let key = (language.to_string(), root_path.clone());

		// Check for existing server, clean up if dead
		{
			let servers = self.servers.read();
			if let Some(instance) = servers.get(&key) {
				if instance.is_alive() {
					return Ok(instance.handle.clone());
				}
				warn!(language = %language, root = ?root_path, "Language server crashed, restarting");
			}
		}

		// Remove dead server if present
		self.servers.write().remove(&key);

		let id = LanguageServerId(self.next_id.fetch_add(1, Ordering::Relaxed));
		info!(
			language = %language,
			command = %config.command,
			root = ?root_path,
			"Starting language server"
		);

		let server_config = ServerConfig::new(&config.command, &root_path)
			.args(config.args.iter().cloned())
			.env(config.env.iter().map(|(k, v)| (k.clone(), v.clone())))
			.timeout(config.timeout_secs);

		let (handle, task) = start_server(
			id,
			config.command.clone(),
			server_config,
			self.event_handler.clone(),
		)?;

		handle
			.initialize(config.enable_snippets, config.config.clone())
			.await?;

		self.servers.write().insert(
			key,
			ServerInstance {
				handle: handle.clone(),
				task,
			},
		);

		Ok(handle)
	}

	/// Get an active client for a language and file path, if one exists and is alive.
	///
	/// Finds the project root from the file path using configured root markers,
	/// then looks up the server for that root.
	///
	/// Returns `None` if no server exists, no config exists, or if the server has crashed.
	/// Dead servers are cleaned up lazily on next `get_or_start` call.
	pub fn get(&self, language: &str, file_path: &Path) -> Option<ClientHandle> {
		let config = self.get_config(language)?;
		let root_path = find_root_path(file_path, &config.root_markers);
		let key = (language.to_string(), root_path);

		let servers = self.servers.read();
		let instance = servers.get(&key)?;
		instance.is_alive().then(|| instance.handle.clone())
	}

	/// Clean up all dead servers and return the number of servers removed.
	pub fn cleanup_dead_servers(&self) -> usize {
		let dead_keys: Vec<_> = self
			.servers
			.read()
			.iter()
			.filter(|(_, instance)| !instance.is_alive())
			.map(|(key, _)| key.clone())
			.collect();

		let count = dead_keys.len();
		if count > 0 {
			let mut servers = self.servers.write();
			for key in dead_keys {
				info!(
					language = %key.0,
					root = ?key.1,
					"Cleaning up dead language server"
				);
				servers.remove(&key);
			}
		}
		count
	}

	/// Get all active clients for a language.
	pub fn get_all(&self, language: &str) -> Vec<ClientHandle> {
		self.servers
			.read()
			.iter()
			.filter(|(k, _)| k.0 == language)
			.map(|(_, s)| s.handle.clone())
			.collect()
	}

	/// Shutdown a specific server.
	pub async fn shutdown(&self, language: &str, root_path: &Path) -> Result<()> {
		let key = (language.to_string(), root_path.to_path_buf());
		let instance = self.servers.write().remove(&key);
		if let Some(instance) = instance {
			instance.handle.shutdown_and_exit().await?;
		}
		Ok(())
	}

	/// Shutdown all servers.
	pub async fn shutdown_all(&self) {
		let instances: Vec<_> = self.servers.write().drain().collect();
		for (_, instance) in instances {
			let _ = instance.handle.shutdown_and_exit().await;
		}
	}

	/// Get the number of active servers.
	pub fn active_count(&self) -> usize {
		self.servers.read().len()
	}
}

/// Find the project root by walking up from the file path.
///
/// Looks for any of the root markers. If none found, returns the file's directory.
/// Always returns an absolute path (required for LSP URIs).
fn find_root_path(file_path: &Path, root_markers: &[String]) -> PathBuf {
	// Canonicalize to get absolute path, fall back to current dir + path if that fails
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

	// No marker found, use the file's directory
	start_dir
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn test_find_root_path_with_marker() {
		// Create a temp directory structure
		let temp = tempfile::tempdir().unwrap();
		let root = temp.path();

		std::fs::write(root.join("Cargo.toml"), "").unwrap();
		let nested = root.join("src").join("nested");
		std::fs::create_dir_all(&nested).unwrap();

		let found = find_root_path(&nested, &["Cargo.toml".into()]);
		assert_eq!(found, root);
	}

	#[test]
	fn test_find_root_path_no_marker() {
		let temp = tempfile::tempdir().unwrap();
		let dir = temp.path();

		let found = find_root_path(dir, &["nonexistent.marker".into()]);
		assert_eq!(found, dir);
	}

	#[test]
	fn test_registry_config() {
		let registry = Registry::new();

		registry.register(
			"rust",
			LanguageServerConfig {
				command: "rust-analyzer".into(),
				root_markers: vec!["Cargo.toml".into()],
				..Default::default()
			},
		);

		assert!(registry.get_config("rust").is_some());
		assert!(registry.get_config("python").is_none());

		let languages = registry.languages();
		assert_eq!(languages, vec!["rust"]);

		registry.unregister("rust");
		assert!(registry.get_config("rust").is_none());
	}
}
