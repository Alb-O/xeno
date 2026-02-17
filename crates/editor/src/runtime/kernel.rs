use std::collections::VecDeque;

use super::RuntimeEvent;
use super::protocol::{ExternalEventEnvelope, ExternalEventKind, LoopDirectiveV2, RuntimeEventEnvelope, RuntimeEventSource};

/// Runtime event coordinator state.
///
/// Owns submission queues and sequence generation used by the event-driven
/// runtime API (`submit_event`, `poll_directive`, `drain_until_idle`).
#[derive(Debug, Default)]
pub(crate) struct RuntimeKernel {
	seq_next: u64,
	frontend_events: VecDeque<RuntimeEventEnvelope>,
	external_events: VecDeque<ExternalEventEnvelope>,
	directives: VecDeque<LoopDirectiveV2>,
}

impl RuntimeKernel {
	fn next_seq(&mut self) -> u64 {
		let seq = self.seq_next;
		self.seq_next = self.seq_next.wrapping_add(1);
		seq
	}

	pub(crate) fn enqueue_frontend(&mut self, event: RuntimeEvent, source: RuntimeEventSource) -> u64 {
		let seq = self.next_seq();
		self.frontend_events.push_back(RuntimeEventEnvelope { seq, source, event });
		seq
	}

	pub(crate) fn enqueue_external(&mut self, kind: ExternalEventKind) -> u64 {
		let seq = self.next_seq();
		self.external_events.push_back(ExternalEventEnvelope { seq, kind });
		seq
	}

	pub(crate) fn pop_frontend(&mut self) -> Option<RuntimeEventEnvelope> {
		self.frontend_events.pop_front()
	}

	pub(crate) fn pop_external(&mut self) -> Option<ExternalEventEnvelope> {
		self.external_events.pop_front()
	}

	pub(crate) fn push_directive(&mut self, directive: LoopDirectiveV2) {
		self.directives.push_back(directive);
	}

	pub(crate) fn pop_directive(&mut self) -> Option<LoopDirectiveV2> {
		self.directives.pop_front()
	}

	pub(crate) fn pending_event_count(&self) -> usize {
		self.frontend_events.len().saturating_add(self.external_events.len())
	}
}
