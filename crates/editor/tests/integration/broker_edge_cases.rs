#[cfg(feature = "lsp")]
mod tests {
	use std::time::Duration;

	use xeno_broker_proto::types::{
		RequestPayload, ServerId, SessionId, SyncEpoch, SyncSeq, WireOp, WireTx,
	};
	use xeno_editor::lsp::broker_transport::BrokerTransport;
	use xeno_lsp::client::transport::LspTransport;
	use xeno_lsp::{AnyNotification, Message};

	use crate::common::{spawn_broker, test_server_config, wait_until};

	#[tokio::test]
	async fn test_broker_reconnect_wedge() {
		let (sock, runtime, launcher, shutdown, _tmp): crate::common::SpawnedBroker =
			spawn_broker().await;

		let t1 = BrokerTransport::with_socket_and_session(sock.clone(), SessionId(1));
		let cfg = test_server_config();
		let s1 = LspTransport::start(t1.as_ref(), cfg.clone())
			.await
			.expect("t1 start");
		let server_id = ServerId(s1.id.0);

		let handle = launcher.get_server(server_id).expect("server handle");

		// 1. Session 1 opens file
		let did_open_1: AnyNotification = serde_json::from_value(serde_json::json!({
			"method": "textDocument/didOpen",
			"params": {
				"textDocument": {
					"uri": "file:///test.rs",
					"languageId": "rust",
					"version": 1,
					"text": "content 1"
				}
			}
		}))
		.unwrap();
		LspTransport::notify(t1.as_ref(), s1.id, did_open_1)
			.await
			.expect("t1 notify");
		t1.shared_state_request(RequestPayload::SharedOpen {
			uri: "file:///test.rs".to_string(),
			text: "content 1".to_string(),
			version_hint: Some(1),
		})
		.await
		.expect("shared state open");


		// Wait for broker to register doc
		assert!(
			wait_until(Duration::from_secs(1), || async {
				runtime.shared_state.is_open("file:///test.rs".to_string()).await
			})
			.await
		);

		// Verify server received didOpen
		{
			let received = handle.received.lock().unwrap();
			assert!(received.iter().any(
				|m| matches!(m, Message::Notification(n) if n.method == "textDocument/didOpen")
			));
		}

		// 2. Session 1 "dies" (disconnects)
		drop(t1);
		runtime.sessions.unregister(SessionId(1)).await;
		runtime.routing.session_lost(SessionId(1)).await;
		runtime.shared_state.session_lost(SessionId(1)).await;

		// Doc should be removed from broker because no one else has it open
		assert!(
			wait_until(Duration::from_secs(1), || async {
				!runtime.shared_state.is_open("file:///test.rs".to_string()).await
			})
			.await
		);

		// 3. Session 2 connects to same project
		let t2 = BrokerTransport::with_socket_and_session(sock.clone(), SessionId(2));
		let s2 = LspTransport::start(t2.as_ref(), cfg.clone())
			.await
			.expect("t2 start");
		assert_eq!(s1.id, s2.id);

		// 4. Session 2 tries to open same file and change it
		let did_open_2: AnyNotification = serde_json::from_value(serde_json::json!({
			"method": "textDocument/didOpen",
			"params": {
				"textDocument": {
					"uri": "file:///test.rs",
					"languageId": "rust",
					"version": 10,
					"text": "content 2"
				}
			}
		}))
		.unwrap();
		LspTransport::notify(t2.as_ref(), s2.id, did_open_2)
			.await
			.expect("t2 notify open");
		t2.shared_state_request(RequestPayload::SharedOpen {
			uri: "file:///test.rs".to_string(),
			text: "content 2".to_string(),
			version_hint: Some(10),
		})
		.await
		.expect("shared state open");

		// Wait for broker to register doc
		assert!(
			wait_until(Duration::from_secs(1), || async {
				runtime.shared_state.is_open("file:///test.rs".to_string()).await
			})
			.await
		);

		// Wait for server to receive the second didOpen before edits.
		assert!(
			wait_until(Duration::from_secs(1), || async {
				let received = handle.received.lock().unwrap();
				received
					.iter()
					.filter(
						|m| matches!(m, Message::Notification(n) if n.method == "textDocument/didOpen"),
					)
					.count()
					== 2
			})
			.await
		);

		let wire_tx = WireTx(vec![
			WireOp::Delete("content 2".chars().count()),
			WireOp::Insert("content 2 updated".into()),
		]);
		t2.shared_state_request(RequestPayload::SharedEdit {
			uri: "file:///test.rs".to_string(),
			epoch: SyncEpoch(1),
			base_seq: SyncSeq(0),
			tx: wire_tx,
		})
		.await
		.expect("shared state edit");

		// Verify server received second didOpen and didChange
		let ok = wait_until(Duration::from_secs(1), || async {
			let received = handle.received.lock().unwrap();
			let opens = received
				.iter()
				.filter(
					|m| matches!(m, Message::Notification(n) if n.method == "textDocument/didOpen"),
				)
				.count();
			let changes = received
				.iter()
				.filter(
					|m| matches!(m, Message::Notification(n) if n.method == "textDocument/didChange"),
				)
				.count();
			opens == 2 && changes == 1
		})
		.await;
		if !ok {
			let received = handle.received.lock().unwrap();
			let opens = received
				.iter()
				.filter(
					|m| matches!(m, Message::Notification(n) if n.method == "textDocument/didOpen"),
				)
				.count();
			let changes = received
				.iter()
				.filter(
					|m| matches!(m, Message::Notification(n) if n.method == "textDocument/didChange"),
				)
				.count();
			panic!("expected opens=2 changes=1, got opens={opens} changes={changes}");
		}

		shutdown.cancel();
	}

