//! Service-level tests for broker actor subsystems.

use std::collections::HashSet;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use tokio::sync::mpsc;
use xeno_broker_proto::types::{Event, IpcFrame, LspServerConfig, Request, Response, SessionId};
use xeno_rpc::MainLoopEvent;

use crate::core::{SessionSink, db};
use crate::services::knowledge::KnowledgeHandle;
use crate::services::routing::{RoutingCmd, RoutingHandle};
use crate::services::sessions::{SessionHandle, SessionService};
use crate::services::shared_state::{SharedStateHandle, SharedStateService};

mod routing;
mod sessions;
mod shared_state;

pub(super) struct TestSession {
	pub session_id: SessionId,
	pub sink: SessionSink,
	pub events_rx: mpsc::UnboundedReceiver<MainLoopEvent<IpcFrame, Request, Response>>,
}

impl TestSession {
	pub fn new(id: u64) -> Self {
		let (tx, rx) = mpsc::unbounded_channel();
		Self {
			session_id: SessionId(id),
			sink: SessionSink::from_sender(tx),
			events_rx: rx,
		}
	}

	pub fn try_event(&mut self) -> Option<Event> {
		self.events_rx.try_recv().ok().and_then(extract_event)
	}

	pub async fn recv_event(&mut self) -> Option<Event> {
		let timeout = tokio::time::timeout(Duration::from_millis(500), self.events_rx.recv());
		timeout.await.ok().flatten().and_then(extract_event)
	}
}

fn extract_event(msg: MainLoopEvent<IpcFrame, Request, Response>) -> Option<Event> {
	match msg {
		MainLoopEvent::Outgoing(IpcFrame::Event(event)) => Some(event),
		_ => None,
	}
}

pub(super) fn test_config(cmd: &str, cwd: &str) -> LspServerConfig {
	LspServerConfig {
		command: cmd.to_string(),
		args: vec!["--test".to_string()],
		env: vec![],
		cwd: Some(cwd.to_string()),
	}
}

pub(super) struct SyncHarness {
	pub sessions: SessionHandle,
	pub sync: SharedStateHandle,
	pub open_docs: Arc<Mutex<HashSet<String>>>,
	pub _routing_rx: mpsc::Receiver<RoutingCmd>,
	pub _db_temp: tempfile::TempDir,
}

pub(super) async fn setup_sync_harness() -> SyncHarness {
	let (sessions_handle, routing_tx, sync_tx) = SessionService::start();

	let (dummy_routing_tx, dummy_routing_rx) = mpsc::channel(8);
	let dummy_routing = RoutingHandle::new(dummy_routing_tx);
	let _ = routing_tx.send(dummy_routing.clone()).await;

	let db_temp = tempfile::tempdir().expect("temp db dir");
	let db = db::BrokerDb::open(db_temp.path().join("broker")).expect("open broker db");

	let (sync, open_docs, knowledge_tx, sync_routing_tx) =
		SharedStateService::start(sessions_handle.clone(), Some(db.storage()));

	let (knowledge_sender, mut knowledge_rx) = mpsc::channel(8);
	let knowledge = KnowledgeHandle::new(knowledge_sender);
	let _ = knowledge_tx.send(knowledge.clone()).await;
	tokio::spawn(async move { while knowledge_rx.recv().await.is_some() {} });

	let _ = sync_tx.send(sync.clone()).await;
	let _ = sync_routing_tx.send(dummy_routing).await;
	tokio::task::yield_now().await;

	SyncHarness {
		sessions: sessions_handle,
		sync,
		open_docs,
		_routing_rx: dummy_routing_rx,
		_db_temp: db_temp,
	}
}
