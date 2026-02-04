//! Service-level tests for broker actor subsystems.

use std::time::Duration;

use tokio::sync::mpsc;
use xeno_broker_proto::types::{Event, IpcFrame, LspServerConfig, Request, Response, SessionId};
use xeno_rpc::MainLoopEvent;

use crate::core::SessionSink;

mod routing;
mod sessions;
mod shared_state;

pub(super) struct TestSession {
	session_id: SessionId,
	sink: SessionSink,
	events_rx: mpsc::UnboundedReceiver<MainLoopEvent<IpcFrame, Request, Response>>,
}

impl TestSession {
	fn new(id: u64) -> Self {
		let (tx, rx) = mpsc::unbounded_channel();
		Self {
			session_id: SessionId(id),
			sink: SessionSink::from_sender(tx),
			events_rx: rx,
		}
	}

	fn try_event(&mut self) -> Option<Event> {
		self.events_rx.try_recv().ok().and_then(extract_event)
	}

	async fn recv_event(&mut self) -> Option<Event> {
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