	#[tokio::test]
	async fn test_broker_owner_close_transfer() {
		let (sock, _runtime, launcher, shutdown, _tmp): crate::common::SpawnedBroker =
			spawn_broker().await;

		let t1 = BrokerTransport::with_socket_and_session(sock.clone(), SessionId(1));
		let t2 = BrokerTransport::with_socket_and_session(sock.clone(), SessionId(2));
		let cfg = test_server_config();

		let s1 = LspTransport::start(t1.as_ref(), cfg.clone())
			.await
			.expect("t1 start");
		let s2 = LspTransport::start(t2.as_ref(), cfg.clone())
			.await
			.expect("t2 start");
		let server_id = ServerId(s1.id.0);

		let handle = launcher.get_server(server_id).expect("server handle");

		// 1. Both open the file. Session 1 is owner.
		let did_open: AnyNotification = serde_json::from_value(serde_json::json!({
			"method": "textDocument/didOpen",
			"params": {
				"textDocument": {
					"uri": "file:///test.rs",
					"languageId": "rust",
					"version": 1,
					"text": "content"
				}
			}
		}))
		.unwrap();

		LspTransport::notify(t1.as_ref(), s1.id, did_open.clone())
			.await
			.expect("t1 notify");
		t1.shared_state_request(RequestPayload::SharedOpen {
			uri: "file:///test.rs".to_string(),
			text: "content".to_string(),
			version_hint: Some(1),
		})
		.await
		.expect("shared state open");
		LspTransport::notify(t2.as_ref(), s2.id, did_open)
			.await
			.expect("t2 notify");
		t2.shared_state_request(RequestPayload::SharedOpen {
			uri: "file:///test.rs".to_string(),
			text: "content".to_string(),
			version_hint: Some(1),
		})
		.await
		.expect("shared state open");

		// Verify only ONE didOpen reached server
		assert!(
			wait_until(Duration::from_secs(1), || async {
				let received = handle.received.lock().unwrap();
				received
					.iter()
					.filter(
						|m| matches!(m, Message::Notification(n) if n.method == "textDocument/didOpen"),
					)
					.count() == 1
			})
			.await
		);

		// 2. Session 1 closes the file.
		let did_close: AnyNotification = serde_json::from_value(serde_json::json!({
			"method": "textDocument/didClose",
			"params": {
				"textDocument": { "uri": "file:///test.rs" }
			}
		}))
		.unwrap();
		LspTransport::notify(t1.as_ref(), s1.id, did_close)
			.await
			.expect("t1 close");
		t1.shared_state_request(RequestPayload::SharedClose {
			uri: "file:///test.rs".to_string(),
		})
		.await
		.expect("shared state close");

		// Verify NO didClose reached server (since T2 still has it open)
		tokio::time::sleep(Duration::from_millis(100)).await;
		{
			let received = handle.received.lock().unwrap();
			assert!(!received.iter().any(
				|m| matches!(m, Message::Notification(n) if n.method == "textDocument/didClose")
			));
		}

		// 3. Session 2 should now be able to send changes (takeover)
		let focus_resp = t2
			.shared_state_request(RequestPayload::SharedFocus {
				uri: "file:///test.rs".to_string(),
				focused: true,
				focus_seq: 1,
			})
			.await
			.expect("shared focus");
		let epoch = match focus_resp {
			xeno_broker_proto::types::ResponsePayload::SharedFocusAck { snapshot } => snapshot.epoch,
			other => panic!("unexpected focus response: {other:?}"),
		};

		t2.shared_state_request(RequestPayload::SharedResync {
			uri: "file:///test.rs".to_string(),
			client_hash64: None,
			client_len_chars: None,
		})
		.await
		.expect("shared resync");

		let wire_tx = WireTx(vec![
			WireOp::Delete("content".chars().count()),
			WireOp::Insert("session 2 update".into()),
		]);
		t2.shared_state_request(RequestPayload::SharedEdit {
			uri: "file:///test.rs".to_string(),
			epoch,
			base_seq: SyncSeq(0),
			tx: wire_tx,
		})
		.await
		.expect("shared state edit");

		assert!(
			wait_until(Duration::from_secs(1), || async {
				let received = handle.received.lock().unwrap();
				received.iter().any(
					|m| matches!(m, Message::Notification(n) if n.method == "textDocument/didChange"),
				)
			})
			.await
		);

		shutdown.cancel();
	}

