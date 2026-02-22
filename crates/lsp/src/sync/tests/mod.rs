use std::path::Path;
use std::sync::Arc;

use async_trait::async_trait;
use lsp_types::{Diagnostic, DiagnosticSeverity, Range, Uri};
use ropey::Rope;
use tokio::sync::{mpsc, oneshot};

use super::*;

mod barriers;
mod change_failures;
mod code_actions;
mod core;
mod formatting;
mod inlay_hints;
mod invariants;
mod lifecycle;
mod navigation;
mod pull_diagnostics;
mod rename_refs;
mod resource_ops;

struct SimpleStubTransport;
#[async_trait]
impl crate::client::transport::LspTransport for SimpleStubTransport {
	fn subscribe_events(&self) -> crate::Result<mpsc::UnboundedReceiver<crate::client::transport::TransportEvent>> {
		let (_, rx) = mpsc::unbounded_channel();
		Ok(rx)
	}
	async fn start(&self, _cfg: crate::client::ServerConfig) -> crate::Result<crate::client::transport::StartedServer> {
		Ok(crate::client::transport::StartedServer {
			id: LanguageServerId::new(1, 0),
		})
	}
	async fn notify(&self, _server: LanguageServerId, _notif: crate::AnyNotification) -> crate::Result<()> {
		Ok(())
	}
	async fn notify_with_barrier(&self, _server: LanguageServerId, _notif: crate::AnyNotification) -> crate::Result<oneshot::Receiver<crate::Result<()>>> {
		let (tx, rx) = oneshot::channel();
		let _ = tx.send(Ok(()));
		Ok(rx)
	}
	async fn request(&self, _server: LanguageServerId, _req: crate::AnyRequest, _timeout: Option<std::time::Duration>) -> crate::Result<crate::AnyResponse> {
		Err(crate::Error::Protocol("SimpleStubTransport".into()))
	}
	async fn reply(
		&self,
		_server: LanguageServerId,
		_id: crate::RequestId,
		_resp: std::result::Result<crate::JsonValue, crate::ResponseError>,
	) -> crate::Result<()> {
		Ok(())
	}
	async fn stop(&self, _server: LanguageServerId) -> crate::Result<()> {
		Ok(())
	}
}

/// Recorded outbound message (notification or request).
#[derive(Debug, Clone)]
struct RecordedMessage {
	server_id: LanguageServerId,
	method: String,
	uri: Option<String>,
	_is_request: bool,
}

/// Transport that records notification/request methods, server ids, and URIs in order.
/// Methods listed in `fail_methods` will return an error instead of succeeding.
struct RecordingTransport {
	messages: std::sync::Mutex<Vec<RecordedMessage>>,
	next_slot: std::sync::atomic::AtomicU32,
	fail_methods: std::sync::Mutex<std::collections::HashSet<String>>,
	/// Canned JSON responses keyed by request method name.
	request_responses: std::sync::Mutex<std::collections::HashMap<String, crate::JsonValue>>,
}

impl RecordingTransport {
	fn new() -> Self {
		Self {
			messages: std::sync::Mutex::new(Vec::new()),
			next_slot: std::sync::atomic::AtomicU32::new(1),
			fail_methods: std::sync::Mutex::new(std::collections::HashSet::new()),
			request_responses: std::sync::Mutex::new(std::collections::HashMap::new()),
		}
	}

	fn set_fail_method(&self, method: &str) {
		self.fail_methods.lock().unwrap().insert(method.to_string());
	}

	fn clear_fail_method(&self, method: &str) {
		self.fail_methods.lock().unwrap().remove(method);
	}

	fn set_request_response(&self, method: &str, response: crate::JsonValue) {
		self.request_responses.lock().unwrap().insert(method.to_string(), response);
	}

	fn recorded(&self) -> Vec<RecordedMessage> {
		self.messages.lock().unwrap().clone()
	}

	fn recorded_methods(&self) -> Vec<String> {
		self.messages.lock().unwrap().iter().map(|n| n.method.clone()).collect()
	}

	fn record_notification(&self, server_id: LanguageServerId, notif: &crate::AnyNotification) -> crate::Result<()> {
		let uri = notif
			.params
			.get("textDocument")
			.and_then(|td| td.get("uri"))
			.and_then(|u| u.as_str())
			.map(|s| s.to_string());

		self.messages.lock().unwrap().push(RecordedMessage {
			server_id,
			method: notif.method.clone(),
			uri,
			_is_request: false,
		});
		if self.fail_methods.lock().unwrap().contains(&notif.method) {
			return Err(crate::Error::Protocol(format!("injected failure for {}", notif.method)));
		}
		Ok(())
	}

	fn record_request(&self, server_id: LanguageServerId, req: &crate::AnyRequest) {
		self.messages.lock().unwrap().push(RecordedMessage {
			server_id,
			method: req.method.clone(),
			uri: None,
			_is_request: true,
		});
	}
}

