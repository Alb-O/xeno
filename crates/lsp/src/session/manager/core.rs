use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use parking_lot::Mutex;
use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;

use super::*;

/// Errors returned when starting [`LspRuntime`].
#[derive(Debug, thiserror::Error)]
pub enum RuntimeStartError {
	/// Runtime start was requested more than once.
	#[error("runtime already started")]
	AlreadyStarted,
	/// Runtime start was called outside of a Tokio runtime.
	#[error("no tokio runtime available")]
	NoRuntime,
	/// Transport event subscription failed.
	#[error("failed to subscribe transport events: {0}")]
	EventSubscription(crate::Error),
}

struct RuntimeState {
	cancel: CancellationToken,
	router_actor: Option<Arc<xeno_worker::ActorHandle<TransportEvent, ()>>>,
	forward_task: Option<JoinHandle<()>>,
}

/// Lifecycle owner for the LSP transport event router.
///
/// The runtime starts the router exactly once and can be shut down
/// independently from [`LspSession`] state.
pub struct LspRuntime {
	sync: DocumentSync,
	transport: Arc<dyn LspTransport>,
	worker_runtime: xeno_worker::WorkerRuntime,
	started: AtomicBool,
	state: Mutex<RuntimeState>,
}

impl LspRuntime {
	fn new(sync: DocumentSync, transport: Arc<dyn LspTransport>, worker_runtime: xeno_worker::WorkerRuntime) -> Self {
		Self {
			sync,
			transport,
			worker_runtime,
			started: AtomicBool::new(false),
			state: Mutex::new(RuntimeState {
				cancel: CancellationToken::new(),
				router_actor: None,
				forward_task: None,
			}),
		}
	}

	/// Start the background event router.
	///
	/// Must be called from within a Tokio runtime.
	pub fn start(&self) -> Result<(), RuntimeStartError> {
		if tokio::runtime::Handle::try_current().is_err() {
			return Err(RuntimeStartError::NoRuntime);
		}

		if self.started.swap(true, Ordering::SeqCst) {
			return Err(RuntimeStartError::AlreadyStarted);
		}

		let mut events_rx = match self.transport.subscribe_events() {
			Ok(rx) => rx,
			Err(err) => {
				self.started.store(false, Ordering::SeqCst);
				return Err(RuntimeStartError::EventSubscription(err));
			}
		};

		let sync = self.sync.clone();
		let transport = self.transport.clone();
		let documents = self.sync.documents_arc();
		let cancel = self.state.lock().cancel.clone();
		let router_actor = Arc::new(
			self.worker_runtime.actor(
				xeno_worker::ActorSpec::new("lsp.runtime.router", xeno_worker::TaskClass::Interactive, move || RouterActor {
					sync: sync.clone(),
					transport: transport.clone(),
					documents: Arc::clone(&documents),
				})
				.supervisor(xeno_worker::SupervisorSpec {
					restart: xeno_worker::RestartPolicy::Never,
					event_buffer: 8,
				}),
			),
		);
		let router_for_forward = Arc::clone(&router_actor);

		let task = self.worker_runtime.spawn(xeno_worker::TaskClass::Background, async move {
			loop {
				tokio::select! {
					_ = cancel.cancelled() => break,
					maybe_event = events_rx.recv() => {
						let Some(event) = maybe_event else {
							break;
						};
						if router_for_forward.send(event).await.is_err() {
							break;
						}
					}
				}
			}
		});

		let mut state = self.state.lock();
		state.router_actor = Some(router_actor);
		state.forward_task = Some(task);
		Ok(())
	}

	/// Stop the event router and wait for the task to exit.
	pub async fn shutdown(&self) {
		let (cancel, router_actor, task) = {
			let mut state = self.state.lock();
			(state.cancel.clone(), state.router_actor.take(), state.forward_task.take())
		};

		cancel.cancel();

		if let Some(actor) = router_actor {
			let _ = actor.shutdown_graceful_or_force(Duration::from_secs(2)).await;
		}
		if let Some(task) = task {
			let _ = task.await;
		}
	}

	/// Returns whether the runtime has been started.
	pub fn is_started(&self) -> bool {
		self.started.load(Ordering::Acquire)
	}
}

struct RouterActor {
	sync: DocumentSync,
	transport: Arc<dyn LspTransport>,
	documents: Arc<DocumentStateManager>,
}

#[async_trait::async_trait]
impl xeno_worker::WorkerActor for RouterActor {
	type Cmd = TransportEvent;
	type Evt = ();

	async fn handle(&mut self, event: Self::Cmd, _ctx: &mut xeno_worker::ActorContext<Self::Evt>) -> Result<xeno_worker::ActorFlow, String> {
		let keep_running = process_transport_event(&self.sync, self.documents.as_ref(), self.transport.as_ref(), event).await;
		Ok(if keep_running {
			xeno_worker::ActorFlow::Continue
		} else {
			xeno_worker::ActorFlow::Stop
		})
	}
}

/// High-level LSP session surface used by editor integration.
pub struct LspSession {
	sync: DocumentSync,
	diagnostics_receiver: Option<DiagnosticsEventReceiver>,
	transport: Arc<dyn LspTransport>,
}

impl LspSession {
	/// Create a new session and runtime pair from a transport.
	pub fn new(transport: Arc<dyn LspTransport>, worker_runtime: xeno_worker::WorkerRuntime) -> (Self, LspRuntime) {
		let (sync, _registry, _documents, diagnostics_receiver) = DocumentSync::create(transport.clone(), worker_runtime.clone());
		let runtime = LspRuntime::new(sync.clone(), transport.clone(), worker_runtime);
		(
			Self {
				sync,
				diagnostics_receiver: Some(diagnostics_receiver),
				transport,
			},
			runtime,
		)
	}