	#[tokio::test]
	async fn test_broker_owner_takeover_without_close() {
		let (sock, _runtime, launcher, shutdown, _tmp): crate::common::SpawnedBroker =
			spawn_broker().await;

		let t1 = BrokerTransport::with_socket_and_session(sock.clone(), SessionId(1));
		let t2 = BrokerTransport::with_socket_and_session(sock.clone(), SessionId(2));
		let cfg = test_server_config();

		let s1 = LspTransport::start(t1.as_ref(), cfg.clone())
			.await
			.expect("t1 start");
		let s2 = LspTransport::start(t2.as_ref(), cfg)
			.await
			.expect("t2 start");
		let server_id = ServerId(s1.id.0);

		let handle = launcher.get_server(server_id).expect("server handle");

		let did_open_1: AnyNotification = serde_json::from_value(serde_json::json!({
			"method": "textDocument/didOpen",
			"params": {
				"textDocument": {
					"uri": "file:///test.rs",
					"languageId": "rust",
					"version": 1,
					"text": "content 1"
				}
			}
		}))
		.unwrap();
		LspTransport::notify(t1.as_ref(), s1.id, did_open_1)
			.await
			.expect("t1 notify");
		t1.shared_state_request(RequestPayload::SharedOpen {
			uri: "file:///test.rs".to_string(),
			text: "content 1".to_string(),
			version_hint: Some(1),
		})
		.await
		.expect("shared state open");

		let did_open_2: AnyNotification = serde_json::from_value(serde_json::json!({
			"method": "textDocument/didOpen",
			"params": {
				"textDocument": {
					"uri": "file:///test.rs",
					"languageId": "rust",
					"version": 2,
					"text": "content 1"
				}
			}
		}))
		.unwrap();
		LspTransport::notify(t2.as_ref(), s2.id, did_open_2)
			.await
			.expect("t2 notify");
		t2.shared_state_request(RequestPayload::SharedOpen {
			uri: "file:///test.rs".to_string(),
			text: "content 1".to_string(),
			version_hint: Some(2),
		})
		.await
		.expect("shared state open");

		assert!(
			wait_until(Duration::from_secs(1), || async {
				let received = handle.received.lock().unwrap();
				received.iter().any(
					|m| matches!(m, Message::Notification(n) if n.method == "textDocument/didOpen"),
				)
			})
			.await
		);

		let focus_resp = t2
			.shared_state_request(RequestPayload::SharedFocus {
				uri: "file:///test.rs".to_string(),
				focused: true,
				focus_seq: 1,
			})
			.await
			.expect("shared focus");
		let epoch = match focus_resp {
			xeno_broker_proto::types::ResponsePayload::SharedFocusAck { snapshot } => snapshot.epoch,
			other => panic!("unexpected focus response: {other:?}"),
		};

		t2.shared_state_request(RequestPayload::SharedResync {
			uri: "file:///test.rs".to_string(),
			client_hash64: None,
			client_len_chars: None,
		})
		.await
		.expect("shared resync");

		let wire_tx = WireTx(vec![
			WireOp::Delete("content 1".chars().count()),
			WireOp::Insert("session 2 update".into()),
		]);
		t2.shared_state_request(RequestPayload::SharedEdit {
			uri: "file:///test.rs".to_string(),
			epoch,
			base_seq: SyncSeq(0),
			tx: wire_tx,
		})
		.await
		.expect("shared state edit");

		assert!(
			wait_until(Duration::from_secs(1), || async {
				let received = handle.received.lock().unwrap();
				received.iter().any(
					|m| matches!(m, Message::Notification(n) if n.method == "textDocument/didChange"),
				)
			})
			.await
		);

		shutdown.cancel();
	}

