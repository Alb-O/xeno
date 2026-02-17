//! Deferred invocation mailbox model used by runtime pump convergence.
//!
//! # Purpose
//!
//! * Defines the deferred invocation envelope executed by `pump`.
//! * Carries explicit execution policy and scope tags per queued item.
//! * Provides targeted clearing primitives for stop-propagation semantics.
//!
//! # Mental model
//!
//! * Producers enqueue `Invocation` values with metadata:
//!   * source (diagnostics)
//!   * execution policy (log-only command path vs enforcing Nu pipeline path)
//!   * scope tag (global or Nu stop scope)
//! * Runtime pump drains items in FIFO order under a bounded per-round cap.
//! * Stop propagation clears only entries in the matching Nu scope tag.
//!
//! # Invariants
//!
//! * FIFO order must be preserved across all producers.
//! * Execution policy must be attached at enqueue time.
//! * Scope-based clearing must not remove unrelated queued invocations.

use std::collections::VecDeque;

use crate::types::{Invocation, InvocationPolicy};

/// Deferred invocation origin used for policy and diagnostics decisions.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeferredInvocationSource {
	/// Invocation queued by action/app effects.
	ActionEffect,
	/// Invocation queued by overlay controllers.
	Overlay,
	/// Invocation queued by command-ops surfaces.
	CommandOps,
	/// Invocation produced by Nu hook effect dispatch.
	NuHookDispatch,
	/// Invocation produced by Nu scheduled macro timers.
	NuScheduledMacro,
}

/// Deferred invocation execution policy.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeferredInvocationExecutionPolicy {
	/// Use log-only invocation policy and unknown-command notification handling.
	LogOnlyCommandPath,
	/// Use enforcing invocation policy and Nu pipeline disposition handling.
	EnforcingNuPipeline,
}

impl DeferredInvocationExecutionPolicy {
	pub const fn invocation_policy(self) -> InvocationPolicy {
		match self {
			Self::LogOnlyCommandPath => InvocationPolicy::log_only(),
			Self::EnforcingNuPipeline => InvocationPolicy::enforcing(),
		}
	}
}

/// Deferred invocation scope tag used for targeted queue clearing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeferredInvocationScope {
	/// Default scope for regular deferred invocations.
	Global,
	/// Nu stop-propagation scope generation.
	NuStopScope(u64),
}

/// Deferred invocation envelope with mailbox sequence metadata.
#[derive(Debug, Clone)]
pub struct DeferredInvocation {
	pub invocation: Invocation,
	pub source: DeferredInvocationSource,
	pub execution: DeferredInvocationExecutionPolicy,
	pub scope: DeferredInvocationScope,
	pub seq: u64,
}

/// FIFO mailbox for deferred invocation execution.
#[derive(Debug, Default)]
pub struct InvocationMailbox {
	seq_next: u64,
	queue: VecDeque<DeferredInvocation>,
}

impl InvocationMailbox {
	/// Enqueues one invocation and returns its mailbox sequence number.
	pub fn enqueue(
		&mut self,
		invocation: Invocation,
		source: DeferredInvocationSource,
		execution: DeferredInvocationExecutionPolicy,
		scope: DeferredInvocationScope,
	) -> u64 {
		let seq = self.seq_next;
		self.seq_next = self.seq_next.wrapping_add(1);
		self.queue.push_back(DeferredInvocation {
			invocation,
			source,
			execution,
			scope,
			seq,
		});
		seq
	}

	/// Pops the next deferred invocation in FIFO order.
	pub fn pop_front(&mut self) -> Option<DeferredInvocation> {
		self.queue.pop_front()
	}

	/// Returns the number of queued deferred invocations.
	pub fn len(&self) -> usize {
		self.queue.len()
	}

	/// Returns true when no deferred invocations are queued.
	pub fn is_empty(&self) -> bool {
		self.queue.is_empty()
	}

