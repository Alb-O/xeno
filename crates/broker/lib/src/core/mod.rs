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
//! | [`SharedStateService`] | Authoritative owner of document text and shared-state protocol. |
//! | [`KnowledgeService`] | Provider of workspace search and background indexing. |
//!
//! # Invariants
//!
//! - Preferred Owner Writes: Only the preferred owner of a document URI may submit deltas to the broker.
//!   - Enforced in: `SharedStateService::handle_edit`
//!   - Tested by: `services::tests::test_shared_state_preferred_owner_enforcement`
//!   - Failure symptom: `NotPreferredOwner` error returned to the editor session.
//!
//! - Focus Transfers Ownership: `SharedFocus` MUST set the preferred owner and owner to the
//!   focused session, bump the ownership epoch, and broadcast ownership changes.
//!   - Enforced in: `SharedStateService::handle_focus`
//!   - Tested by: `services::tests::test_shared_state_focus_transfers_ownership`
//!   - Failure symptom: Focused sessions remain read-only, leaving diagnostics tied to a previous owner.
//!
//! - Resync After Transfer: A new owner MUST resync before submitting deltas after a focus transfer.
//!   - Enforced in: `SharedStateService::handle_edit`
//!   - Tested by: `services::tests::test_shared_state_transfer_requires_resync_before_edit`
//!   - Failure symptom: Divergent edits are accepted without alignment, corrupting shared state.
//!
//! - Idle Ownership Release: Documents MUST transition to the unlocked state when the owner is
//!   inactive beyond the idle timeout.
//!   - Enforced in: `SharedStateService::handle_idle_tick`
//!   - Tested by: `services::tests::test_shared_state_idle_unlocks_owner`
//!   - Failure symptom: An inactive owner blocks other sessions from becoming writable.
//!
//! - Diagnostics Replay: The broker MUST cache the latest `publishDiagnostics` payload per
//!   document (even when no sessions are attached) and replay it to newly attached sessions.
//!   - Enforced in: `RoutingService::handle_server_notif`, `RoutingService::attach_session`
//!   - Tested by: `services::tests::test_routing_diagnostics_replay_on_attach`,
//!     `services::tests::test_routing_diagnostics_replay_after_session_lost`
//!   - Failure symptom: Newly attached or reconnected editors show no diagnostics until another publish event.
//!
//! - Broker-Owned LSP Docs: LSP `didOpen`/`didChange`/`didClose` notifications MUST be emitted
//!   from broker-owned shared state; session-originated text sync notifications MUST NOT be
//!   forwarded to servers.
//!   - Enforced in: `RoutingService::handle_session_text_sync`, `SharedStateService::handle_open`,
//!     `SharedStateService::handle_edit`, `SharedStateService::handle_close`
//!   - Tested by: `services::tests::test_routing_lsp_docs_from_sync`
//!   - Failure symptom: Diagnostics stall or diverge after the originating session disconnects.
//!
//! - Conditional Resync: When `SharedResync` provides `client_hash64` and `client_len_chars`
//!   matching broker state, the broker MUST respond with an empty snapshot payload.
//!   - Enforced in: `SharedStateService::handle_resync`
//!   - Tested by: `services::tests::test_shared_state_resync_matches_fingerprint_returns_empty`
//!   - Failure symptom: Editors clear syntax on no-op resyncs, causing highlight flicker after focus changes.
//!
//! - No-op Snapshot Apply: Editors MUST skip applying empty snapshot text when the local
//!   fingerprint matches the snapshot fingerprint.
//!   - Enforced in: `Editor::handle_shared_state_event`, `shared_state::should_apply_snapshot_text`
//!   - Tested by: `shared_state::tests::test_snapshot_apply_skips_matching_empty`
//!   - Failure symptom: Syntax trees are dropped on ownership transfer even when content is unchanged.
//!
//! - Session Loss LSP Close: When a session loss removes the final open reference for a document,
//!   routing MUST close broker-owned LSP doc state immediately.
//!   - Enforced in: `RoutingService::handle_session_lost`
//!   - Tested by: `services::tests::test_routing_session_lost_closes_lsp_docs`
//!   - Failure symptom: Reconnected sessions never trigger a fresh `didOpen`, leaving diagnostics stale.
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
//! 2. Service -> Service (MPSC): Services communicate via internal handles (e.g. [`SharedStateService`] signals [`KnowledgeService`]).
//! 3. SharedState -> Routing: Authoritative text updates drive broker-owned LSP `didOpen`/`didChange`/`didClose`.
//! 4. Broker -> LSP (Stdio): [`RoutingService`] transmits messages via [`LspProxyService`].
//! 5. LSP -> Broker (Stdio): Inbound messages received by [`LspProxyService`], routed back to [`RoutingService`].
//! 6. Broker -> Editor (IPC): [`SessionService`] transmits events or responses back to the connected socket.
//!
//! # Lifecycle
//!
//! - Startup: `BrokerRuntime::new` starts services in a tiered sequence to resolve cyclic handle dependencies.
//! - Session: `Subscribe` registers a sink in [`SessionService`]. Drop cleans up via `Unregister`.
//! - Server: `LspStart` triggers process spawn; idle lease timer manages termination when all sessions detach.
//! - Sync: `SharedStateService` updates preferred owners on focus, releases ownership on idle/blur/disconnect, and broadcasts unlocks before new owners resync.
//! - LSP Docs: Broker-owned LSP documents open on first SharedState open and close on final SharedState close.
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
//! - Changing sync logic: Modify [`SharedStateService`] and ensure the preferred-owner invariant is maintained.

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