	#[tokio::test]
	async fn test_broker_string_wire_ids() {
		let (sock, _runtime, launcher, shutdown, _tmp): crate::common::SpawnedBroker =
			spawn_broker().await;

		let t1 = BrokerTransport::with_socket_and_session(sock.clone(), SessionId(1));
		let cfg = test_server_config();
		let s1 = LspTransport::start(t1.as_ref(), cfg.clone())
			.await
			.expect("t1 start");
		let server_id = ServerId(s1.id.0);

		let handle = launcher.get_server(server_id).expect("server handle");

		// Send a request from editor
		let req: xeno_lsp::AnyRequest = serde_json::from_value(serde_json::json!({
			"id": 1,
			"method": "textDocument/hover",
			"params": {
				"textDocument": { "uri": "file:///test.rs" },
				"position": { "line": 0, "character": 0 }
			}
		}))
		.unwrap();

		let t1_clone = t1.clone();
		let request_future = tokio::spawn(async move {
			LspTransport::request(t1_clone.as_ref(), s1.id, req, None).await
		});

		// Verify ID is a string and matches expected format "b:{server_id}:{wire_num}"
		assert!(
			wait_until(Duration::from_secs(1), || async {
				let received = handle.received.lock().unwrap();
				received.iter().any(|m| {
					if let Message::Request(r) = m {
						if r.method == "textDocument/hover" {
							if let xeno_lsp::RequestId::String(s) = &r.id {
								return s.starts_with(&format!("b:{}:", server_id.0));
							}
						}
					}
					false
				})
			})
			.await
		);

		// Verify editor received response
		let editor_resp = request_future
			.await
			.expect("task join")
			.expect("request fail");
		assert_eq!(
			editor_resp.result,
			Some(serde_json::json!({ "contents": "hover content" }))
		);
		// Original ID should be restored
		assert_eq!(editor_resp.id, xeno_lsp::RequestId::Number(1));

		shutdown.cancel();
	}
}
