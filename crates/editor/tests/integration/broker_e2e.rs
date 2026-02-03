#[cfg(feature = "lsp")]
mod tests {
	use std::time::Duration;

	use xeno_broker_proto::types::{RequestPayload, ServerId, SessionId};
	use xeno_editor::lsp::broker_transport::BrokerTransport;
	use xeno_lsp::client::transport::{LspTransport, StartedServer, TransportEvent};
	use xeno_lsp::{AnyNotification, AnyResponse, Message};
	use xeno_rpc::MainLoopEvent;

	use crate::common::{SpawnedBroker, spawn_broker, test_server_config};

	#[tokio::test]
	async fn test_broker_e2e_dedup_and_fanout() {
		let (sock, runtime, launcher, shutdown, _tmp): SpawnedBroker = spawn_broker().await;

		let t1 = BrokerTransport::with_socket_and_session(sock.clone(), SessionId(1));
		let t2 = BrokerTransport::with_socket_and_session(sock.clone(), SessionId(2));

		let rx1 = t1.events();
		let rx2 = t2.events();

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
		t1.shared_state_request(RequestPayload::SharedOpen {
			uri: "file:///test.rs".to_string(),
			text: "test content".to_string(),
			version_hint: Some(1),
		})
		.await
		.expect("shared state open");

		// Get fake server handle and wait for didOpen to arrive
		let mut attempts = 0;
		let handle = loop {
			if let Some(h) = launcher.get_server(server_id) {
				// Wait a bit for the message to propagate through the IPC.
				tokio::time::sleep(Duration::from_millis(100)).await;
				break h;
			}
			tokio::time::sleep(Duration::from_millis(10)).await;
			attempts += 1;
			if attempts > 50 {
				panic!("Timeout waiting for server handle");
			}
		};

		// Also ensure broker has the doc registered by checking directly
		let mut attempts = 0;
		while !runtime
			.shared_state
			.is_open("file:///test.rs".to_string())
			.await
		{
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
	async fn test_broker_diagnostics_replayed_to_new_session() {
		let (sock, runtime, launcher, shutdown, _tmp): SpawnedBroker = spawn_broker().await;

		let t1 = BrokerTransport::with_socket_and_session(sock.clone(), SessionId(1));
		let mut rx1 = t1.events();

		let cfg = test_server_config();
		let s1: StartedServer = LspTransport::start(t1.as_ref(), cfg.clone())
			.await
			.expect("t1 start");
		let server_id = ServerId(s1.id.0);

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
		t1.shared_state_request(RequestPayload::SharedOpen {
			uri: "file:///test.rs".to_string(),
			text: "test content".to_string(),
			version_hint: Some(1),
		})
		.await
		.expect("shared state open");

		let mut attempts = 0;
		let handle = loop {
			if let Some(h) = launcher.get_server(server_id) {
				tokio::time::sleep(Duration::from_millis(100)).await;
				break h;
			}
			tokio::time::sleep(Duration::from_millis(10)).await;
			attempts += 1;
			if attempts > 50 {
				panic!("Timeout waiting for server handle");
			}
		};

		let mut attempts = 0;
		while !runtime
			.shared_state
			.is_open("file:///test.rs".to_string())
			.await
		{
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

		let mut received = false;
		tokio::time::timeout(Duration::from_secs(2), async {
			while let Some(event) = rx1.recv().await {
				if matches!(event, TransportEvent::Diagnostics { uri, .. } if uri == "file:///test.rs")
				{
					received = true;
					break;
				}
			}
		})
		.await
		.expect("Timeout waiting for diagnostics");
		assert!(received, "t1 did not receive diagnostics");

		let t2 = BrokerTransport::with_socket_and_session(sock.clone(), SessionId(2));
		let mut rx2 = t2.events();
		let _s2: StartedServer = LspTransport::start(t2.as_ref(), cfg)
			.await
			.expect("t2 start");

		let mut replayed = false;
		tokio::time::timeout(Duration::from_secs(2), async {
			while let Some(event) = rx2.recv().await {
				if matches!(event, TransportEvent::Diagnostics { uri, .. } if uri == "file:///test.rs")
				{
					replayed = true;
					break;
				}
			}
		})
		.await
		.expect("Timeout waiting for replayed diagnostics");
		assert!(replayed, "t2 did not receive replayed diagnostics");

		shutdown.cancel();
	}

	#[tokio::test]
	async fn test_broker_diagnostics_replayed_after_disconnect() {
		let (sock, runtime, launcher, shutdown, _tmp): SpawnedBroker = spawn_broker().await;

		let t1 = BrokerTransport::with_socket_and_session(sock.clone(), SessionId(1));
		let mut rx1 = t1.events();

		let cfg = test_server_config();
		let s1: StartedServer = LspTransport::start(t1.as_ref(), cfg.clone())
			.await
			.expect("t1 start");
		let server_id = ServerId(s1.id.0);

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
		t1.shared_state_request(RequestPayload::SharedOpen {
			uri: "file:///test.rs".to_string(),
			text: "test content".to_string(),
			version_hint: Some(1),
		})
		.await
		.expect("shared state open");

		let mut attempts = 0;
		let handle = loop {
			if let Some(h) = launcher.get_server(server_id) {
				tokio::time::sleep(Duration::from_millis(100)).await;
				break h;
			}
			tokio::time::sleep(Duration::from_millis(10)).await;
			attempts += 1;
			if attempts > 50 {
				panic!("Timeout waiting for server handle");
			}
		};

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

		let mut received = false;
		tokio::time::timeout(Duration::from_secs(2), async {
			while let Some(event) = rx1.recv().await {
				if matches!(event, TransportEvent::Diagnostics { uri, .. } if uri == "file:///test.rs")
				{
					received = true;
					break;
				}
			}
		})
		.await
		.expect("Timeout waiting for diagnostics");
		assert!(received, "t1 did not receive diagnostics");

		drop(t1);
		runtime.sessions.unregister(SessionId(1)).await;
		runtime.routing.session_lost(SessionId(1)).await;
		runtime.shared_state.session_lost(SessionId(1)).await;

		let t2 = BrokerTransport::with_socket_and_session(sock.clone(), SessionId(2));
		let mut rx2 = t2.events();
		let _s2: StartedServer = LspTransport::start(t2.as_ref(), cfg)
			.await
			.expect("t2 start");

		let mut replayed = false;
		tokio::time::timeout(Duration::from_secs(2), async {
			while let Some(event) = rx2.recv().await {
				if matches!(event, TransportEvent::Diagnostics { uri, .. } if uri == "file:///test.rs")
				{
					replayed = true;
					break;
				}
			}
		})
		.await
		.expect("Timeout waiting for replayed diagnostics");
		assert!(replayed, "t2 did not receive replayed diagnostics");

		shutdown.cancel();
	}

	#[tokio::test]
	async fn test_broker_e2e_leader_routing_and_reply() {
		let (sock, _runtime, launcher, shutdown, _tmp): crate::common::SpawnedBroker =
			spawn_broker().await;

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
		let (resp_tx, resp_rx) = tokio::sync::oneshot::channel::<AnyResponse>();
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

	#[tokio::test(start_paused = true)]
	async fn test_broker_e2e_persistence_warm_reattach() {
		let (sock, _runtime, _launcher, shutdown, _tmp): crate::common::SpawnedBroker =
			spawn_broker().await;

		let cfg = test_server_config();

		// Client 1 starts server
		let t1 = BrokerTransport::with_socket_and_session(sock.clone(), SessionId(1));
		let s1 = LspTransport::start(t1.as_ref(), cfg.clone())
			.await
			.expect("t1 start");

		// Drop client 1 (disconnect)
		drop(t1);
		// Small sleep to allow broker to process disconnect
		tokio::time::sleep(Duration::from_millis(50)).await;

		// Client 2 connects to the same project
		let t2 = BrokerTransport::with_socket_and_session(sock.clone(), SessionId(2));
		let s2 = LspTransport::start(t2.as_ref(), cfg.clone())
			.await
			.expect("t2 start");

		// Should be the same server ID
		assert_eq!(s1.id, s2.id);

		shutdown.cancel();
	}

	#[tokio::test(start_paused = true)]
	async fn test_broker_e2e_persistence_lease_expiry() {
		let (sock, runtime, _launcher, shutdown, _tmp): SpawnedBroker = spawn_broker().await;

		let cfg = test_server_config();

		// Client 1 starts server
		let t1 = BrokerTransport::with_socket_and_session(sock.clone(), SessionId(1));
		let s1 = LspTransport::start(t1.as_ref(), cfg.clone())
			.await
			.expect("t1 start");

		// Drop client 1
		drop(t1);
		runtime.sessions.unregister(SessionId(1)).await;
		runtime.routing.session_lost(SessionId(1)).await;
		runtime.shared_state.session_lost(SessionId(1)).await;

		// Wait for broker to process disconnect
		tokio::time::sleep(Duration::from_millis(100)).await;

		// Advance time past default 5 min lease
		tokio::time::advance(Duration::from_secs(301)).await;
		// Give the cleanup task a chance to run
		tokio::task::yield_now().await;
		tokio::task::yield_now().await;

		// Client 2 connects
		let t2 = BrokerTransport::with_socket_and_session(sock.clone(), SessionId(2));
		let s2 = LspTransport::start(t2.as_ref(), cfg.clone())
			.await
			.expect("t2 start");

		// Should be a NEW server ID
		assert_ne!(
			s1.id, s2.id,
			"Server should have expired and a new one started"
		);

		shutdown.cancel();
	}
}
