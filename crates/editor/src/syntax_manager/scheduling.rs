use std::time::Instant;

use xeno_runtime_language::LanguageId;
use xeno_runtime_language::syntax::{Syntax, SyntaxError};

use super::types::{DocEpoch, OptKey, TaskId};

pub(crate) struct DocSched {
	pub(super) epoch: DocEpoch,
	pub(super) last_edit_at: Instant,
	pub(super) last_visible_at: Instant,
	pub(super) cooldown_until: Option<Instant>,
	pub(super) active_task: Option<TaskId>,
	pub(super) active_task_detached: bool,
	pub(super) completed: Option<CompletedSyntaxTask>,
	/// If true, bypasses the debounce gate for the next background parse.
	pub(super) force_no_debounce: bool,
}

impl DocSched {
	pub(super) fn new(now: Instant) -> Self {
		Self {
			epoch: DocEpoch(0),
			last_edit_at: now,
			last_visible_at: now,
			cooldown_until: None,
			active_task: None,
			active_task_detached: false,
			completed: None,
			force_no_debounce: false,
		}
	}

	/// Invalidates the current scheduling window, bumping the epoch to discard stale tasks.
	///
	/// NOTE: Invalidation does not imply cancellation of the background thread; permits
	/// are released only on task completion to maintain strict concurrency bounds.
	pub(super) fn invalidate(&mut self) {
		self.epoch = self.epoch.next();
		self.active_task = None;
		self.active_task_detached = false;
		self.completed = None;
		self.cooldown_until = None;
		self.force_no_debounce = false;
	}
}

pub(super) struct CompletedSyntaxTask {
	pub(super) doc_version: u64,
	pub(super) lang_id: LanguageId,
	pub(super) opts: OptKey,
	pub(super) result: Result<Syntax, SyntaxError>,
	pub(super) is_viewport: bool,
}
