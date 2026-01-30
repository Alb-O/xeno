#[cfg(feature = "lsp")]
mod tests {
	use std::sync::Arc;
	use std::time::Duration;

	use tokio_util::sync::CancellationToken;
	use xeno_broker::core::BrokerCore;
	use xeno_broker::ipc;
	use xeno_broker::test_helpers::TestLauncher;
	use xeno_broker_proto::types::{ServerId, SessionId};
	use xeno_editor::lsp::broker_transport::BrokerTransport;
	use xeno_lsp::client::transport::{LspTransport, StartedServer, TransportEvent};
	use xeno_lsp::{AnyNotification, AnyResponse, Message};
	use xeno_rpc::MainLoopEvent;

	async fn spawn_broker() -> (
		std::path::PathBuf,
		Arc<BrokerCore>,
		TestLauncher,
		CancellationToken,
		tempfile::TempDir,
	) {
		let _ = tracing_subscriber::fmt::try_init();
		let tmp = tempfile::tempdir().unwrap();
		let sock = tmp.path().join("broker.sock");
		let core = BrokerCore::new();
		let launcher = TestLauncher::new();
		let shutdown = CancellationToken::new();

		let core_clone = core.clone();
		let launcher_clone = Arc::new(launcher.clone());
		let sock_clone = sock.clone();
		let shutdown_clone = shutdown.clone();

		tokio::spawn(async move {
			ipc::serve_with_launcher(sock_clone, core_clone, shutdown_clone, launcher_clone)
				.await
				.unwrap();
		});

		// Wait for socket to be ready
		let mut attempts = 0;
		while !sock.exists() && attempts < 50 {
			tokio::time::sleep(Duration::from_millis(10)).await;
			attempts += 1;
		}

		(sock, core, launcher, shutdown, tmp)
	}

	fn test_server_config() -> xeno_lsp::ServerConfig {
		xeno_lsp::ServerConfig::new("rust-analyzer", "/test")
	}

	#[tokio::test]
	async fn test_broker_e2e_dedup_and_fanout() {
		let (sock, _core, launcher, shutdown, _tmp) = spawn_broker().await;

		let t1 = BrokerTransport::with_socket_and_session(sock.clone(), SessionId(1));
		let t2 = BrokerTransport::with_socket_and_session(sock.clone(), SessionId(2));

		let mut rx1 = t1.events();
		let mut rx2 = t2.events();

		let cfg = test_server_config();
		let s1: StartedServer = LspTransport::start(t1.as_ref(), cfg.clone())
			.await
			.expect("t1 start");
		let s2: StartedServer = LspTransport::start(t2.as_ref(), cfg)
			.await
			.expect("t2 start");

		// Assert dedup works: both should have same server id
		assert_eq!(s1.id, s2.id);
		let server_id = ServerId(s1.id.0);

		// Client 1 sends didOpen to register document in broker
		let did_open: AnyNotification = serde_json::from_str(
			r#"{
			"method": "textDocument/didOpen",
			"params": {
				"textDocument": {
					"uri": "file:///test.rs",
					"languageId": "rust",
					"version": 1,
					"text": "test content"
				}
			}
		}"#,
		)
		.unwrap();
		LspTransport::notify(t1.as_ref(), s1.id, did_open)
			.await
			.expect("t1 notify");

		// Get fake server handle and wait for didOpen to arrive
		let mut attempts = 0;
		let handle = loop {
			if let Some(h) = launcher.get_server(server_id) {
				// We can't easily check FakeLsp internals from here without more wiring,
				// so we'll just wait a bit for the message to propagate through the IPC.
				tokio::time::sleep(Duration::from_millis(100)).await;
				break h;
			}
			tokio::time::sleep(Duration::from_millis(10)).await;
			attempts += 1;
			if attempts > 50 {
				panic!("Timeout waiting for server handle");
			}
		};

		// Also ensure broker has the doc registered by checking directly if we have access to core
		let mut attempts = 0;
		while _core.get_doc_by_uri(server_id, "file:///test.rs").is_none() {
			tokio::time::sleep(Duration::from_millis(10)).await;
			attempts += 1;
			if attempts > 100 {
				panic!("Timeout waiting for document registration in broker");
			}
		}

		let diags = serde_json::json!({
			"method": "textDocument/publishDiagnostics",
			"params": {
				"uri": "file:///test.rs",
				"diagnostics": []
			}
		});
		let msg = Message::Notification(serde_json::from_value(diags).unwrap());
		handle
			.server_socket
			.send(MainLoopEvent::Outgoing(msg))
			.expect("send diags");

		// Both should receive diagnostics
		let check_recv = |mut rx: tokio::sync::mpsc::UnboundedReceiver<TransportEvent>| async move {
			tokio::time::timeout(Duration::from_secs(2), async {
				loop {
					match rx.recv().await {
						Some(TransportEvent::Diagnostics { uri, .. })
							if uri == "file:///test.rs" =>
						{
							return;
						}
						Some(_) => continue,
						None => panic!("Event stream closed"),
					}
				}
			})
			.await
			.expect("Timeout waiting for diagnostics")
		};

		tokio::join!(check_recv(rx1), check_recv(rx2));

		shutdown.cancel();
	}

	#[tokio::test]
	async fn test_broker_e2e_leader_routing_and_reply() {
		let (sock, _core, launcher, shutdown, _tmp) = spawn_broker().await;

		let t1 = BrokerTransport::with_socket_and_session(sock.clone(), SessionId(1));
		let t2 = BrokerTransport::with_socket_and_session(sock.clone(), SessionId(2));

		let mut rx1 = t1.events();
		let _rx2 = t2.events();

		let cfg = test_server_config();
		let s1: StartedServer = LspTransport::start(t1.as_ref(), cfg.clone())
			.await
			.expect("t1 start");
		let _s2: StartedServer = LspTransport::start(t2.as_ref(), cfg)
			.await
			.expect("t2 start");

		let server_id = ServerId(s1.id.0);
		let handle = launcher.get_server(server_id).expect("server handle");

		// Trigger server->client request
		let (resp_tx, resp_rx) = tokio::sync::oneshot::channel();
		let req: xeno_lsp::AnyRequest = serde_json::from_str(
			r#"{
			"id": 123,
			"method": "workspace/configuration",
			"params": {}
		}"#,
		)
		.unwrap();

		handle
			.server_socket
			.send(MainLoopEvent::OutgoingRequest(req, resp_tx))
			.expect("send req");

		// Leader (t1) should receive it
		let _req_received = tokio::time::timeout(Duration::from_secs(2), async {
			loop {
				match rx1.recv().await {
					Some(TransportEvent::Message {
						message: Message::Request(r),
						..
					}) if r.method == "workspace/configuration" => return r,
					Some(_) => continue,
					None => panic!("Event stream closed"),
				}
			}
		})
		.await
		.expect("Leader timeout waiting for request");

		// Leader replies
		LspTransport::reply(t1.as_ref(), s1.id, Ok(serde_json::json!([{}])))
			.await
			.expect("t1 reply");

		// Fake server should receive response
		let resp: AnyResponse = tokio::time::timeout(Duration::from_secs(2), resp_rx)
			.await
			.expect("Server timeout waiting for response")
			.expect("response channel closed");
		assert!(resp.error.is_none());

		shutdown.cancel();
	}
}
