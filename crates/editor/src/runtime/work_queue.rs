//! Unified runtime work queue for deferred execution in runtime drain phases.
//!
//! This queue replaces split deferred mechanisms (overlay commit counters,
//! workspace edit queues, and invocation mailboxes) with one FIFO queue.
//! Invocation payloads are executed directly through `run_invocation` during drain.

use std::collections::VecDeque;
#[cfg(feature = "lsp")]
use std::collections::HashMap;
use std::time::Instant;

use crate::Editor;
use crate::runtime::protocol::RuntimeCauseId;
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

/// Non-payload runtime work kind tag used for bounded-cardinality metrics.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RuntimeWorkKindTag {
	Invocation,
	OverlayCommit,
	#[cfg(feature = "lsp")]
	WorkspaceEdit,
}

impl RuntimeWorkKind {
	pub const fn kind_tag(&self) -> RuntimeWorkKindTag {
		match self {
			Self::Invocation(_) => RuntimeWorkKindTag::Invocation,
			Self::OverlayCommit => RuntimeWorkKindTag::OverlayCommit,
			#[cfg(feature = "lsp")]
			Self::WorkspaceEdit(_) => RuntimeWorkKindTag::WorkspaceEdit,
		}
	}
}

/// Per-kind runtime work counts.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct RuntimeWorkKindCounts {
	pub invocation: usize,
	pub overlay_commit: usize,
	#[cfg(feature = "lsp")]
	pub workspace_edit: usize,
}

impl RuntimeWorkKindCounts {
	pub(crate) fn add_kind(&mut self, tag: RuntimeWorkKindTag) {
		match tag {
			RuntimeWorkKindTag::Invocation => {
				self.invocation = self.invocation.saturating_add(1);
			}
			RuntimeWorkKindTag::OverlayCommit => {
				self.overlay_commit = self.overlay_commit.saturating_add(1);
			}
			#[cfg(feature = "lsp")]
			RuntimeWorkKindTag::WorkspaceEdit => {
				self.workspace_edit = self.workspace_edit.saturating_add(1);
			}
		}
	}

	pub(crate) fn add_from(&mut self, other: Self) {
		self.invocation = self.invocation.saturating_add(other.invocation);
		self.overlay_commit = self.overlay_commit.saturating_add(other.overlay_commit);
		#[cfg(feature = "lsp")]
		{
			self.workspace_edit = self.workspace_edit.saturating_add(other.workspace_edit);
		}
	}
}

/// Per-kind oldest queued runtime work age snapshot in milliseconds.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct RuntimeWorkKindOldestAgesMs {
	pub invocation_ms: Option<u64>,
	pub overlay_commit_ms: Option<u64>,
	#[cfg(feature = "lsp")]
	pub workspace_edit_ms: Option<u64>,
}

impl RuntimeWorkKindOldestAgesMs {
	pub(crate) fn observe_kind_age_ms(&mut self, tag: RuntimeWorkKindTag, age_ms: u64) {
		match tag {
			RuntimeWorkKindTag::Invocation => {
				self.invocation_ms = Some(self.invocation_ms.unwrap_or(0).max(age_ms));
			}
			RuntimeWorkKindTag::OverlayCommit => {
				self.overlay_commit_ms = Some(self.overlay_commit_ms.unwrap_or(0).max(age_ms));
			}
			#[cfg(feature = "lsp")]
			RuntimeWorkKindTag::WorkspaceEdit => {
				self.workspace_edit_ms = Some(self.workspace_edit_ms.unwrap_or(0).max(age_ms));
			}
		}
	}
}

/// Queue entry carrying sequence and scope metadata.
#[derive(Debug, Clone)]
pub struct RuntimeWorkItem {
	pub kind: RuntimeWorkKind,
	pub kind_tag: RuntimeWorkKindTag,
	pub scope: WorkScope,
	pub seq: u64,
	pub cause_id: Option<RuntimeCauseId>,
	pub enqueued_at: Instant,
}

/// FIFO queue for deferred runtime work.
#[derive(Debug, Default)]
pub struct RuntimeWorkQueue {
	seq_next: u64,
	queue: VecDeque<RuntimeWorkItem>,
	/// Reply channels for workspace edit items, keyed by sequence number.
	/// Stored separately because `oneshot::Sender` is not `Clone`.
	/// Tuple: (reply sender, deadline after which edit should not be applied).
	#[cfg(feature = "lsp")]
	apply_edit_replies: HashMap<u64, (tokio::sync::oneshot::Sender<xeno_lsp::sync::ApplyEditResult>, Instant)>,
}

impl RuntimeWorkQueue {
	/// Enqueues one runtime work item and returns its sequence number.
	pub fn enqueue(&mut self, kind: RuntimeWorkKind, scope: WorkScope) -> u64 {
		self.enqueue_with_cause(kind, scope, None)
	}

