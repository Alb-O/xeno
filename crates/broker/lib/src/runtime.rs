//! Broker runtime orchestration.

use std::sync::Arc;
use std::time::Duration;

use crate::launcher::LspLauncher;
use crate::services::{knowledge, routing, sessions, shared_state};

/// Orchestrates the lifecycle and wiring of broker services.
///
/// This struct holds handles to all active services in the broker ecosystem.
/// Services operate as independent actors communicating via channels.
pub struct BrokerRuntime {
	/// Handle for session management and IPC delivery.
	pub sessions: sessions::SessionHandle,
	/// Handle for LSP server lifecycle and routing.
	pub routing: routing::RoutingHandle,
	/// Handle for shared document state.
	pub shared_state: shared_state::SharedStateHandle,
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
		let (sessions, sessions_routing_tx, sessions_shared_tx) = sessions::SessionService::start();
		let (shared_state, open_docs, shared_knowledge_tx, shared_routing_tx) =
			shared_state::SharedStateService::start(sessions.clone());
		let knowledge = knowledge::KnowledgeService::start(shared_state.clone(), open_docs);

		let _ = shared_knowledge_tx.try_send(knowledge.clone());

		let routing = routing::RoutingService::start(
			sessions.clone(),
			knowledge.clone(),
			launcher,
			idle_lease,
		);

		let _ = sessions_routing_tx.try_send(routing.clone());
		let _ = sessions_shared_tx.try_send(shared_state.clone());
		let _ = shared_routing_tx.try_send(routing.clone());

		Arc::new(Self {
			sessions,
			routing,
			shared_state,
			knowledge,
		})
	}

	/// Triggers a graceful shutdown of all services and managed LSP processes.
	pub async fn shutdown(&self) {
		self.routing.terminate_all().await;
	}
}
