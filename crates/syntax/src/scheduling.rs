use std::collections::VecDeque;
use std::time::{Duration, Instant};

use xeno_language::LanguageId;
use xeno_language::{Syntax, SyntaxError};

use super::tasks::TaskClass;
use super::types::{DocEpoch, OptKey, TaskId, ViewportKey};

/// Distinguishes the two viewport sub-lanes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum ViewportLane {
	/// Stage-A: fast, partial parse.
	///
	/// Uses tier-configured viewport injections by default; L-tier history edits
	/// may elevate this lane to eager injections to reduce undo repaint churn.
	Urgent,
	/// Stage-B: enrichment parse with eager injections.
	Enrich,
}

/// State of a single task lane (active task, requested version, cooldown).
#[derive(Default)]
pub(super) struct LaneState {
	pub(super) active: Option<TaskId>,
	pub(super) requested_doc_version: u64,
	pub(super) cooldown_until: Option<Instant>,
}

impl LaneState {
	/// Resets lane to default (no active task, version 0, no cooldown).
	#[inline]
	pub(super) fn clear(&mut self) {
		*self = Self::default();
	}

	/// Returns true if a task is active.
	#[inline]
	pub(super) fn is_active(&self) -> bool {
		self.active.is_some()
	}

	/// Returns true if this lane is in cooldown.
	#[inline]
	pub(super) fn in_cooldown(&self, now: Instant) -> bool {
		self.cooldown_until.is_some_and(|t| now < t)
	}

	/// Sets cooldown until the given instant.
	#[inline]
	pub(super) fn set_cooldown(&mut self, until: Instant) {
		self.cooldown_until = Some(until);
	}
}

/// Grouped lane states for the three independent task lanes.
///
/// Cooldown semantics differ per lane:
/// * `viewport_urgent`: lane-level cooldown (`LaneState.cooldown_until`).
/// * `viewport_enrich`: per-key cooldown (`ViewportEntry.stage_b_cooldown_until`);
///   the lane-level `LaneState.cooldown_until` is unused.
/// * `bg`: lane-level cooldown (`LaneState.cooldown_until`).
#[derive(Default)]
pub(super) struct Lanes {
	pub(super) viewport_urgent: LaneState,
	pub(super) viewport_enrich: LaneState,
	pub(super) bg: LaneState,
}

impl Lanes {
	/// Clears all lanes.
	pub(super) fn clear_all(&mut self) {
		self.viewport_urgent.clear();
		self.viewport_enrich.clear();
		self.bg.clear();
		debug_assert!(
			self.viewport_enrich.cooldown_until.is_none(),
			"viewport_enrich uses per-key cooldown (ViewportEntry.stage_b_cooldown_until), not lane-level"
		);
	}
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
	pub(super) last_edit_source: super::EditSource,
	pub(super) last_visible_at: Instant,
	pub(super) lanes: Lanes,

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
			last_edit_source: super::EditSource::Typing,
			last_visible_at: now,
			lanes: Lanes::default(),
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
		self.lanes.clear_all();
		self.completed.clear();
		self.force_no_debounce = false;
		self.last_edit_source = super::EditSource::Typing;
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

	/// Returns true if any task lane is active.
	pub(super) fn any_active(&self) -> bool {
		self.lanes.viewport_urgent.is_active() || self.lanes.viewport_enrich.is_active() || self.lanes.bg.is_active()
	}

	/// Returns true if either viewport sub-lane is active.
	#[cfg(test)]
	pub(super) fn viewport_any_active(&self) -> bool {
		self.lanes.viewport_urgent.is_active() || self.lanes.viewport_enrich.is_active()
	}

	/// Returns true if the viewport urgent (Stage-A) lane is active.
	pub(super) fn viewport_urgent_active(&self) -> bool {
		self.lanes.viewport_urgent.is_active()
	}

	/// Returns true if the viewport enrich (Stage-B) lane is active.
	pub(super) fn viewport_enrich_active(&self) -> bool {
		self.lanes.viewport_enrich.is_active()
	}

	/// Returns true if the background lane is active.
	pub(super) fn bg_active(&self) -> bool {
		self.lanes.bg.is_active()
	}
}

/// Result of a finished background parse, awaiting installation into the document's syntax slot.
pub(super) struct CompletedSyntaxTask {
	pub(super) doc_version: u64,
	pub(super) lang_id: LanguageId,
	pub(super) opts: OptKey,
	pub(super) result: Result<Syntax, SyntaxError>,
	pub(super) class: TaskClass,
	pub(super) elapsed: Duration,
	pub(super) viewport_key: Option<ViewportKey>,
	pub(super) viewport_lane: Option<ViewportLane>,
}