	/// Enqueues one runtime work item with explicit causal metadata.
	pub fn enqueue_with_cause(&mut self, kind: RuntimeWorkKind, scope: WorkScope, cause_id: Option<RuntimeCauseId>) -> u64 {
		let seq = self.seq_next;
		self.seq_next = self.seq_next.wrapping_add(1);
		let kind_tag = kind.kind_tag();
		self.queue.push_back(RuntimeWorkItem {
			kind,
			kind_tag,
			scope,
			seq,
			cause_id,
			enqueued_at: Instant::now(),
		});
		seq
	}

	/// Enqueues one deferred invocation item and returns its sequence number.
	pub fn enqueue_invocation(&mut self, invocation: Invocation, source: RuntimeWorkSource, execution: WorkExecutionPolicy, scope: WorkScope) -> u64 {
		self.enqueue_invocation_with_cause(invocation, source, execution, scope, None)
	}

	/// Enqueues one deferred invocation item with explicit causal metadata.
	pub fn enqueue_invocation_with_cause(
		&mut self,
		invocation: Invocation,
		source: RuntimeWorkSource,
		execution: WorkExecutionPolicy,
		scope: WorkScope,
		cause_id: Option<RuntimeCauseId>,
	) -> u64 {
		self.enqueue_with_cause(RuntimeWorkKind::Invocation(QueuedInvocation { invocation, source, execution }), scope, cause_id)
	}

	/// Enqueues one deferred overlay commit item and returns its sequence number.
	pub fn enqueue_overlay_commit(&mut self) -> u64 {
		self.enqueue_overlay_commit_with_cause(None)
	}

	/// Enqueues one deferred overlay commit item with explicit causal metadata.
	pub fn enqueue_overlay_commit_with_cause(&mut self, cause_id: Option<RuntimeCauseId>) -> u64 {
		self.enqueue_with_cause(RuntimeWorkKind::OverlayCommit, WorkScope::Global, cause_id)
	}

	/// Enqueues one deferred workspace edit item and returns its sequence number.
	#[cfg(feature = "lsp")]
	pub fn enqueue_workspace_edit(&mut self, edit: xeno_lsp::lsp_types::WorkspaceEdit) -> u64 {
		self.enqueue_workspace_edit_with_cause(edit, None, None)
	}

	/// Enqueues one deferred workspace edit item with optional reply channel and explicit causal metadata.
	#[cfg(feature = "lsp")]
	pub fn enqueue_workspace_edit_with_cause(
		&mut self,
		edit: xeno_lsp::lsp_types::WorkspaceEdit,
		reply: Option<(tokio::sync::oneshot::Sender<xeno_lsp::sync::ApplyEditResult>, Instant)>,
		cause_id: Option<RuntimeCauseId>,
	) -> u64 {
		let seq = self.enqueue_with_cause(RuntimeWorkKind::WorkspaceEdit(edit), WorkScope::Global, cause_id);
		if let Some(entry) = reply {
			self.apply_edit_replies.insert(seq, entry);
		}
		seq
	}

	/// Pops the next work item in FIFO order.
	pub fn pop_front(&mut self) -> Option<RuntimeWorkItem> {
		self.queue.pop_front()
	}

	/// Takes the reply entry (sender + deadline) for a workspace edit item.
	#[cfg(feature = "lsp")]
	pub fn take_apply_edit_reply(&mut self, seq: u64) -> Option<(tokio::sync::oneshot::Sender<xeno_lsp::sync::ApplyEditResult>, Instant)> {
		self.apply_edit_replies.remove(&seq)
	}

	/// Returns queued item count.
	pub fn len(&self) -> usize {
		self.queue.len()
	}

	/// Returns true when queue is empty.
	pub fn is_empty(&self) -> bool {
		self.queue.is_empty()
	}

	/// Returns pending runtime work counts grouped by kind tag.
	pub fn depth_by_kind(&self) -> RuntimeWorkKindCounts {
		let mut counts = RuntimeWorkKindCounts::default();
		for item in &self.queue {
			counts.add_kind(item.kind_tag);
		}
		counts
	}

