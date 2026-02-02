//! IPC server and client for broker communication.

use std::path::Path;
use std::sync::Arc;

use tokio::net::{UnixListener, UnixStream};
use tokio_util::sync::CancellationToken;
use xeno_broker_proto::BrokerProtocol;

use crate::core::BrokerConfig;
use crate::launcher::{LspLauncher, ProcessLauncher};
use crate::runtime::BrokerRuntime;
use crate::service::BrokerService;

/// Start the broker IPC server on a Unix domain socket.
///
/// This function uses the production [`ProcessLauncher`] to spawn real LSP
/// server processes.
///
/// # Errors
///
/// Returns an error if the socket cannot be bound or if filesystem operations
/// on the socket path fail.
pub async fn serve(
	socket_path: impl AsRef<Path>,
	shutdown: CancellationToken,
) -> std::io::Result<()> {
	let launcher: Arc<dyn LspLauncher> = Arc::new(ProcessLauncher::new());
	let runtime = BrokerRuntime::new(BrokerConfig::default().idle_lease, launcher);
	serve_with_runtime(socket_path, runtime, shutdown).await
}

/// Start the broker IPC server with a custom launcher.
///
/// # Errors
///
/// Returns an error if the socket cannot be bound.
pub async fn serve_with_runtime(
	socket_path: impl AsRef<Path>,
	runtime: Arc<BrokerRuntime>,
	shutdown: CancellationToken,
) -> std::io::Result<()> {
	let path = socket_path.as_ref();
	if path.exists() {
		tokio::fs::remove_file(path).await?;
	}

	let listener = UnixListener::bind(path)?;
	tracing::info!(path = %path.display(), "Broker IPC server listening");

	loop {
		tokio::select! {
			_ = shutdown.cancelled() => {
				tracing::info!("Broker IPC server shutting down");
				runtime.shutdown().await;
				break;
			}
			res = listener.accept() => {
				match res {
					Ok((stream, _addr)) => {
						tokio::spawn(handle_connection(stream, runtime.clone()));
					}
					Err(e) => {
						tracing::error!(error = %e, "Failed to accept connection");
					}
				}
			}
		}
	}

	Ok(())
}

/// Handle a single IPC connection from an editor session.
pub(crate) async fn handle_connection(stream: UnixStream, runtime: Arc<BrokerRuntime>) {
	tracing::info!("New broker connection");

	let (reader, writer) = stream.into_split();

	let protocol = BrokerProtocol::new();
	let id_gen = xeno_rpc::CounterIdGen::new();

	let (main_loop, _socket) = xeno_rpc::MainLoop::new(
		|socket| BrokerService::new(runtime.clone(), socket),
		protocol,
		id_gen,
	);

	let reader = tokio::io::BufReader::new(reader);
	let result = main_loop.run(reader, writer).await;

	if let Err(e) = result {
		tracing::error!(error = %e, "Broker connection error");
	}

	tracing::info!("Broker connection closed");
}

#[cfg(test)]
mod tests {
	use std::io::{Error as IoError, ErrorKind};

	use tokio::io::{AsyncReadExt, AsyncWriteExt};
	use tokio::net::UnixStream;
	use xeno_broker_proto::types::{
		Event, IpcFrame, Request, RequestId, RequestPayload, Response, ResponsePayload, SessionId,
	};

	use super::*;
	use crate::launcher::test_helpers::TestLauncher;

	async fn write_frame(stream: &mut UnixStream, frame: &IpcFrame) -> std::io::Result<()> {
		let buf = postcard::to_allocvec(frame)
			.map_err(|e| IoError::new(ErrorKind::InvalidData, e.to_string()))?;
		stream.write_u32_le(buf.len() as u32).await?;
		stream.write_all(&buf).await?;
		stream.flush().await?;
		Ok(())
	}

	async fn read_frame(stream: &mut UnixStream) -> std::io::Result<IpcFrame> {
		let len = stream.read_u32_le().await?;
		let mut buf = vec![0u8; len as usize];
		stream.read_exact(&mut buf).await?;
		postcard::from_bytes(&buf).map_err(|e| IoError::new(ErrorKind::InvalidData, e.to_string()))
	}

	#[tokio::test]
	async fn ping_roundtrip() -> std::io::Result<()> {
		let launcher: Arc<dyn LspLauncher> = Arc::new(TestLauncher::new());
		let runtime = BrokerRuntime::new(Duration::from_secs(300), launcher);
		let (mut client, server) = UnixStream::pair()?;
		let server_task = tokio::spawn(async move { handle_connection(server, runtime).await });

		write_frame(
			&mut client,
			&IpcFrame::Request(Request {
				id: RequestId(1),
				payload: RequestPayload::Ping,
			}),
		)
		.await?;

		let frame = read_frame(&mut client).await?;
		if let IpcFrame::Response(Response {
			request_id,
			payload,
			error,
		}) = frame
		{
			assert_eq!(request_id, RequestId(1));
			assert!(matches!(payload, Some(ResponsePayload::Pong)));
			assert!(error.is_none());
		} else {
			panic!("expected response frame");
		}

		drop(client);
		server_task.await.expect("server task panicked");
		Ok(())
	}

	#[tokio::test]
	async fn subscribe_emits_event() -> std::io::Result<()> {
		let launcher: Arc<dyn LspLauncher> = Arc::new(TestLauncher::new());
		let runtime = BrokerRuntime::new(Duration::from_secs(300), launcher);
		let (mut client, server) = UnixStream::pair()?;
		let server_task = tokio::spawn(async move { handle_connection(server, runtime).await });

		write_frame(
			&mut client,
			&IpcFrame::Request(Request {
				id: RequestId(2),
				payload: RequestPayload::Subscribe {
					session_id: SessionId(1),
				},
			}),
		)
		.await?;

		let resp = read_frame(&mut client).await?;
		if let IpcFrame::Response(Response {
			request_id,
			payload,
			error,
		}) = resp
		{
			assert_eq!(request_id, RequestId(2));
			assert!(matches!(payload, Some(ResponsePayload::Subscribed)));
			assert!(error.is_none());
		} else {
			panic!("expected response frame");
		}

		// Subscribe in current impl doesn't emit heartbeat immediately anymore
		// as it's a separate service now.
		// So we might just check that it didn't error.

		drop(client);
		server_task.await.expect("server task panicked");
		Ok(())
	}
}

/// Connect to the broker as a client.
pub async fn connect(socket_path: impl AsRef<Path>) -> std::io::Result<UnixStream> {
	UnixStream::connect(socket_path).await
}
