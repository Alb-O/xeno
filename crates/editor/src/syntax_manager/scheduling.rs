use std::collections::VecDeque;
use std::time::{Duration, Instant};

use xeno_language::LanguageId;
use xeno_language::syntax::{InjectionPolicy, Syntax, SyntaxError};

use super::tasks::TaskClass;
use super::types::{DocEpoch, OptKey, TaskId, ViewportKey};

/// Distinguishes the two viewport sub-lanes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum ViewportLane {
	/// Stage-A: fast, partial parse with limited injections.
	Urgent,
	/// Stage-B: enrichment parse with eager injections.
	Enrich,
}

/// Per-document scheduling state for background syntax parsing.
///
/// Maintains three independent task lanes (viewport-urgent, viewport-enrich,
/// and background) so that Stage-A can always kick immediately even while
/// Stage-B enrichment is running, and both are independent of full/incremental
/// background parses.
pub(crate) struct DocSched {
	pub(super) epoch: DocEpoch,
	pub(super) last_edit_at: Instant,
	pub(super) last_visible_at: Instant,
	pub(super) cooldown_until: Option<Instant>,

	/// Viewport urgent lane: Stage-A viewport tasks.
	pub(super) active_viewport_urgent: Option<TaskId>,
	pub(super) active_viewport_urgent_detached: bool,
	pub(super) requested_viewport_urgent_doc_version: u64,

	/// Viewport enrich lane: Stage-B viewport tasks.
	pub(super) active_viewport_enrich: Option<TaskId>,
	pub(super) active_viewport_enrich_detached: bool,
	pub(super) requested_viewport_enrich_doc_version: u64,

	/// Background lane: full and incremental parse tasks.
	pub(super) active_bg: Option<TaskId>,
	pub(super) active_bg_detached: bool,
	pub(super) requested_bg_doc_version: u64,

	/// Queue of completed tasks awaiting installation.
	pub(super) completed: VecDeque<CompletedSyntaxTask>,

	/// If true, bypasses the debounce gate for the next background parse.
	pub(super) force_no_debounce: bool,

	/// Viewport focus stability tracking for Stage-B gating.
	pub(super) viewport_focus_key: Option<ViewportKey>,
	pub(super) viewport_focus_doc_version: u64,
	pub(super) viewport_focus_stable_polls: u8,
}

impl DocSched {
	pub(super) fn new(now: Instant) -> Self {
		Self {
			epoch: DocEpoch(0),
			last_edit_at: now,
			last_visible_at: now,
			cooldown_until: None,
			active_viewport_urgent: None,
			active_viewport_urgent_detached: false,
			requested_viewport_urgent_doc_version: 0,
			active_viewport_enrich: None,
			active_viewport_enrich_detached: false,
			requested_viewport_enrich_doc_version: 0,
			active_bg: None,
			active_bg_detached: false,
			requested_bg_doc_version: 0,
			completed: VecDeque::new(),
			force_no_debounce: false,
			viewport_focus_key: None,
			viewport_focus_doc_version: 0,
			viewport_focus_stable_polls: 0,
		}
	}

	/// Invalidates the current scheduling window, bumping the epoch to discard stale tasks.
	///
	/// Clears all lanes and the completion queue. Permits are released only
	/// on task completion to maintain strict concurrency bounds.
	pub(super) fn invalidate(&mut self) {
		self.epoch = self.epoch.next();
		self.active_viewport_urgent = None;
		self.active_viewport_urgent_detached = false;
		self.requested_viewport_urgent_doc_version = 0;
		self.active_viewport_enrich = None;
		self.active_viewport_enrich_detached = false;
		self.requested_viewport_enrich_doc_version = 0;
		self.active_bg = None;
		self.active_bg_detached = false;
		self.requested_bg_doc_version = 0;
		self.completed.clear();
		self.cooldown_until = None;
		self.force_no_debounce = false;
		self.viewport_focus_key = None;
		self.viewport_focus_doc_version = 0;
		self.viewport_focus_stable_polls = 0;
	}

	/// Updates viewport focus tracking and returns the new stable poll count.
	///
	/// If the key and doc_version match the previous call, increments the counter
	/// (saturating). Otherwise resets to 1.
	pub(super) fn note_viewport_focus(&mut self, key: ViewportKey, doc_version: u64) -> u8 {
		if self.viewport_focus_key == Some(key) && self.viewport_focus_doc_version == doc_version {
			self.viewport_focus_stable_polls = self.viewport_focus_stable_polls.saturating_add(1);
		} else {
			self.viewport_focus_key = Some(key);
			self.viewport_focus_doc_version = doc_version;
			self.viewport_focus_stable_polls = 1;
		}
		self.viewport_focus_stable_polls
	}

	/// Returns true if any task lane is active and not detached.
	pub(super) fn any_active(&self) -> bool {
		self.viewport_urgent_active() || self.viewport_enrich_active() || self.bg_active()
	}

	/// Returns true if either viewport sub-lane is active and not detached.
	pub(super) fn viewport_any_active(&self) -> bool {
		self.viewport_urgent_active() || self.viewport_enrich_active()
	}

	/// Returns true if the viewport urgent (Stage-A) lane is active and not detached.
	pub(super) fn viewport_urgent_active(&self) -> bool {
		self.active_viewport_urgent.is_some() && !self.active_viewport_urgent_detached
	}

	/// Returns true if the viewport enrich (Stage-B) lane is active and not detached.
	pub(super) fn viewport_enrich_active(&self) -> bool {
		self.active_viewport_enrich.is_some() && !self.active_viewport_enrich_detached
	}

	/// Returns true if the background lane is active and not detached.
	pub(super) fn bg_active(&self) -> bool {
		self.active_bg.is_some() && !self.active_bg_detached
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
	pub(super) viewport_key: Option<ViewportKey>,
	pub(super) viewport_lane: Option<ViewportLane>,
}
