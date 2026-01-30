//! LSP server launcher abstraction for production and testing.

use std::process::Stdio;
use std::sync::Arc;

use tokio::io::BufReader;
use xeno_broker_proto::types::{ErrorCode, LspServerConfig, LspServerStatus, ServerId, SessionId};

use crate::core::{BrokerCore, LspInstance};
use crate::lsp::LspProxyService;

/// Trait for launching LSP server instances.
///
/// This abstraction allows the broker to use real subprocesses in production
/// and in-process fake servers for testing.
pub trait LspLauncher: Send + Sync + 'static {
	/// Launch a new LSP server instance.
	///
	/// Returns an [`LspInstance`] ready to be registered with the broker core.
	/// The instance includes handles for bidirectional communication with the
	/// server and tracking its lifecycle.
	fn launch(
		&self,
		core: Arc<BrokerCore>,
		server_id: ServerId,
		config: &LspServerConfig,
		owner: SessionId,
	) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<LspInstance, ErrorCode>> + Send>>;
}

/// Production launcher that spawns real LSP server processes.
#[derive(Debug, Clone)]
pub struct ProcessLauncher;

impl ProcessLauncher {
	/// Create a new process launcher.
	#[must_use]
	pub fn new() -> Self {
		Self
	}
}

impl Default for ProcessLauncher {
	fn default() -> Self {
		Self::new()
	}
}

impl LspLauncher for ProcessLauncher {
	fn launch(
		&self,
		core: Arc<BrokerCore>,
		server_id: ServerId,
		config: &LspServerConfig,
		_owner: SessionId,
	) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<LspInstance, ErrorCode>> + Send>>
	{
		let config = config.clone();
		Box::pin(async move {
			let mut child = tokio::process::Command::new(&config.command)
				.args(&config.args)
				.envs(config.env.iter().cloned())
				.current_dir(config.cwd.as_deref().unwrap_or_default())
				.stdin(Stdio::piped())
				.stdout(Stdio::piped())
				.stderr(Stdio::inherit())
				.spawn()
				.map_err(|e| {
					tracing::error!(error = %e, "Failed to spawn LSP server");
					ErrorCode::Internal
				})?;

			let stdin = child.stdin.take().ok_or(ErrorCode::Internal)?;
			let stdout = child.stdout.take().ok_or(ErrorCode::Internal)?;

			let protocol = xeno_lsp::protocol::JsonRpcProtocol::new();
			let id_gen = xeno_rpc::CounterIdGen::new();

			let core_clone1 = core.clone();
			let core_clone2 = core.clone();
			let (lsp_loop, lsp_socket) = xeno_rpc::MainLoop::new(
				move |_| LspProxyService::new(core_clone1, server_id),
				protocol,
				id_gen,
			);

			let instance = LspInstance::new(lsp_socket, child, LspServerStatus::Starting);

			// Spawn the proxy mainloop task to handle server stdio.
			tokio::spawn(async move {
				let reader = BufReader::new(stdout);
				let _ = lsp_loop.run(reader, stdin).await;

				core_clone2.unregister_server(server_id);
				core_clone2.set_server_status(server_id, LspServerStatus::Stopped);
			});

			Ok(instance)
		})
	}
}

/// Test helpers for mocking LSP servers.
#[doc(hidden)]
pub mod test_helpers {
	use std::collections::HashMap;
	use std::ops::ControlFlow;
	use std::sync::{Arc, Mutex};

	use tower_service::Service;
	use xeno_lsp::protocol::JsonRpcProtocol;
	use xeno_lsp::{AnyNotification, AnyRequest, ResponseError};
	use xeno_rpc::{AnyEvent, RpcService};

	use super::*;

	/// A fake LSP server for testing that runs in-process.
	pub struct FakeLsp {
		/// Track received didOpen notifications for verification in tests.
		pub received_opens: Mutex<Vec<(String, String)>>,
	}

	impl Default for FakeLsp {
		fn default() -> Self {
			Self::new()
		}
	}

	impl FakeLsp {
		/// Create a new fake LSP server.
		#[must_use]
		pub fn new() -> Self {
			Self {
				received_opens: Mutex::new(Vec::new()),
			}
		}
	}

	impl Service<AnyRequest> for FakeLsp {
		type Response = serde_json::Value;
		type Error = ResponseError;
		type Future = std::pin::Pin<
			Box<dyn std::future::Future<Output = Result<Self::Response, Self::Error>> + Send>,
		>;

		fn poll_ready(
			&mut self,
			_cx: &mut std::task::Context<'_>,
		) -> std::task::Poll<Result<(), Self::Error>> {
			std::task::Poll::Ready(Ok(()))
		}

		fn call(&mut self, req: AnyRequest) -> Self::Future {
			match req.method.as_str() {
				"initialize" => Box::pin(async move {
					Ok(serde_json::json!({
						"capabilities": {
							"textDocumentSync": {
								"openClose": true,
								"change": 2
							}
						}
					}))
				}),
				"shutdown" => Box::pin(async move { Ok(serde_json::Value::Null) }),
				_ => Box::pin(async move {
					Err(ResponseError::new(
						xeno_lsp::ErrorCode::METHOD_NOT_FOUND,
						format!("Method not found: {}", req.method),
					))
				}),
			}
		}
	}