	/// Returns oldest queued runtime work age grouped by kind tag.
	pub fn oldest_age_ms_by_kind(&self) -> RuntimeWorkKindOldestAgesMs {
		let mut ages = RuntimeWorkKindOldestAgesMs::default();
		let now = Instant::now();
		for item in &self.queue {
			let age_ms = now.saturating_duration_since(item.enqueued_at).as_millis().min(u128::from(u64::MAX)) as u64;
			ages.observe_kind_age_ms(item.kind_tag, age_ms);
		}
		ages
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
	///
	/// Any workspace edit reply channels for removed items are completed with
	/// `applied: false` so the server gets a fast rejection instead of timing out.
	pub fn remove_scope(&mut self, scope: WorkScope) -> usize {
		let before = self.queue.len();
		#[cfg(feature = "lsp")]
		{
			let seqs: Vec<u64> = self.queue.iter()
				.filter(|item| item.scope == scope && matches!(item.kind, RuntimeWorkKind::WorkspaceEdit(_)))
				.map(|item| item.seq)
				.collect();
			for seq in seqs {
				self.reject_apply_edit_reply(seq, "work scope cancelled");
			}
		}
		self.queue.retain(|item| item.scope != scope);
		before.saturating_sub(self.queue.len())
	}

	/// Rejects a pending apply-edit reply with `applied: false`.
	#[cfg(feature = "lsp")]
	fn reject_apply_edit_reply(&mut self, seq: u64, reason: &str) {
		if let Some((tx, _deadline)) = self.apply_edit_replies.remove(&seq) {
			let _ = tx.send(xeno_lsp::sync::ApplyEditResult {
				applied: false,
				failure_reason: Some(reason.to_string()),
				failed_change: None,
			});
		}
	}

	#[cfg(test)]
	pub fn snapshot(&self) -> Vec<RuntimeWorkItem> {
		self.queue.iter().cloned().collect()
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
		let cause_id = self.runtime_active_cause_id();
		let seq = self
			.state
			.runtime_work_queue_mut()
			.enqueue_invocation_with_cause(invocation, source, execution, scope, cause_id);
		self.metrics().record_runtime_work_queue_depth(self.state.runtime_work_queue().len() as u64);
		seq
	}

	/// Enqueues one deferred overlay commit as runtime work.
	pub(crate) fn enqueue_runtime_overlay_commit_work(&mut self) -> u64 {
		let cause_id = self.runtime_active_cause_id();
		let seq = self.state.runtime_work_queue_mut().enqueue_overlay_commit_with_cause(cause_id);
		self.metrics().record_runtime_work_queue_depth(self.state.runtime_work_queue().len() as u64);
		seq
	}

	/// Enqueues one deferred workspace edit as runtime work with an optional reply entry.
	#[cfg(feature = "lsp")]
	pub(crate) fn enqueue_runtime_workspace_edit_work(
		&mut self,
		edit: xeno_lsp::lsp_types::WorkspaceEdit,
		reply: Option<(tokio::sync::oneshot::Sender<xeno_lsp::sync::ApplyEditResult>, std::time::Instant)>,
	) -> u64 {
		let cause_id = self.runtime_active_cause_id();
		let seq = self.state.runtime_work_queue_mut().enqueue_workspace_edit_with_cause(edit, reply, cause_id);
		self.metrics().record_runtime_work_queue_depth(self.state.runtime_work_queue().len() as u64);
		seq
	}

	/// Pops one deferred runtime work item in FIFO order.
	pub(crate) fn pop_runtime_work(&mut self) -> Option<RuntimeWorkItem> {
		let item = self.state.runtime_work_queue_mut().pop_front();
		if item.is_some() {
			self.metrics().record_runtime_work_queue_depth(self.state.runtime_work_queue().len() as u64);
		}
		item
	}

	/// Removes deferred runtime work matching a specific scope.
	pub(crate) fn clear_runtime_work_scope(&mut self, scope: WorkScope) -> usize {
		let removed = self.state.runtime_work_queue_mut().remove_scope(scope);
		if removed > 0 {
			self.metrics().record_runtime_work_queue_depth(self.state.runtime_work_queue().len() as u64);
		}
		removed
	}

	/// Returns deferred runtime work queue length.
	pub(crate) fn runtime_work_len(&self) -> usize {
		self.state.runtime_work_queue().len()
	}

	/// Returns oldest queued runtime work age grouped by kind.
	pub(crate) fn runtime_work_oldest_age_ms_by_kind(&self) -> RuntimeWorkKindOldestAgesMs {
		self.state.runtime_work_queue().oldest_age_ms_by_kind()
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

	#[cfg(test)]
	pub(crate) fn runtime_work_snapshot(&self) -> Vec<RuntimeWorkItem> {
		self.state.runtime_work_queue().snapshot()
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

	#[cfg(feature = "lsp")]
	#[test]
	fn remove_scope_rejects_apply_edit_replies() {
		let mut queue = RuntimeWorkQueue::default();
		let (tx, mut rx) = tokio::sync::oneshot::channel();
		let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
		queue.enqueue_workspace_edit_with_cause(
			xeno_lsp::lsp_types::WorkspaceEdit::default(),
			Some((tx, deadline)),
			None,
		);

		// Enqueue it under a non-global scope to test removal.
		// Since workspace edits always use Global, we manually override the scope.
		queue.queue.back_mut().unwrap().scope = super::WorkScope::NuStopScope(99);

		queue.remove_scope(super::WorkScope::NuStopScope(99));
		assert!(queue.is_empty());

		// Reply must have been sent with applied=false.
		let result = rx.try_recv().expect("reply must be sent");
		assert!(!result.applied);
		assert_eq!(result.failure_reason.as_deref(), Some("work scope cancelled"));
	}
}
