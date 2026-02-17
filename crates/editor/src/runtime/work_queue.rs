//! Unified runtime work queue for deferred execution on the pump.
//!
//! This queue replaces split deferred mechanisms (overlay commit counters,
//! workspace edit queues, and invocation mailboxes) with one FIFO queue.

use std::collections::VecDeque;

use crate::Editor;
use crate::types::{Invocation, InvocationPolicy};

/// Deferred invocation origin used for policy and diagnostics decisions.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuntimeWorkSource {
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
pub enum WorkExecutionPolicy {
	/// Use log-only invocation policy and unknown-command notification handling.
	LogOnlyCommandPath,
	/// Use enforcing invocation policy and Nu pipeline disposition handling.
	EnforcingNuPipeline,
}

impl WorkExecutionPolicy {
	pub const fn invocation_policy(self) -> InvocationPolicy {
		match self {
			Self::LogOnlyCommandPath => InvocationPolicy::log_only(),
			Self::EnforcingNuPipeline => InvocationPolicy::enforcing(),
		}
	}
}

/// Runtime work scope tag used for targeted queue clearing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WorkScope {
	/// Default scope for regular deferred work.
	Global,
	/// Nu stop-propagation scope generation.
	NuStopScope(u64),
}

/// Invocation payload queued as runtime work.
#[derive(Debug, Clone)]
pub struct QueuedInvocation {
	pub invocation: Invocation,
	pub source: RuntimeWorkSource,
	pub execution: WorkExecutionPolicy,
}

/// Deferred runtime work item payload.
#[derive(Debug, Clone)]
pub enum RuntimeWorkKind {
	/// Deferred invocation to execute with source-aware policy.
	Invocation(QueuedInvocation),
	/// Deferred overlay commit request.
	OverlayCommit,
	/// Deferred workspace edit to apply on pump.
	#[cfg(feature = "lsp")]
	WorkspaceEdit(xeno_lsp::lsp_types::WorkspaceEdit),
}

/// Queue entry carrying sequence and scope metadata.
#[derive(Debug, Clone)]
pub struct RuntimeWorkItem {
	pub kind: RuntimeWorkKind,
	pub scope: WorkScope,
	pub seq: u64,
}

/// FIFO queue for deferred runtime work.
#[derive(Debug, Default)]
pub struct RuntimeWorkQueue {
	seq_next: u64,
	queue: VecDeque<RuntimeWorkItem>,
}

impl RuntimeWorkQueue {
	/// Enqueues one runtime work item and returns its sequence number.
	pub fn enqueue(&mut self, kind: RuntimeWorkKind, scope: WorkScope) -> u64 {
		let seq = self.seq_next;
		self.seq_next = self.seq_next.wrapping_add(1);
		self.queue.push_back(RuntimeWorkItem { kind, scope, seq });
		seq
	}

	/// Enqueues one deferred invocation item and returns its sequence number.
	pub fn enqueue_invocation(&mut self, invocation: Invocation, source: RuntimeWorkSource, execution: WorkExecutionPolicy, scope: WorkScope) -> u64 {
		self.enqueue(RuntimeWorkKind::Invocation(QueuedInvocation { invocation, source, execution }), scope)
	}

	/// Enqueues one deferred overlay commit item and returns its sequence number.
	pub fn enqueue_overlay_commit(&mut self) -> u64 {
		self.enqueue(RuntimeWorkKind::OverlayCommit, WorkScope::Global)
	}

	/// Enqueues one deferred workspace edit item and returns its sequence number.
	#[cfg(feature = "lsp")]
	pub fn enqueue_workspace_edit(&mut self, edit: xeno_lsp::lsp_types::WorkspaceEdit) -> u64 {
		self.enqueue(RuntimeWorkKind::WorkspaceEdit(edit), WorkScope::Global)
	}

	/// Pops the next work item in FIFO order.
	pub fn pop_front(&mut self) -> Option<RuntimeWorkItem> {
		self.queue.pop_front()
	}

	/// Returns queued item count.
	pub fn len(&self) -> usize {
		self.queue.len()
	}

	/// Returns true when queue is empty.
	pub fn is_empty(&self) -> bool {
		self.queue.is_empty()
	}

	/// Returns true when at least one overlay commit item is queued.
	pub fn has_overlay_commit(&self) -> bool {
		self.queue.iter().any(|item| matches!(item.kind, RuntimeWorkKind::OverlayCommit))
	}

	/// Returns number of queued workspace edit items.
	#[cfg(feature = "lsp")]
	pub fn pending_workspace_edits(&self) -> usize {
		self.queue.iter().filter(|item| matches!(item.kind, RuntimeWorkKind::WorkspaceEdit(_))).count()
	}

