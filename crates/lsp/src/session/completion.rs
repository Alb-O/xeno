use std::time::Duration;

use lsp_types::{CompletionContext, CompletionResponse, CompletionTriggerKind, Position, Uri};
use tokio::time::sleep;
use tokio_util::sync::CancellationToken;

use crate::ClientHandle;

/// The kind of trigger for a completion request.
pub enum CompletionTrigger {
	/// Triggered by user typing.
	Typing,
	/// Triggered manually (e.g. by shortcut).
	Manual,
}

impl CompletionTrigger {
	/// Returns the debounce duration for this trigger kind.
	pub fn debounce(&self) -> Duration {
		match self {
			CompletionTrigger::Typing => Duration::from_millis(80),
			CompletionTrigger::Manual => Duration::ZERO,
		}
	}
}

/// A request for code completion.
pub struct CompletionRequest<T> {
	/// Identifier for the request (e.g. buffer ID).
	pub id: T,
	/// The start index for replacement.
	pub replace_start: usize,
	/// The LSP client handle.
	pub client: ClientHandle,
	/// The document URI.
	pub uri: Uri,
	/// The cursor position.
	pub position: Position,
	/// Debounce duration.
	pub debounce: Duration,
	/// The kind of completion trigger.
	pub trigger_kind: CompletionTriggerKind,
	/// The trigger character, if any.
	pub trigger_character: Option<String>,
}

/// Controller for managing completion requests, including debouncing and cancellation.
pub struct CompletionController {
	generation: u64,
	in_flight: Option<InFlightCompletion>,
	worker_runtime: xeno_worker::WorkerRuntime,
}

struct InFlightCompletion {
	cancel: CancellationToken,
}

impl Default for CompletionController {
	fn default() -> Self {
		Self::new(xeno_worker::WorkerRuntime::new())
	}
}

impl CompletionController {
	/// Creates a new completion controller.
	pub fn new(worker_runtime: xeno_worker::WorkerRuntime) -> Self {
		Self {
			generation: 0,
			in_flight: None,
			worker_runtime,
		}
	}

	/// Returns the current generation of completion requests.
	pub fn generation(&self) -> u64 {
		self.generation
	}

	/// Cancels any in-flight completion request.
	pub fn cancel(&mut self) {
		if let Some(in_flight) = self.in_flight.take() {
			in_flight.cancel.cancel();
		}
	}

	/// Triggers a new completion request.
	///
	/// This will cancel any existing in-flight request and spawn a new task.
	pub fn trigger<T, F>(&mut self, request: CompletionRequest<T>, callback: F)
	where
		T: Send + 'static,
		F: FnOnce(u64, T, usize, Option<CompletionResponse>) + Send + 'static,
	{
		self.generation = self.generation.wrapping_add(1);
		let generation = self.generation;
		if let Some(in_flight) = self.in_flight.take() {
			in_flight.cancel.cancel();
		}

		let cancel = CancellationToken::new();
		self.in_flight = Some(InFlightCompletion { cancel: cancel.clone() });

		self.worker_runtime.spawn(xeno_worker::TaskClass::Background, async move {
			if request.debounce > Duration::ZERO {
				tokio::select! {
					_ = cancel.cancelled() => return,
					_ = sleep(request.debounce) => {}
				}
			} else if cancel.is_cancelled() {
				return;
			}

			let response = request
				.client
				.completion(
					request.uri,
					request.position,
					Some(CompletionContext {
						trigger_kind: request.trigger_kind,
						trigger_character: request.trigger_character.clone(),
					}),
				)
				.await
				.ok()
				.flatten();

			if cancel.is_cancelled() {
				return;
			}

			callback(generation, request.id, request.replace_start, response);
		});
	}
}
