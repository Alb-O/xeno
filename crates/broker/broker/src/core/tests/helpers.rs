//! Common test utilities and helpers.

use std::time::Duration;

use tokio::sync::mpsc;
use xeno_broker_proto::types::{LspServerConfig, LspServerStatus, SessionId};
use xeno_rpc::{MainLoopEvent, PeerSocket};

use crate::core::{IpcFrame, LspInstance, Request, Response, SessionSink};

/// A test harness that captures events sent to sessions.
pub struct TestSession {
	pub session_id: SessionId,
	pub sink: SessionSink,
	pub events_rx: mpsc::UnboundedReceiver<MainLoopEvent<IpcFrame, Request, Response>>,
}

impl TestSession {
	/// Create a new test session with a unique ID.
	pub fn new(id: u64) -> Self {
		let (tx, rx) = mpsc::unbounded_channel();
		let sink = PeerSocket::from_sender(tx);
		Self {
			session_id: SessionId(id),
			sink,
			events_rx: rx,
		}
	}

	/// Try to receive an event, returning None if none available.
	pub fn try_recv(&mut self) -> Option<MainLoopEvent<IpcFrame, Request, Response>> {
		self.events_rx.try_recv().ok()
	}

	/// Wait for an event with a timeout.
	#[allow(dead_code)]
	pub async fn recv_timeout(&mut self) -> Option<MainLoopEvent<IpcFrame, Request, Response>> {
		let timeout: tokio::time::Timeout<_> =
			tokio::time::timeout(Duration::from_millis(100), self.events_rx.recv());
		timeout.await.ok().flatten()
	}
}

pub fn test_config(cmd: &str, cwd: &str) -> LspServerConfig {
	LspServerConfig {
		command: cmd.to_string(),
		args: vec!["--test".to_string()],
		env: vec![],
		cwd: Some(cwd.to_string()),
	}
}

pub fn mock_instance() -> LspInstance {
	let (tx, _rx) = mpsc::unbounded_channel();
	LspInstance::mock(PeerSocket::from_sender(tx), LspServerStatus::Starting)
}
