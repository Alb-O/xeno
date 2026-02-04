use std::collections::{HashMap, HashSet};

use tokio::sync::oneshot;
use xeno_broker_proto::types::SessionId;

use super::lsp_doc::LspDocState;

/// Metadata for a managed LSP server instance.
pub struct ServerEntry {
	/// Communication handle and process monitor.
	pub instance: crate::core::LspInstance,
	/// Identity of the project (command/args/cwd).
	pub project: crate::core::ProjectKey,
	/// Set of editor sessions currently participating.
	pub attached: HashSet<SessionId>,
	/// Session ID elected to handle server-to-client requests.
	pub leader: SessionId,
	/// Tracker for document versions on this server.
	pub docs: crate::core::text_sync::DocRegistry,
	/// Broker-owned LSP document state keyed by URI.
	pub lsp_docs: HashMap<String, LspDocState>,
	/// Token for invalidating stale lease timers.
	pub lease_gen: u64,
	/// Ownership tracker for text sync gating.
	pub doc_owners: crate::core::text_sync::DocOwnerRegistry,
	/// Monotonic sequence for broker-originated request IDs.
	pub next_wire_req_id: u64,
}

#[derive(Debug)]
pub struct PendingS2cReq {
	pub responder: SessionId,
	pub tx: oneshot::Sender<crate::core::LspReplyResult>,
}

#[derive(Debug)]
pub struct PendingC2sReq {
	pub origin_session: SessionId,
	pub origin_id: xeno_lsp::RequestId,
}
