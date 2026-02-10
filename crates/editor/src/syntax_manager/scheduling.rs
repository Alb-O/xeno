use std::time::{Duration, Instant};

use xeno_runtime_language::LanguageId;
use xeno_runtime_language::syntax::{InjectionPolicy, Syntax, SyntaxError};

use super::tasks::TaskClass;
use super::types::{DocEpoch, OptKey, TaskId};

/// Per-document scheduling state for background syntax parsing.
///
/// Tracks the current epoch, edit timestamps, cooldown timers, and the latest
/// completed parse result. The syntax manager reads and updates this on every
/// tick to decide whether to launch, skip, or install a parse.
pub(crate) struct DocSched {
	pub(super) epoch: DocEpoch,
	pub(super) last_edit_at: Instant,
	pub(super) last_visible_at: Instant,
	pub(super) cooldown_until: Option<Instant>,
	pub(super) active_task: Option<TaskId>,
	pub(super) active_task_class: Option<TaskClass>,
	pub(super) active_task_detached: bool,
	/// Document version for which the last task was requested.
	pub(super) requested_doc_version: u64,
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
			active_task_class: None,
			active_task_detached: false,
			requested_doc_version: 0,
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
		self.active_task_class = None;
		self.active_task_detached = false;
		self.requested_doc_version = 0;
		self.completed = None;
		self.cooldown_until = None;
		self.force_no_debounce = false;
	}
}

/// Result of a finished background parse, awaiting installation into the document's syntax slot.
pub(super) struct CompletedSyntaxTask {
	pub(super) doc_version: u64,
	pub(super) lang_id: LanguageId,
	pub(super) opts: OptKey,
	pub(super) result: Result<Syntax, SyntaxError>,
	pub(super) class: TaskClass,
	pub(super) injections: InjectionPolicy,
	pub(super) elapsed: Duration,
}
