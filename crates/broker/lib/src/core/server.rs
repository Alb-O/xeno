//! Server lifecycle management types.

use std::sync::Mutex;
use std::time::Duration;

use xeno_broker_proto::types::LspServerStatus;

use super::LspTx;

/// Handle to a child process.
#[derive(Debug)]
#[non_exhaustive]
pub enum ChildHandle {
	/// Real spawned process.
	Real(tokio::process::Child),
}

/// Channels for controlling and monitoring a server instance.
#[derive(Debug)]
pub struct ServerControl {
	/// Channel to request graceful termination.
	pub term_tx: tokio::sync::oneshot::Sender<()>,
	/// Channel to await completion of termination.
	pub done_rx: tokio::sync::oneshot::Receiver<()>,
}

/// A running LSP server instance and its associated handles.
#[derive(Debug)]
pub struct LspInstance {
	/// Socket for sending requests/notifications to the server's stdio.
	pub lsp_tx: LspTx,
	/// Control channels for the server lifecycle monitor.
	pub control: Option<ServerControl>,
	/// Synchronized server lifecycle status.
	pub status: Mutex<LspServerStatus>,
}

impl LspInstance {
	/// Create a new LspInstance with control channels.
	pub fn new(lsp_tx: LspTx, control: ServerControl, status: LspServerStatus) -> Self {
		Self {
			lsp_tx,
			control: Some(control),
			status: Mutex::new(status),
		}
	}

	/// Create a mock LspInstance for tests.
	#[doc(hidden)]
	pub fn mock(lsp_tx: LspTx, status: LspServerStatus) -> Self {
		Self {
			lsp_tx,
			control: None,
			status: Mutex::new(status),
		}
	}

	/// Best-effort graceful shutdown, then kill if needed.
	pub async fn terminate(mut self) {
		let Some(control) = self.control.take() else {
			return;
		};

		let _ = control.term_tx.send(());
		let _ = tokio::time::timeout(Duration::from_secs(2), control.done_rx).await;
	}
}
