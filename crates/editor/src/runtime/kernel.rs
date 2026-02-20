use std::array;
use std::collections::VecDeque;
use std::time::Instant;

use super::RuntimeEvent;
use super::protocol::{LoopDirectiveV2, RuntimeCauseId, RuntimeEventEnvelope, RuntimeEventSource};

/// Runtime event coordinator state.
///
/// Owns submission queues and sequence generation used by the event-driven
/// runtime API (`submit_event`, `poll_directive`, `drain_until_idle`).
#[derive(Debug, Default)]
pub(crate) struct RuntimeKernel {
	seq_next: u64,
	cause_next: u64,
	frontend_events: VecDeque<RuntimeEventEnvelope>,
	directives: VecDeque<LoopDirectiveV2>,
}

impl RuntimeKernel {
	fn next_seq(&mut self) -> u64 {
		let seq = self.seq_next;
		self.seq_next = self.seq_next.wrapping_add(1);
		seq
	}

	fn next_cause_id(&mut self) -> RuntimeCauseId {
		let cause = RuntimeCauseId(self.cause_next);
		self.cause_next = self.cause_next.wrapping_add(1);
		cause
	}

	pub(crate) fn enqueue_frontend(&mut self, event: RuntimeEvent, source: RuntimeEventSource) -> (u64, RuntimeCauseId) {
		let seq = self.next_seq();
		let cause_id = self.next_cause_id();
		self.frontend_events.push_back(RuntimeEventEnvelope {
			seq,
			cause_id,
			submitted_at: Instant::now(),
			source,
			event,
		});
		(seq, cause_id)
	}

	pub(crate) fn pop_frontend(&mut self) -> Option<RuntimeEventEnvelope> {
		self.frontend_events.pop_front()
	}

	pub(crate) fn peek_frontend(&self) -> Option<&RuntimeEventEnvelope> {
		self.frontend_events.front()
	}

	pub(crate) fn push_directive(&mut self, directive: LoopDirectiveV2) {
		self.directives.push_back(directive);
	}

	pub(crate) fn pop_directive(&mut self) -> Option<LoopDirectiveV2> {
		self.directives.pop_front()
	}

	pub(crate) fn pending_event_count(&self) -> usize {
		self.frontend_events.len()
	}

	pub(crate) fn pending_event_count_by_source(&self) -> [usize; 3] {
		let mut counts = array::from_fn(|_| 0usize);
		for envelope in &self.frontend_events {
			let idx = envelope.source.idx();
			counts[idx] = counts[idx].saturating_add(1);
		}
		counts
	}
}