	/// Removes queued deferred invocations matching the source.
	pub fn remove_source(&mut self, source: DeferredInvocationSource) -> usize {
		let before = self.queue.len();
		self.queue.retain(|item| item.source != source);
		before.saturating_sub(self.queue.len())
	}

	/// Removes queued deferred invocations matching the scope tag.
	pub fn remove_scope(&mut self, scope: DeferredInvocationScope) -> usize {
		let before = self.queue.len();
		self.queue.retain(|item| item.scope != scope);
		before.saturating_sub(self.queue.len())
	}
}

#[cfg(test)]
mod tests {
	use super::{DeferredInvocationExecutionPolicy, DeferredInvocationScope, DeferredInvocationSource, InvocationMailbox};
	use crate::types::Invocation;

	#[test]
	fn fifo_order_is_stable() {
		let mut mailbox = InvocationMailbox::default();
		let seq0 = mailbox.enqueue(
			Invocation::command("one", Vec::new()),
			DeferredInvocationSource::ActionEffect,
			DeferredInvocationExecutionPolicy::LogOnlyCommandPath,
			DeferredInvocationScope::Global,
		);
		let seq1 = mailbox.enqueue(
			Invocation::command("two", Vec::new()),
			DeferredInvocationSource::Overlay,
			DeferredInvocationExecutionPolicy::LogOnlyCommandPath,
			DeferredInvocationScope::Global,
		);
		assert!(seq1 > seq0);

		let first = mailbox.pop_front().expect("first invocation should exist");
		let second = mailbox.pop_front().expect("second invocation should exist");
		assert!(matches!(first.invocation, Invocation::Command(_)));
		assert!(matches!(second.invocation, Invocation::Command(_)));
		assert_eq!(first.seq, seq0);
		assert_eq!(second.seq, seq1);
		assert!(mailbox.is_empty());
	}

	#[test]
	fn remove_source_prunes_matching_items_only() {
		let mut mailbox = InvocationMailbox::default();
		mailbox.enqueue(
			Invocation::command("queued", Vec::new()),
			DeferredInvocationSource::NuHookDispatch,
			DeferredInvocationExecutionPolicy::EnforcingNuPipeline,
			DeferredInvocationScope::NuStopScope(1),
		);
		mailbox.enqueue(
			Invocation::command("keep", Vec::new()),
			DeferredInvocationSource::Overlay,
			DeferredInvocationExecutionPolicy::LogOnlyCommandPath,
			DeferredInvocationScope::Global,
		);
		mailbox.enqueue(
			Invocation::command("queued2", Vec::new()),
			DeferredInvocationSource::NuScheduledMacro,
			DeferredInvocationExecutionPolicy::EnforcingNuPipeline,
			DeferredInvocationScope::NuStopScope(2),
		);

		let removed = mailbox.remove_source(DeferredInvocationSource::NuHookDispatch);
		assert_eq!(removed, 1);
		assert_eq!(mailbox.len(), 2);
	}

	#[test]
	fn remove_scope_prunes_matching_scope_only() {
		let mut mailbox = InvocationMailbox::default();
		mailbox.enqueue(
			Invocation::command("keep", Vec::new()),
			DeferredInvocationSource::Overlay,
			DeferredInvocationExecutionPolicy::LogOnlyCommandPath,
			DeferredInvocationScope::Global,
		);
		mailbox.enqueue(
			Invocation::command("drop", Vec::new()),
			DeferredInvocationSource::NuHookDispatch,
			DeferredInvocationExecutionPolicy::EnforcingNuPipeline,
			DeferredInvocationScope::NuStopScope(11),
		);
		mailbox.enqueue(
			Invocation::command("drop2", Vec::new()),
			DeferredInvocationSource::NuScheduledMacro,
			DeferredInvocationExecutionPolicy::EnforcingNuPipeline,
			DeferredInvocationScope::NuStopScope(11),
		);

		let removed = mailbox.remove_scope(DeferredInvocationScope::NuStopScope(11));
		assert_eq!(removed, 2);
		assert_eq!(mailbox.len(), 1);
	}
}
