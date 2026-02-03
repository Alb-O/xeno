//! Broker runtime orchestration.

use std::sync::Arc;
use std::time::Duration;

use crate::launcher::LspLauncher;
use crate::services::{buffer_sync, knowledge, routing, sessions};

/// Orchestrates the lifecycle and wiring of broker services.
///
/// This struct holds handles to all active services in the broker ecosystem.
/// Services operate as independent actors communicating via channels.
pub struct BrokerRuntime {
	/// Handle for session management and IPC delivery.
	pub sessions: sessions::SessionHandle,
	/// Handle for LSP server lifecycle and routing.
	pub routing: routing::RoutingHandle,
	/// Handle for document synchronization state.
	pub sync: buffer_sync::BufferSyncHandle,
	/// Handle for workspace intelligence and search.
	pub knowledge: knowledge::KnowledgeHandle,
}

impl BrokerRuntime {
	/// Initializes all services and establishes cross-service channel wiring.
	///
	/// Uses a tiered startup sequence to resolve cyclic dependencies between
	/// services (e.g., `SessionService` needs `RoutingHandle`, and vice versa).
	#[must_use]
	pub fn new(idle_lease: Duration, launcher: Arc<dyn LspLauncher>) -> Arc<Self> {
		let (sessions, sessions_routing_tx, sessions_sync_tx) = sessions::SessionService::start();
		let (sync, open_docs, sync_knowledge_tx, sync_routing_tx) =
			buffer_sync::BufferSyncService::start(sessions.clone());
		let knowledge = knowledge::KnowledgeService::start(sync.clone(), open_docs);

		let _ = sync_knowledge_tx.send(knowledge.clone());

		let routing = routing::RoutingService::start(
			sessions.clone(),
			knowledge.clone(),
			launcher,
			idle_lease,
		);

		let _ = sessions_routing_tx.send(routing.clone());
		let _ = sessions_sync_tx.send(sync.clone());
		let _ = sync_routing_tx.send(routing.clone());

		Arc::new(Self {
			sessions,
			routing,
			sync,
			knowledge,
		})
	}

	/// Triggers a graceful shutdown of all services and managed LSP processes.
	pub async fn shutdown(&self) {
		self.routing.terminate_all().await;
	}
}