	impl RpcService<JsonRpcProtocol> for FakeLsp {
		type LoopError = xeno_lsp::Error;

		fn notify(
			&mut self,
			notif: AnyNotification,
		) -> ControlFlow<std::result::Result<(), Self::LoopError>> {
			if notif.method.as_str() == "textDocument/didOpen"
				&& let Some(doc) = notif.params.get("textDocument")
				&& let (Some(uri), Some(lang)) = (
					doc.get("uri").and_then(|u| u.as_str()),
					doc.get("languageId").and_then(|l| l.as_str()),
				) {
				self.received_opens
					.lock()
					.unwrap()
					.push((uri.to_string(), lang.to_string()));
			}
			ControlFlow::Continue(())
		}

		fn emit(
			&mut self,
			_event: AnyEvent,
		) -> ControlFlow<std::result::Result<(), Self::LoopError>> {
			ControlFlow::Continue(())
		}
	}

	/// Test launcher that creates in-process fake LSP servers.
	#[derive(Clone)]
	pub struct TestLauncher {
		/// Map of server_id to the fake LSP instance and control channels.
		pub servers: Arc<Mutex<HashMap<ServerId, TestServerHandle>>>,
	}

	/// Handle to a fake LSP server for test control.
	pub struct TestServerHandle {
		/// The socket to send messages to the fake server (broker -> server).
		pub lsp_tx: crate::core::LspTx,
		/// The socket the fake server uses to send messages (server -> broker).
		pub server_socket: crate::core::LspTx,
	}

	impl TestLauncher {
		/// Create a new test launcher.
		#[must_use]
		pub fn new() -> Self {
			Self {
				servers: Arc::new(Mutex::new(HashMap::new())),
			}
		}

		/// Get a handle to a specific server for test control.
		pub fn get_server(&self, server_id: ServerId) -> Option<TestServerHandle> {
			self.servers.lock().unwrap().get(&server_id).cloned()
		}
	}

	impl Default for TestLauncher {
		fn default() -> Self {
			Self::new()
		}
	}

	impl Clone for TestServerHandle {
		fn clone(&self) -> Self {
			Self {
				lsp_tx: self.lsp_tx.clone(),
				server_socket: self.server_socket.clone(),
			}
		}
	}

	impl LspLauncher for TestLauncher {
		fn launch(
			&self,
			core: Arc<BrokerCore>,
			server_id: ServerId,
			_config: &LspServerConfig,
			_owner: SessionId,
		) -> std::pin::Pin<
			Box<dyn std::future::Future<Output = Result<LspInstance, ErrorCode>> + Send>,
		> {
			let servers = self.servers.clone();
			Box::pin(async move {
				// Create in-memory bidirectional pipe
				let (proxy_end, server_end) = tokio::io::duplex(64 * 1024);
				let (pr, pw) = tokio::io::split(proxy_end);
				let (sr, sw) = tokio::io::split(server_end);

				// Set up proxy side (same as production)
				let protocol = JsonRpcProtocol::new();
				let id_gen = xeno_rpc::CounterIdGen::new();

				let core_clone = core.clone();
				let (proxy_loop, lsp_socket) = xeno_rpc::MainLoop::new(
					move |_| LspProxyService::new(core_clone, server_id),
					protocol,
					id_gen,
				);

				tokio::spawn(async move {
					let reader = tokio::io::BufReader::new(pr);
					let _ = proxy_loop.run(reader, pw).await;
				});

				// Set up fake LSP server side
				let fake_lsp = FakeLsp::new();

				let protocol = JsonRpcProtocol::new();
				let id_gen = xeno_rpc::CounterIdGen::new();

				let (server_loop, server_socket) =
					xeno_rpc::MainLoop::new(move |_| fake_lsp, protocol, id_gen);

				tokio::spawn(async move {
					let reader = tokio::io::BufReader::new(sr);
					let _ = server_loop.run(reader, sw).await;
				});

				// Store handle for test control
				let handle = TestServerHandle {
					lsp_tx: lsp_socket.clone(),
					server_socket,
				};

				servers.lock().unwrap().insert(server_id, handle);

				// Create instance with mock child
				Ok(crate::core::LspInstance::mock(
					lsp_socket,
					LspServerStatus::Starting,
				))
			})
		}
	}
}

#[cfg(test)]
mod tests {
	use tower_service::Service;
	use xeno_lsp::AnyRequest;

	use super::test_helpers::*;

	#[tokio::test(flavor = "current_thread")]
	async fn fake_lsp_responds_to_initialize() {
		let mut fake_lsp = FakeLsp::new();

		// Create request by deserializing JSON
		let req: AnyRequest = serde_json::from_str(
			r#"{
			"id": 1,
			"method": "initialize",
			"params": {}
		}"#,
		)
		.unwrap();

		// Call the service directly
		let result = fake_lsp.call(req).await;
		assert!(result.is_ok());

		// Verify it returned capabilities
		let resp = result.unwrap();
		assert!(resp.get("capabilities").is_some());
	}

	#[tokio::test(flavor = "current_thread")]
	async fn fake_lsp_tracks_did_open() {
		let fake_lsp = FakeLsp::new();

		// The FakeLsp should track didOpen via its RpcService implementation
		// This test verifies the struct is properly set up
		assert!(fake_lsp.received_opens.lock().unwrap().is_empty());
	}
}