#[async_trait]
impl crate::client::transport::LspTransport for RecordingTransport {
	fn subscribe_events(&self) -> crate::Result<mpsc::UnboundedReceiver<crate::client::transport::TransportEvent>> {
		let (_, rx) = mpsc::unbounded_channel();
		Ok(rx)
	}
	async fn start(&self, _cfg: crate::client::ServerConfig) -> crate::Result<crate::client::transport::StartedServer> {
		let slot = self.next_slot.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
		Ok(crate::client::transport::StartedServer {
			id: LanguageServerId::new(slot, 0),
		})
	}
	async fn notify(&self, server: LanguageServerId, notif: crate::AnyNotification) -> crate::Result<()> {
		self.record_notification(server, &notif)?;
		Ok(())
	}
	async fn notify_with_barrier(&self, server: LanguageServerId, notif: crate::AnyNotification) -> crate::Result<oneshot::Receiver<crate::Result<()>>> {
		self.record_notification(server, &notif)?;
		let (tx, rx) = oneshot::channel();
		let _ = tx.send(Ok(()));
		Ok(rx)
	}
	async fn request(&self, server: LanguageServerId, req: crate::AnyRequest, _timeout: Option<std::time::Duration>) -> crate::Result<crate::AnyResponse> {
		self.record_request(server, &req);
		if let Some(response) = self.request_responses.lock().unwrap().get(&req.method).cloned() {
			return Ok(crate::AnyResponse::new_ok(req.id, response));
		}
		Err(crate::Error::Protocol("RecordingTransport: no canned response".into()))
	}
	async fn reply(
		&self,
		_server: LanguageServerId,
		_id: crate::RequestId,
		_resp: std::result::Result<crate::JsonValue, crate::ResponseError>,
	) -> crate::Result<()> {
		Ok(())
	}
	async fn stop(&self, _server: LanguageServerId) -> crate::Result<()> {
		Ok(())
	}
}

/// Transport that handles initialize requests and records notifications.
/// Combines RecordingTransport's recording with initialize capability.
struct InitRecordingTransport {
	inner: RecordingTransport,
}

impl InitRecordingTransport {
	fn new() -> Self {
		Self {
			inner: RecordingTransport::new(),
		}
	}

	fn set_fail_method(&self, method: &str) {
		self.inner.set_fail_method(method);
	}

	fn with_capabilities(capabilities: lsp_types::ServerCapabilities) -> Self {
		let t = Self::new();
		t.inner.set_request_response(
			"initialize",
			serde_json::to_value(lsp_types::InitializeResult {
				capabilities,
				server_info: None,
			})
			.unwrap(),
		);
		t
	}
}

#[async_trait]
impl crate::client::transport::LspTransport for InitRecordingTransport {
	fn subscribe_events(&self) -> crate::Result<mpsc::UnboundedReceiver<crate::client::transport::TransportEvent>> {
		self.inner.subscribe_events()
	}
	async fn start(&self, cfg: crate::client::ServerConfig) -> crate::Result<crate::client::transport::StartedServer> {
		self.inner.start(cfg).await
	}
	async fn notify(&self, server: LanguageServerId, notif: crate::AnyNotification) -> crate::Result<()> {
		self.inner.notify(server, notif).await
	}
	async fn notify_with_barrier(&self, server: LanguageServerId, notif: crate::AnyNotification) -> crate::Result<oneshot::Receiver<crate::Result<()>>> {
		self.inner.notify_with_barrier(server, notif).await
	}
	async fn request(&self, server: LanguageServerId, req: crate::AnyRequest, _timeout: Option<std::time::Duration>) -> crate::Result<crate::AnyResponse> {
		self.inner.record_request(server, &req);
		// Check inner's canned responses first (includes initialize).
		if let Some(response) = self.inner.request_responses.lock().unwrap().get(&req.method).cloned() {
			return Ok(crate::AnyResponse::new_ok(req.id, response));
		}
		// Default: return default InitializeResult for initialize.
		if req.method == "initialize" {
			return Ok(crate::AnyResponse::new_ok(
				req.id,
				serde_json::to_value(lsp_types::InitializeResult {
					capabilities: lsp_types::ServerCapabilities::default(),
					server_info: None,
				})
				.unwrap(),
			));
		}
		Err(crate::Error::Protocol(format!("InitRecordingTransport: no handler for {}", req.method)))
	}
	async fn reply(
		&self,
		_server: LanguageServerId,
		_id: crate::RequestId,
		_resp: std::result::Result<crate::JsonValue, crate::ResponseError>,
	) -> crate::Result<()> {
		Ok(())
	}
	async fn stop(&self, _server: LanguageServerId) -> crate::Result<()> {
		Ok(())
	}
}
