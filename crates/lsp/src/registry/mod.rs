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
use tokio::sync::{Mutex, watch};
use tracing::{info, warn};

use crate::Result;
use crate::client::transport::LspTransport;
use crate::client::{ClientHandle, LanguageServerId, LspSlotId, ServerConfig};

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
	/// The instance identifier (slot + generation).
	#[allow(dead_code)]
	id: LanguageServerId,
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
/// The core indices (`servers`, `server_meta`, `id_index`) MUST be updated atomically
/// to maintain consistency across server lifecycle operations (start, stop, crash recovery).
/// Slot/generation tracking (`slot_ids`, `slot_gens`, `next_slot_id`) provides stable,
/// generation-aware [`LanguageServerId`] values so the event router can detect and
/// discard events from stale server instances.
struct RegistryState {
	/// Active server instances keyed by `(language, root_path)`.
	servers: HashMap<(String, PathBuf), ServerInstance>,
	/// Server metadata for answering server-initiated requests.
	server_meta: HashMap<LanguageServerId, ServerMeta>,
	/// Reverse index for O(1) removal by server ID.
	id_index: HashMap<LanguageServerId, (String, PathBuf)>,
	/// Mapping from slot to a unique ID used in [`LanguageServerId`].
	slot_ids: HashMap<(String, PathBuf), LspSlotId>,
	/// Generation counter per slot.
	slot_gens: HashMap<(String, PathBuf), u32>,
	/// Next available slot ID.
	next_slot_id: u32,
}

impl RegistryState {
	fn new() -> Self {
		Self {
			servers: HashMap::new(),
			server_meta: HashMap::new(),
			id_index: HashMap::new(),
			slot_ids: HashMap::new(),
			slot_gens: HashMap::new(),
			next_slot_id: 0,
		}
	}

	/// Returns the slot ID for a given key, creating one if it doesn't exist.
	fn get_or_create_slot_id(&mut self, key: &(String, PathBuf)) -> LspSlotId {
		if let Some(&id) = self.slot_ids.get(key) {
			id
		} else {
			let id = LspSlotId(self.next_slot_id);
			self.next_slot_id += 1;
			self.slot_ids.insert(key.clone(), id);
			id
		}
	}

	/// Increments and returns the next generation for a slot.
	fn next_gen(&mut self, key: &(String, PathBuf)) -> u32 {
		let generation = self.slot_gens.get(key).copied().unwrap_or(0) + 1;
		self.slot_gens.insert(key.clone(), generation);
		generation
	}
}

/// Tracking state for a server startup in progress.
struct InFlightStart {
	tx: watch::Sender<Option<Arc<Result<ClientHandle>>>>,
	rx: watch::Receiver<Option<Arc<Result<ClientHandle>>>>,
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
	inflight: Arc<Mutex<HashMap<(String, PathBuf), Arc<InFlightStart>>>>,
}

