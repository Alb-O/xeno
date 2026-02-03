//! Broker subsystem architecture and shared types.
//!
//! # Purpose
//!
//! The Broker is a background daemon that coordinates multiple editor sessions,
//! manages LSP server lifecycles, and provides workspace-wide intelligence. It
//! acts as a central authority for document state and a router for LSP traffic.
//!
//! # Mental model
//!
//! The broker operates as a collection of isolated actor services orchestrated by
//! a single [`BrokerRuntime`]. Communications between services and external
//! editor sessions occur via asynchronous message passing (MPSC channels).
//!
//! # Key types
//!
//! | Type | Role |
//! | --- | --- |
//! | [`BrokerRuntime`] | Orchestrator that wires and owns service handles. |
//! | [`SessionService`] | Owner of IPC sinks; handles delivery and session loss detection. |
//! | [`RoutingService`] | Manager of LSP processes and server-to-client request routing. |
//! | [`BufferSyncService`] | Authoritative owner of document text and sync protocol. |
//! | [`KnowledgeService`] | Provider of workspace search and background indexing. |
//!
//! # Invariants
//!
//! - Single Writer: Only the elected owner of a document URI may submit deltas to the broker.
//!   - Enforced in: `BufferSyncService::handle_delta`
//!   - Tested by: `services::tests::test_sync_ownership_enforcement`
//!   - Failure symptom: `NotDocOwner` error returned to the editor session.
//!
//! - Up-for-Grabs Ownership: `TakeOwnership` MUST be denied when a document already has
//!   an owner; only unlocked documents may grant a new owner.
//!   - Enforced in: `BufferSyncService::handle_take_ownership`
//!   - Tested by: `services::tests::test_buffer_sync_take_ownership_denied_when_owner_active`
//!   - Failure symptom: Two sessions can both write, causing divergent document state.
//!
//! - Idle Ownership Release: Documents MUST transition to the unlocked state when the owner is
//!   inactive beyond the idle timeout.
//!   - Enforced in: `BufferSyncService::handle_idle_tick`
//!   - Tested by: `services::tests::test_buffer_sync_idle_unlocks_owner`
//!   - Failure symptom: An inactive owner blocks other sessions from becoming writable.
//!
//! - Atomic Request Registration: S2C requests MUST be registered in the pending map before being transmitted to the leader.
//!   - Enforced in: `RoutingService::handle_begin_s2c`
//!   - Tested by: `services::tests::test_s2c_registration_order`
//!   - Failure symptom: Responses from the leader may be dropped if they arrive before registration completes.
//!
//! - Deterministic Teardown: All pending requests for a server MUST be cancelled with `REQUEST_CANCELLED` when the server exits.
//!   - Enforced in: `RoutingService::handle_server_exit`
//!   - Tested by: `services::tests::test_server_exit_cancellation`
//!   - Failure symptom: Editor sessions or proxy services may hang waiting for a response that will never arrive.
//!
//! # Data flow
//!
//! 1. Editor -> Broker (IPC): Request received by [`BrokerService`], dispatched to relevant service handle.
//! 2. Service -> Service (MPSC): Services communicate via internal handles (e.g. [`BufferSyncService`] signals [`KnowledgeService`]).
//! 3. Broker -> LSP (Stdio): [`RoutingService`] transmits messages via [`LspProxyService`].
//! 4. LSP -> Broker (Stdio): Inbound messages received by [`LspProxyService`], routed back to [`RoutingService`].
//! 5. Broker -> Editor (IPC): [`SessionService`] transmits events or responses back to the connected socket.
//!
//! # Lifecycle
//!
//! - Startup: `BrokerRuntime::new` starts services in a tiered sequence to resolve cyclic handle dependencies.
//! - Session: `Subscribe` registers a sink in [`SessionService`]. Drop cleans up via `Unregister`.
//! - Server: `LspStart` triggers process spawn; idle lease timer manages termination when all sessions detach.
//! - Sync: `BufferSyncService` releases ownership on idle, explicit release, or disconnect, broadcasting unlocks before new owners confirm.
//! - Shutdown: `BrokerRuntime::shutdown` triggers `TerminateAll` in the routing service, killing all LSP processes.
//!
//! # Concurrency & ordering
//!
//! Services are single-threaded actors. Concurrency is achieved by running services in parallel tasks.
//! Ordering within a service is strictly FIFO based on channel arrival.
//! Cross-service ordering is eventually consistent; e.g., a "session lost" signal may reach routing before sync.
//!
//! # Failure modes & recovery
//!
//! - Send Failure: If [`SessionService`] fails to deliver to a sink, it triggers a `SessionLost` fan-out to all services for cleanup.
//! - Server Crash: Process monitor in launcher signals `ServerExited` to [`RoutingService`], which performs deterministic teardown.
//! - Deadlock: Prevented by ensuring cross-service cleanup signals (like `session_lost`) are always spawned as new tasks rather than awaited in-loop.
//!
//! # Recipes
//!
//! - Adding a new IPC request: Update `broker-proto`, then add handler in [`BrokerService::call`] and the target service.
//! - Changing sync logic: Modify [`BufferSyncService`] and ensure the single-writer invariant is maintained.