	/// Create a session and runtime pair from existing registry/doc state.
	pub fn with_state(registry: Arc<Registry>, documents: Arc<DocumentStateManager>, worker_runtime: xeno_worker::WorkerRuntime) -> (Self, LspRuntime) {
		let transport = registry.transport();
		let sync = DocumentSync::with_registry(registry, documents);
		let runtime = LspRuntime::new(sync.clone(), transport.clone(), worker_runtime);
		(
			Self {
				sync,
				diagnostics_receiver: None,
				transport,
			},
			runtime,
		)
	}

	/// Poll for pending diagnostic events.
	pub fn poll_diagnostics(&mut self) -> Vec<DiagnosticsEvent> {
		let Some(ref mut receiver) = self.diagnostics_receiver else {
			return Vec::new();
		};

		let mut events = Vec::new();
		while let Ok(event) = receiver.try_recv() {
			events.push(event);
		}
		events
	}

	/// Get the diagnostics version counter.
	pub fn diagnostics_version(&self) -> u64 {
		self.sync.documents().diagnostics_version()
	}

	/// Configure a language server.
	pub fn configure_server(&self, language: impl Into<String>, config: LanguageServerConfig) {
		self.sync.registry().register(language, config);
	}

	/// Remove a language server configuration.
	pub fn remove_server(&self, language: &str) {
		self.sync.registry().unregister(language);
	}

	/// Get the document sync coordinator.
	pub fn sync(&self) -> &DocumentSync {
		&self.sync
	}

	/// Get the server registry.
	pub fn registry(&self) -> &Registry {
		self.sync.registry()
	}

	/// Get the document state manager.
	pub fn documents(&self) -> &DocumentStateManager {
		self.sync.documents()
	}

	/// Shutdown all language servers.
	pub async fn shutdown_all(&self) {
		let ids = self.sync.registry().shutdown_all();
		for id in ids {
			let _ = self.transport.stop(id).await;
		}
	}
}

fn event_server_id(event: &TransportEvent) -> Option<crate::client::LanguageServerId> {
	match event {
		TransportEvent::Diagnostics { server, .. } => Some(*server),
		TransportEvent::Message { server, .. } => Some(*server),
		TransportEvent::Status { server, .. } => Some(*server),
		TransportEvent::Disconnected => None,
	}
}

async fn process_transport_event(sync: &DocumentSync, documents: &DocumentStateManager, transport: &dyn LspTransport, event: TransportEvent) -> bool {
	if let Some(id) = event_server_id(&event)
		&& !sync.registry().is_current(id)
	{
		tracing::debug!(server_id = %id, "Dropping event from stale server instance");
		return true;
	}

	match event {
		TransportEvent::Diagnostics {
			server: _,
			uri,
			version,
			diagnostics,
		} => process_diagnostics_event(documents, uri, version, diagnostics),
		TransportEvent::Message { server, message } => process_message_event(sync, transport, documents, server, message).await,
		TransportEvent::Status { server, status } => process_status_event(sync, documents, transport, server, status).await,
		TransportEvent::Disconnected => return false,
	}

	true
}

fn process_diagnostics_event(documents: &DocumentStateManager, uri: String, version: Option<u32>, diagnostics: serde_json::Value) {
	let Ok(uri) = uri.parse::<lsp_types::Uri>() else {
		return;
	};
	let Ok(diags) = serde_json::from_value::<Vec<lsp_types::Diagnostic>>(diagnostics) else {
		return;
	};

	documents.update_diagnostics(&uri, diags, version.and_then(|v| i32::try_from(v).ok()));
}

async fn process_message_event(
	sync: &DocumentSync,
	transport: &dyn LspTransport,
	documents: &DocumentStateManager,
	server: crate::client::LanguageServerId,
	message: crate::Message,
) {
	match message {
		crate::Message::Request(req) => {
			tracing::debug!(server_id = %server, method = %req.method, "Handling server request");
			let req_id = req.id.clone();
			let result = crate::session::server_requests::handle_server_request(sync, server, req).await;
			if let Err(err) = transport.reply(server, req_id, result).await {
				tracing::error!(server_id = %server, error = ?err, "Failed to reply to server request");
			}
		}
		crate::Message::Notification(notif) => {
			if notif.method == "$/progress" {
				if let Ok(params) = serde_json::from_value::<lsp_types::ProgressParams>(notif.params) {
					documents.update_progress(server, params);
				}
			} else if notif.method == "window/logMessage" || notif.method == "window/showMessage" {
				tracing::debug!(server_id = %server, method = %notif.method, "Server notification");
			}
		}
		crate::Message::Response(_) => {}
	}
}

async fn process_status_event(
	sync: &DocumentSync,
	documents: &DocumentStateManager,
	transport: &dyn LspTransport,
	server: crate::client::LanguageServerId,
	status: crate::client::transport::TransportStatus,
) {
	use crate::client::transport::TransportStatus;

	match status {
		TransportStatus::Stopped | TransportStatus::Crashed => {
			if let Some(meta) = sync.registry().remove_server(server) {
				tracing::warn!(
					server_id = %server,
					language = %meta.language,
					status = ?status,
					"LSP server stopped, removed from registry"
				);
			}

			let _ = transport.stop(server).await;
			documents.clear_server_progress(server);
		}
		TransportStatus::Starting | TransportStatus::Running => {
			tracing::debug!(server_id = %server, status = ?status, "LSP server status update");
		}
	}
}