impl Registry {
	/// Create a new registry with the given transport.
	pub fn new(transport: Arc<dyn LspTransport>) -> Self {
		Self {
			configs: RwLock::new(HashMap::new()),
			state: RwLock::new(RegistryState::new()),
			transport,
			inflight: Arc::new(Mutex::new(HashMap::new())),
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

	/// Synchronous check for a running server.
	fn get_running(&self, key: &(String, PathBuf)) -> Option<ClientHandle> {
		let state = self.state.read();
		state.servers.get(key).map(|i| i.handle.clone())
	}

	/// Get or start a language server for a file path.
	///
	/// Returns an existing server handle if one is running for the resolved `(language, root_path)` key,
	/// otherwise starts a new server using singleflight pattern to prevent duplicate starts.
	///
	/// # Singleflight Protocol
	///
	/// 1. Fast path: check if server already running
	/// 2. Leader election: first caller becomes leader, others become waiters
	/// 3. Leader work:
	///    - Re-check if server was started by a previous leader
	///    - Call `transport.start()`, inserts into state
	///    - Populate shared result via `watch` channel
	///    - Remove inflight entry and notify waiters
	/// 4. Waiters wait on `watch` channel and receive result directly
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

		// 1. Fast path
		if let Some(handle) = self.get_running(&key) {
			return Ok(handle);
		}

		// 2. Leader election
		let (inflight, is_leader) = {
			let mut inflight_map = self.inflight.lock().await;
			if let Some(f) = inflight_map.get(&key) {
				(f.clone(), false)
			} else {
				let (tx, rx) = watch::channel(None);
				let f = Arc::new(InFlightStart { tx, rx });
				inflight_map.insert(key.clone(), f.clone());
				(f, true)
			}
		};

		if !is_leader {
			// 3a. Wait for leader
			let mut rx = inflight.rx.clone();
			loop {
				let result = {
					let borrow = rx.borrow();
					borrow.as_ref().cloned()
				};

				if let Some(res) = result {
					return (*res).clone();
				}

				if rx.changed().await.is_err() {
					return Err(crate::Error::Protocol(
						"Leader dropped without result".into(),
					));
				}
			}
		}

		// 3b. Leader work
		let mut guard = StartGuard::new(
			key.clone(),
			self.inflight.clone(),
			inflight.clone(),
			self.transport.clone(),
		);

		// Re-check state after lock acquisition to prevent double-start
		if let Some(handle) = self.get_running(&key) {
			return guard.complete(Ok(handle));
		}

		let (slot_id, generation) = {
			let mut state = self.state.write();
			let slot_id = state.get_or_create_slot_id(&key);
			let generation = state.next_gen(&key);
			(slot_id, generation)
		};
		let instance_id = LanguageServerId {
			slot: slot_id,
			generation,
		};

		info!(language = %language, command = %config.command, root = ?root_path, %instance_id, "Starting language server");

		let server_config = ServerConfig::new(instance_id, &config.command, &root_path)
			.args(config.args.iter().cloned())
			.env(config.env.iter().map(|(k, v)| (k.clone(), v.clone())))
			.timeout(config.timeout_secs);

		let started_res = self.transport.start(server_config).await;

		let final_res = match started_res {
			Ok(started) => {
				guard.note_started(started.id);
				let handle = {
					let mut state = self.state.write();
					// Final pathological race check
					if let Some(existing) = state.servers.get(&key) {
						existing.handle.clone()
					} else {
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
								id: started.id,
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
				Ok(handle)
			}
			Err(e) => Err(e),
		};

		guard.complete(final_res)
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
	pub fn shutdown_all(&self) -> Vec<LanguageServerId> {
		let mut state = self.state.write();
		let ids: Vec<LanguageServerId> = state.id_index.keys().copied().collect();
		state.servers.clear();
		state.server_meta.clear();
		state.id_index.clear();
		state.slot_ids.clear();
		state.slot_gens.clear();
		ids
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

	/// Returns true if the given instance ID is currently active in the registry.
	pub fn is_current(&self, id: LanguageServerId) -> bool {
		self.state.read().id_index.contains_key(&id)
	}

	/// Retrieve metadata for a server by its ID.
	///
	/// Returns `None` if the server has not been started or has been shut down.
	pub fn get_server_meta(&self, server_id: LanguageServerId) -> Option<ServerMeta> {
		self.state.read().server_meta.get(&server_id).cloned()
	}
}

/// Guard that un-wedges the inflight start map on drop if the leader fails or is cancelled.
struct StartGuard {
	key: (String, PathBuf),
	inflight_map: Arc<Mutex<HashMap<(String, PathBuf), Arc<InFlightStart>>>>,
	inflight: Arc<InFlightStart>,
	transport: Arc<dyn LspTransport>,
	started_id: Option<LanguageServerId>,
	completed: bool,
}

impl StartGuard {
	fn new(
		key: (String, PathBuf),
		inflight_map: Arc<Mutex<HashMap<(String, PathBuf), Arc<InFlightStart>>>>,
		inflight: Arc<InFlightStart>,
		transport: Arc<dyn LspTransport>,
	) -> Self {
		Self {
			key,
			inflight_map,
			inflight,
			transport,
			started_id: None,
			completed: false,
		}
	}

	fn note_started(&mut self, id: LanguageServerId) {
		self.started_id = Some(id);
	}

	fn complete(mut self, res: Result<ClientHandle>) -> Result<ClientHandle> {
		self.completed = true;

		// 1) publish result to waiters (sync, no await points)
		let _ = self.inflight.tx.send(Some(Arc::new(res.clone())));

		// 2) remove inflight entry asynchronously (so cancellation after this point can't wedge)
		let key = self.key.clone();
		let inflight_map = Arc::clone(&self.inflight_map);
		tokio::spawn(async move {
			let mut map = inflight_map.lock().await;
			map.remove(&key);
		});

		res
	}
}

impl Drop for StartGuard {
	fn drop(&mut self) {
		if self.completed {
			return;
		}

		// Leader exited early: unblock waiters + un-wedge inflight.
		let key = self.key.clone();
		let inflight_map = Arc::clone(&self.inflight_map);
		let tx = self.inflight.tx.clone();
		let transport = Arc::clone(&self.transport);
		let started_id = self.started_id;

		tokio::spawn(async move {
			// If we already spawned a server but never registered it, try to stop it.
			if let Some(id) = started_id {
				let _ = transport.stop(id).await;
			}

			// Remove inflight entry to allow subsequent retry.
			{
				let mut map = inflight_map.lock().await;
				map.remove(&key);
			}

			// Publish a deterministic error so waiters don't hang.
			let _ = tx.send(Some(Arc::new(Err(crate::Error::Protocol(
				"LSP start aborted (leader cancelled)".into(),
			)))));
		});
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

#[cfg(test)]
mod tests;
