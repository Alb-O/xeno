//! Runtime-owned deferred work state and mailbox drain policy.
//!
//! This module centralizes deferred runtime work storage (overlay commit
//! requests, deferred workspace edits, and deferred invocations.

#[cfg(feature = "lsp")]
use std::collections::VecDeque;

use crate::Editor;
use crate::runtime::mailbox::{DeferredInvocation, DeferredInvocationExecutionPolicy, DeferredInvocationScope, DeferredInvocationSource, InvocationMailbox};
use crate::types::Invocation;

/// Unified deferred-runtime state drained by runtime pump phases.
#[derive(Debug, Default)]
pub(crate) struct RuntimeDeferredState {
	overlay_commit_requests: usize,
	#[cfg(feature = "lsp")]
	workspace_edits: VecDeque<xeno_lsp::lsp_types::WorkspaceEdit>,
	invocation_mailbox: InvocationMailbox,
}

impl RuntimeDeferredState {
	/// Enqueues one deferred overlay commit request.
	pub(crate) fn request_overlay_commit(&mut self) {
		self.overlay_commit_requests = self.overlay_commit_requests.saturating_add(1);
	}

	/// Returns true when at least one overlay commit request is pending.
	#[cfg(test)]
	pub(crate) fn has_overlay_commit(&self) -> bool {
		self.overlay_commit_requests > 0
	}

	/// Pops exactly one pending overlay commit request when available.
	pub(crate) fn take_overlay_commit_once(&mut self) -> bool {
		if self.overlay_commit_requests == 0 {
			return false;
		}
		self.overlay_commit_requests -= 1;
		true
	}

	/// Appends one deferred workspace edit.
	#[cfg(feature = "lsp")]
	pub(crate) fn push_workspace_edit(&mut self, edit: xeno_lsp::lsp_types::WorkspaceEdit) {
		self.workspace_edits.push_back(edit);
	}

	/// Drains all pending deferred workspace edits.
	#[cfg(feature = "lsp")]
	pub(crate) fn take_workspace_edits(&mut self) -> Vec<xeno_lsp::lsp_types::WorkspaceEdit> {
		self.workspace_edits.drain(..).collect()
	}

	/// Returns the number of pending deferred workspace edits.
	#[cfg(all(feature = "lsp", test))]
	pub(crate) fn pending_workspace_edits(&self) -> usize {
		self.workspace_edits.len()
	}

	/// Enqueues one deferred invocation and returns its mailbox sequence number.
	pub(crate) fn enqueue_invocation(
		&mut self,
		invocation: Invocation,
		source: DeferredInvocationSource,
		execution: DeferredInvocationExecutionPolicy,
		scope: DeferredInvocationScope,
	) -> u64 {
		self.invocation_mailbox.enqueue(invocation, source, execution, scope)
	}

	/// Pops one deferred invocation in FIFO order.
	pub(crate) fn pop_invocation(&mut self) -> Option<DeferredInvocation> {
		self.invocation_mailbox.pop_front()
	}

	/// Removes deferred invocations scoped to one Nu stop-propagation generation.
	pub(crate) fn remove_invocation_scope(&mut self, scope: DeferredInvocationScope) -> usize {
		self.invocation_mailbox.remove_scope(scope)
	}

	/// Returns the current deferred invocation mailbox length.
	pub(crate) fn invocation_len(&self) -> usize {
		self.invocation_mailbox.len()
	}

	/// Returns true when no deferred invocations are queued.
	#[cfg(test)]
	pub(crate) fn invocation_is_empty(&self) -> bool {
		self.invocation_mailbox.is_empty()
	}
}

impl Editor {
	/// Enqueues one deferred overlay commit request.
	pub(crate) fn request_overlay_commit_deferred(&mut self) {
		self.state.runtime_deferred_mut().request_overlay_commit();
	}

	/// Pops one deferred overlay commit request, if any.
	pub(crate) fn take_overlay_commit_deferred_once(&mut self) -> bool {
		self.state.runtime_deferred_mut().take_overlay_commit_once()
	}

	/// Returns true when a deferred overlay commit request is pending.
	#[cfg(test)]
	pub(crate) fn has_overlay_commit_deferred(&self) -> bool {
		self.state.runtime_deferred().has_overlay_commit()
	}

	/// Enqueues one deferred workspace edit.
	#[cfg(feature = "lsp")]
	pub(crate) fn enqueue_workspace_edit_deferred(&mut self, edit: xeno_lsp::lsp_types::WorkspaceEdit) {
		self.state.runtime_deferred_mut().push_workspace_edit(edit);
	}

	/// Drains all deferred workspace edits.
	#[cfg(feature = "lsp")]
	pub(crate) fn take_workspace_edits_deferred(&mut self) -> Vec<xeno_lsp::lsp_types::WorkspaceEdit> {
		self.state.runtime_deferred_mut().take_workspace_edits()
	}

	/// Returns the number of pending deferred workspace edits.
	#[cfg(all(feature = "lsp", test))]
	pub(crate) fn pending_workspace_edits_deferred(&self) -> usize {
		self.state.runtime_deferred().pending_workspace_edits()
	}

	/// Enqueues one deferred invocation into runtime-deferred mailbox.
	pub(crate) fn enqueue_runtime_deferred_invocation(
		&mut self,
		invocation: Invocation,
		source: DeferredInvocationSource,
		execution: DeferredInvocationExecutionPolicy,
		scope: DeferredInvocationScope,
	) -> u64 {
		self.state.runtime_deferred_mut().enqueue_invocation(invocation, source, execution, scope)
	}

	/// Pops one deferred invocation from runtime-deferred mailbox.
	pub(crate) fn pop_runtime_deferred_invocation(&mut self) -> Option<DeferredInvocation> {
		self.state.runtime_deferred_mut().pop_invocation()
	}

	/// Removes deferred invocations matching a specific scope.
	pub(crate) fn remove_runtime_deferred_invocation_scope(&mut self, scope: DeferredInvocationScope) -> usize {
		self.state.runtime_deferred_mut().remove_invocation_scope(scope)
	}

	/// Returns deferred invocation mailbox length.
	pub(crate) fn runtime_deferred_invocation_len(&self) -> usize {
		self.state.runtime_deferred().invocation_len()
	}

	/// Returns true when deferred invocation mailbox is empty.
	#[cfg(test)]
	pub(crate) fn runtime_deferred_invocation_is_empty(&self) -> bool {
		self.state.runtime_deferred().invocation_is_empty()
	}
}
