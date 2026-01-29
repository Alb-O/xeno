//! LSP server lifecycle: process spawning and initialization.

use std::process::Stdio;
use std::sync::Arc;
use std::time::Duration;

use tokio::process::Command;
use tokio::sync::{Notify, OnceCell, mpsc, watch};
use tracing::{error, info, warn};

use super::config::{LanguageServerId, ServerConfig};
use super::event_handler::{NoOpEventHandler, SharedEventHandler};
use super::handle::ClientHandle;
use super::outbox::{OUTBOUND_QUEUE_LEN, outbound_dispatcher};
use super::router_setup::{ClientState, build_router};
use super::state::ServerState;
use crate::{MainLoop, Result, uri_from_path};

/// Start a language server process and return a handle to communicate with it.
///
/// This spawns the server process and starts the main loop in a background task.
/// Returns a [`ClientHandle`] that can be used to send requests and notifications.
///
/// # Arguments
///
/// * `id` - Unique identifier for this server instance
/// * `name` - Human-readable name for the server
/// * `config` - Server configuration (command, args, root path, etc.)
/// * `event_handler` - Optional handler for server-to-client events (diagnostics, etc.)
///
/// # Returns
///
/// A tuple of:
/// * `ClientHandle` - Handle for communicating with the server
/// * `JoinHandle` - Handle to the background task running the main loop
pub fn start_server(
	id: LanguageServerId,
	name: String,
	config: ServerConfig,
	event_handler: Option<SharedEventHandler>,
) -> Result<(ClientHandle, tokio::task::JoinHandle<Result<()>>)> {
	let root_uri = uri_from_path(&config.root_path);

	let mut cmd = Command::new(&config.command);
	cmd.args(&config.args)
		.envs(&config.env)
		.stdin(Stdio::piped())
		.stdout(Stdio::piped())
		.stderr(Stdio::piped())
		.current_dir(&config.root_path)
		.kill_on_drop(true);

	// Detach from controlling TTY to prevent LSP from writing directly to terminal
	#[cfg(unix)]
	cmd.process_group(0);

	let mut process = cmd.spawn().map_err(|e| crate::Error::ServerSpawn {
		server: config.command.clone(),
		reason: e.to_string(),
	})?;

	let stdin = process
		.stdin
		.take()
		.ok_or_else(|| crate::Error::ServerSpawn {
			server: config.command.clone(),
			reason: "stdin pipe not available".into(),
		})?;
	let stdout = process
		.stdout
		.take()
		.ok_or_else(|| crate::Error::ServerSpawn {
			server: config.command.clone(),
			reason: "stdout pipe not available".into(),
		})?;
	let stderr = process
		.stderr
		.take()
		.ok_or_else(|| crate::Error::ServerSpawn {
			server: config.command.clone(),
			reason: "stderr pipe not available".into(),
		})?;

	tokio::spawn({
		let server_id = id;
		async move {
			use tokio::io::AsyncBufReadExt;
			let reader = tokio::io::BufReader::new(stderr);
			let mut lines = reader.lines();
			while let Ok(Some(line)) = lines.next_line().await {
				warn!(server_id = server_id.0, stderr = %line, "LSP server stderr");
			}
		}
	});

	let capabilities = Arc::new(OnceCell::new());
	let initialize_notify = Arc::new(Notify::new());
	let (state_tx, state_rx) = watch::channel(ServerState::Starting);

	let handler: SharedEventHandler = event_handler.unwrap_or_else(|| Arc::new(NoOpEventHandler));
	let state = Arc::new(ClientState::new(id, handler));
	let (main_loop, socket) = MainLoop::new_client(|_socket| build_router(state));

	let (outbound_tx, outbound_rx) = mpsc::channel(OUTBOUND_QUEUE_LEN);
	tokio::spawn(outbound_dispatcher(outbound_rx, socket, state_rx));

	let handle = ClientHandle {
		id,
		name,
		capabilities,
		root_path: config.root_path,
		root_uri,
		initialize_notify,
		outbound_tx,
		timeout: Duration::from_secs(config.timeout_secs),
		state_tx,
	};

	let server_id = id;
	let join_handle = tokio::spawn(async move {
		let result = main_loop.run_buffered(stdout, stdin).await;
		if let Err(ref e) = result {
			error!(server_id = server_id.0, error = %e, "LSP main loop error");
		} else {
			info!(server_id = server_id.0, "LSP main loop exited normally");
		}

		drop(process);
		result
	});

	Ok((handle, join_handle))
}