	/// Removes queued items matching the scope tag.
	pub fn remove_scope(&mut self, scope: WorkScope) -> usize {
		let before = self.queue.len();
		self.queue.retain(|item| item.scope != scope);
		before.saturating_sub(self.queue.len())
	}
}

impl Editor {
	/// Enqueues one deferred invocation as runtime work.
	pub(crate) fn enqueue_runtime_invocation_work(
		&mut self,
		invocation: Invocation,
		source: RuntimeWorkSource,
		execution: WorkExecutionPolicy,
		scope: WorkScope,
	) -> u64 {
		self.state.runtime_work_queue_mut().enqueue_invocation(invocation, source, execution, scope)
	}

	/// Enqueues one deferred overlay commit as runtime work.
	pub(crate) fn enqueue_runtime_overlay_commit_work(&mut self) -> u64 {
		self.state.runtime_work_queue_mut().enqueue_overlay_commit()
	}

	/// Enqueues one deferred workspace edit as runtime work.
	#[cfg(feature = "lsp")]
	pub(crate) fn enqueue_runtime_workspace_edit_work(&mut self, edit: xeno_lsp::lsp_types::WorkspaceEdit) -> u64 {
		self.state.runtime_work_queue_mut().enqueue_workspace_edit(edit)
	}

	/// Pops one deferred runtime work item in FIFO order.
	pub(crate) fn pop_runtime_work(&mut self) -> Option<RuntimeWorkItem> {
		self.state.runtime_work_queue_mut().pop_front()
	}

	/// Removes deferred runtime work matching a specific scope.
	pub(crate) fn clear_runtime_work_scope(&mut self, scope: WorkScope) -> usize {
		self.state.runtime_work_queue_mut().remove_scope(scope)
	}

	/// Returns deferred runtime work queue length.
	pub(crate) fn runtime_work_len(&self) -> usize {
		self.state.runtime_work_queue().len()
	}

	/// Returns true when at least one overlay commit work item is queued.
	#[cfg(test)]
	pub(crate) fn has_runtime_overlay_commit_work(&self) -> bool {
		self.state.runtime_work_queue().has_overlay_commit()
	}

	/// Returns queued workspace edit work count.
	#[cfg(all(feature = "lsp", test))]
	pub(crate) fn pending_runtime_workspace_edit_work(&self) -> usize {
		self.state.runtime_work_queue().pending_workspace_edits()
	}

	/// Returns true when deferred runtime work queue is empty.
	#[cfg(test)]
	pub(crate) fn runtime_work_is_empty(&self) -> bool {
		self.state.runtime_work_queue().is_empty()
	}
}

#[cfg(test)]
mod tests {
	use super::{RuntimeWorkQueue, RuntimeWorkSource, WorkExecutionPolicy, WorkScope};
	use crate::types::Invocation;

	#[test]
	fn fifo_order_is_stable_across_work_kinds() {
		let mut queue = RuntimeWorkQueue::default();
		let seq0 = queue.enqueue_invocation(
			Invocation::command("one", Vec::new()),
			RuntimeWorkSource::ActionEffect,
			WorkExecutionPolicy::LogOnlyCommandPath,
			WorkScope::Global,
		);
		let seq1 = queue.enqueue_overlay_commit();
		let seq2 = queue.enqueue_invocation(
			Invocation::command("two", Vec::new()),
			RuntimeWorkSource::Overlay,
			WorkExecutionPolicy::LogOnlyCommandPath,
			WorkScope::Global,
		);

		let first = queue.pop_front().expect("first item should exist");
		let second = queue.pop_front().expect("second item should exist");
		let third = queue.pop_front().expect("third item should exist");

		assert_eq!(first.seq, seq0);
		assert_eq!(second.seq, seq1);
		assert_eq!(third.seq, seq2);
		assert!(queue.is_empty());
	}

	#[test]
	fn remove_scope_prunes_matching_scope_only() {
		let mut queue = RuntimeWorkQueue::default();
		queue.enqueue_invocation(
			Invocation::command("keep", Vec::new()),
			RuntimeWorkSource::Overlay,
			WorkExecutionPolicy::LogOnlyCommandPath,
			WorkScope::Global,
		);
		queue.enqueue_invocation(
			Invocation::command("drop", Vec::new()),
			RuntimeWorkSource::NuHookDispatch,
			WorkExecutionPolicy::EnforcingNuPipeline,
			WorkScope::NuStopScope(11),
		);
		queue.enqueue_invocation(
			Invocation::command("drop2", Vec::new()),
			RuntimeWorkSource::NuScheduledMacro,
			WorkExecutionPolicy::EnforcingNuPipeline,
			WorkScope::NuStopScope(11),
		);

		let removed = queue.remove_scope(WorkScope::NuStopScope(11));
		assert_eq!(removed, 2);
		assert_eq!(queue.len(), 1);
	}
}
