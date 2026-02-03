#[cfg(feature = "lsp")]
mod tests {
	use std::time::Duration;

	use xeno_broker_proto::types::{RequestPayload, ServerId, SessionId};
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
		t1.buffer_sync_request(RequestPayload::BufferSyncOpen {
			uri: "file:///test.rs".to_string(),
			text: "content 1".to_string(),
			version_hint: Some(1),
		})
		.await
		.expect("buffer sync open");

		// Wait for broker to register doc
		assert!(
			wait_until(Duration::from_secs(1), || async {
				runtime.sync.is_open("file:///test.rs".to_string()).await
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
		runtime.sync.session_lost(SessionId(1)).await;

		// Doc should be removed from broker because no one else has it open
		assert!(
			wait_until(Duration::from_secs(1), || async {
				!runtime.sync.is_open("file:///test.rs".to_string()).await
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
		t2.buffer_sync_request(RequestPayload::BufferSyncOpen {
			uri: "file:///test.rs".to_string(),
			text: "content 2".to_string(),
			version_hint: Some(10),
		})
		.await
		.expect("buffer sync open");

		// Wait for broker to register doc
		assert!(
			wait_until(Duration::from_secs(1), || async {
				runtime.sync.is_open("file:///test.rs".to_string()).await
			})
			.await
		);

		let did_change: AnyNotification = serde_json::from_value(serde_json::json!({
			"method": "textDocument/didChange",
			"params": {
				"textDocument": {
					"uri": "file:///test.rs",
					"version": 11
				},
				"contentChanges": [{"text": "content 2 updated"}]
			}
		}))
		.unwrap();
		LspTransport::notify(t2.as_ref(), s2.id, did_change)
			.await
			.expect("t2 notify change");

		// Verify server received second didOpen and didChange
		assert!(
			wait_until(Duration::from_secs(1), || async {
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
			.await
		);

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
		LspTransport::notify(t2.as_ref(), s2.id, did_open)
			.await
			.expect("t2 notify");

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

		// Verify NO didClose reached server (since T2 still has it open)
		tokio::time::sleep(Duration::from_millis(100)).await;
		{
			let received = handle.received.lock().unwrap();
			assert!(!received.iter().any(
				|m| matches!(m, Message::Notification(n) if n.method == "textDocument/didClose")
			));
		}

		// 3. Session 2 should now be able to send changes (takeover)
		let did_change: AnyNotification = serde_json::from_value(serde_json::json!({
			"method": "textDocument/didChange",
			"params": {
				"textDocument": {
					"uri": "file:///test.rs",
					"version": 2
				},
				"contentChanges": [{"text": "session 2 update"}]
			}
		}))
		.unwrap();
		LspTransport::notify(t2.as_ref(), s2.id, did_change)
			.await
			.expect("t2 notify change");

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