pub mod knowledge;
pub mod server;
pub mod text_sync;

use std::time::Duration;

use url::Url;
use xeno_broker_proto::types::{ErrorCode, IpcFrame, Request, Response};
use xeno_rpc::PeerSocket;

/// Sink for sending events to a connected session.
pub type SessionSink = PeerSocket<IpcFrame, Request, Response>;

/// Sink for sending messages to an LSP server.
pub type LspTx = PeerSocket<xeno_lsp::Message, xeno_lsp::AnyRequest, xeno_lsp::AnyResponse>;

/// Result of a server-to-editor request.
pub type LspReplyResult = Result<serde_json::Value, xeno_lsp::ResponseError>;

/// Configuration for the broker.
#[derive(Debug, Clone)]
pub struct BrokerConfig {
	/// Duration to keep an idle server alive after all sessions detach.
	pub idle_lease: Duration,
}

impl Default for BrokerConfig {
	fn default() -> Self {
		Self {
			idle_lease: Duration::from_secs(300),
		}
	}
}

pub use server::{ChildHandle, LspInstance, ServerControl};
pub use text_sync::{DocGateDecision, DocGateKind, DocGateResult};
use xeno_broker_proto::types::LspServerConfig;

/// Unique key for deduplicating LSP servers by project identity.
///
/// Servers are shared across editor sessions that match the same command,
/// arguments, and working directory.
#[derive(Debug, Clone, Hash, PartialEq, Eq)]
pub struct ProjectKey {
	/// Command used to start the server.
	pub command: String,
	/// Arguments passed to the command.
	pub args: Vec<String>,
	/// Project root directory.
	pub cwd: String,
}

impl From<&LspServerConfig> for ProjectKey {
	fn from(cfg: &LspServerConfig) -> Self {
		let cwd = cfg
			.cwd
			.clone()
			.unwrap_or_else(|| format!("__NO_CWD_{:016x}__", compute_config_hash(cfg)));
		Self {
			command: cfg.command.clone(),
			args: cfg.args.clone(),
			cwd,
		}
	}
}

/// Compute a stable hash of config for the no-cwd sentinel.
fn compute_config_hash(cfg: &LspServerConfig) -> u64 {
	use std::collections::hash_map::DefaultHasher;
	use std::hash::{Hash, Hasher};
	let mut hasher = DefaultHasher::new();
	cfg.command.hash(&mut hasher);
	cfg.args.hash(&mut hasher);
	hasher.finish()
}

/// Session entry tracking attached servers and the communication sink.
#[derive(Debug)]
pub struct SessionEntry {
	/// Outbound communication channel.
	pub sink: SessionSink,
	/// Set of servers this session is attached to.
	pub attached: std::collections::HashSet<xeno_broker_proto::types::ServerId>,
}

/// Normalizes a URI for consistent document tracking across services.
///
/// Removes fragments/queries and normalizes file paths (e.g. Windows drive casing).
///
/// # Errors
///
/// Returns `ErrorCode::InvalidArgs` if the URI is malformed.
pub fn normalize_uri(uri: &str) -> Result<String, ErrorCode> {
	let mut u = Url::parse(uri).map_err(|_| ErrorCode::InvalidArgs)?;

	u.set_fragment(None);
	u.set_query(None);

	if u.scheme() == "file"
		&& let Ok(path) = u.to_file_path()
	{
		#[cfg(windows)]
		let path = {
			let mut p = path;
			if let Some(std::path::Component::Prefix(prefix)) = p.components().next() {
				if let std::path::Prefix::Disk(drive) | std::path::Prefix::VerbatimDisk(drive) =
					prefix.kind()
				{
					let drive_str = (drive as char).to_lowercase().next().unwrap().to_string();
					let mut new_path = std::path::PathBuf::from(format!("{}:\\", drive_str));
					new_path.push(p.components().skip(1).collect::<std::path::PathBuf>());
					p = new_path;
				}
			}
			p
		};

		if let Ok(normalized) = Url::from_file_path(path) {
			u = normalized;
		}
	}

	Ok(u.to_string())
}
