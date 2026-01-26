use std::time::Duration;

use lsp_types::{CompletionContext, CompletionResponse, CompletionTriggerKind, Position, Uri};
use tokio::time::sleep;
use tokio_util::sync::CancellationToken;

use crate::ClientHandle;

pub enum CompletionTrigger {
	Typing,
	Manual,
}

impl CompletionTrigger {
	pub fn debounce(&self) -> Duration {
		match self {
			CompletionTrigger::Typing => Duration::from_millis(80),
			CompletionTrigger::Manual => Duration::ZERO,
		}
	}
}

pub struct CompletionRequest<T> {
	pub id: T,
	pub replace_start: usize,
	pub client: ClientHandle,
	pub uri: Uri,
	pub position: Position,
	pub debounce: Duration,
	pub trigger_kind: CompletionTriggerKind,
	pub trigger_character: Option<String>,
}

pub struct CompletionController {
	generation: u64,
	in_flight: Option<InFlightCompletion>,
}

struct InFlightCompletion {
	cancel: CancellationToken,
}

impl Default for CompletionController {
	fn default() -> Self {
		Self::new()
	}
}

impl CompletionController {
	pub fn new() -> Self {
		Self {
			generation: 0,
			in_flight: None,
		}
	}

	pub fn generation(&self) -> u64 {
		self.generation
	}

	pub fn cancel(&mut self) {
		if let Some(in_flight) = self.in_flight.take() {
			in_flight.cancel.cancel();
		}
	}

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
		self.in_flight = Some(InFlightCompletion {
			cancel: cancel.clone(),
		});

		tokio::spawn(async move {
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
