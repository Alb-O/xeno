//! IPC server and client for broker communication.

use std::path::Path;

use tokio::net::{UnixListener, UnixStream};

use crate::protocol::BrokerProtocol;
use crate::service::BrokerService;

/// Start the broker IPC server on a Unix domain socket.
pub async fn serve(socket_path: impl AsRef<Path>) -> std::io::Result<()> {
	// Remove existing socket file
	let path = socket_path.as_ref();
	if path.exists() {
		tokio::fs::remove_file(path).await?;
	}

	// Create listener
	let listener = UnixListener::bind(path)?;
	tracing::info!(path = %path.display(), "Broker IPC server listening");

	// Accept connections
	loop {
		match listener.accept().await {
			Ok((stream, _addr)) => {
				tokio::spawn(handle_connection(stream));
			}
			Err(e) => {
				tracing::error!(error = %e, "Failed to accept connection");
			}
		}
	}
}

/// Handle a single IPC connection.
pub(crate) async fn handle_connection(stream: UnixStream) {
	tracing::info!("New broker connection");

	// Split into read/write halves
	let (reader, writer) = stream.into_split();

	// Create protocol
	let protocol = BrokerProtocol::new();
	let id_gen = xeno_rpc::CounterIdGen::new();

	// Create mainloop
	let (main_loop, _socket) =
		xeno_rpc::MainLoop::new(|_socket| BrokerService::new(), protocol, id_gen);

	// Run the mainloop
	// Note: This is a simplified version. In production, you'd want proper
	// buffering and error handling here.
	let reader = tokio::io::BufReader::new(reader);
	if let Err(e) = main_loop.run(reader, writer).await {
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
		let (mut client, server) = UnixStream::pair()?;
		let server_task = tokio::spawn(async move { handle_connection(server).await });

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
		}) = frame
		{
			assert_eq!(request_id, RequestId(1));
			assert!(matches!(payload, ResponsePayload::Pong));
		} else {
			panic!("expected response frame");
		}

		drop(client);
		server_task.await.expect("server task panicked");
		Ok(())
	}

	#[tokio::test]
	async fn subscribe_emits_event() -> std::io::Result<()> {
		let (mut client, server) = UnixStream::pair()?;
		let server_task = tokio::spawn(async move { handle_connection(server).await });

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
		}) = resp
		{
			assert_eq!(request_id, RequestId(2));
			assert!(matches!(payload, ResponsePayload::Subscribed));
		} else {
			panic!("expected response frame");
		}

		let event = read_frame(&mut client).await?;
		assert!(matches!(event, IpcFrame::Event(Event::Heartbeat)));

		drop(client);
		server_task.await.expect("server task panicked");
		Ok(())
	}
}

/// Connect to the broker as a client.
pub async fn connect(socket_path: impl AsRef<Path>) -> std::io::Result<UnixStream> {
	UnixStream::connect(socket_path).await
}
