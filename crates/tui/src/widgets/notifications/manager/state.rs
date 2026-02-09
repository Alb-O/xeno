//! Internal state for individual toast notifications.

use std::time::{Duration, Instant};

use super::super::toast::Toast;
use super::super::types::{AnimationPhase, AutoDismiss, Timing};
use crate::layout::Rect;

/// Default duration for toast entry animation.
pub(super) const DEFAULT_ENTRY_DURATION: Duration = Duration::from_millis(300);
/// Default duration for toast exit animation.
pub(super) const DEFAULT_EXIT_DURATION: Duration = Duration::from_millis(200);
/// Default dwell time before auto-dismissing a toast.
pub(super) const DEFAULT_DWELL_DURATION: Duration = Duration::from_secs(4);

/// Internal state for a single toast notification.
#[derive(Debug)]
pub(super) struct ToastState {
	/// The toast content and configuration.
	pub(super) toast: Toast,
	/// Current animation phase.
	pub(super) phase: AnimationPhase,
	/// Animation progress within current phase (0.0 to 1.0).
	pub(super) progress: f32,
	/// When the toast was created.
	pub(super) created_at: Instant,
	/// Time remaining before auto-dismiss (None = manual dismiss only).
	pub(super) remaining_dwell: Option<Duration>,
	/// Duration of entry animation.
	pub(super) entry_duration: Duration,
	/// Duration of exit animation.
	pub(super) exit_duration: Duration,
	/// Computed rectangle at full visibility.
	pub(super) full_rect: Rect,
	/// Number of stacked duplicate notifications (1 = no duplicates).
	pub(super) stack_count: u32,
	/// Original dwell duration for resetting on stack increment.
	pub(super) original_dwell: Option<Duration>,
}

impl ToastState {
	/// Creates a new toast state with default timings.
	pub(super) fn new(toast: Toast) -> Self {
		let entry_duration = match toast.entry_timing {
			Timing::Auto => DEFAULT_ENTRY_DURATION,
			Timing::Fixed(d) => d,
		};
		let exit_duration = match toast.exit_timing {
			Timing::Auto => DEFAULT_EXIT_DURATION,
			Timing::Fixed(d) => d,
		};
		let remaining_dwell = match toast.auto_dismiss {
			AutoDismiss::Never => None,
			AutoDismiss::After(d) if d.is_zero() => Some(DEFAULT_DWELL_DURATION),
			AutoDismiss::After(d) => Some(d),
		};
		let original_dwell = remaining_dwell;

		Self {
			toast,
			phase: AnimationPhase::Pending,
			progress: 0.0,
			created_at: Instant::now(),
			remaining_dwell,
			entry_duration,
			exit_duration,
			full_rect: Rect::default(),
			stack_count: 1,
			original_dwell,
		}
	}

	/// Advances the toast animation by the given time delta.
	pub(super) fn update(&mut self, delta: Duration) {
		if self.phase == AnimationPhase::Pending {
			self.phase = AnimationPhase::Entering;
			self.progress = 0.0;
		}

		let phase_duration = match self.phase {
			AnimationPhase::Entering => self.entry_duration,
			AnimationPhase::Exiting => self.exit_duration,
			_ => Duration::ZERO,
		};

		if !phase_duration.is_zero()
			&& matches!(
				self.phase,
				AnimationPhase::Entering | AnimationPhase::Exiting
			) {
			self.progress =
				(self.progress + delta.as_secs_f32() / phase_duration.as_secs_f32()).min(1.0);

			if self.progress >= 1.0 {
				match self.phase {
					AnimationPhase::Entering => {
						self.phase = AnimationPhase::Dwelling;
						self.progress = 0.0;
					}
					AnimationPhase::Exiting => {
						self.phase = AnimationPhase::Finished;
					}
					_ => {}
				}
			}
		}

		if self.phase == AnimationPhase::Dwelling
			&& let Some(remaining) = self.remaining_dwell.as_mut()
		{
			*remaining = remaining.saturating_sub(delta);
			if remaining.is_zero() {
				self.phase = AnimationPhase::Exiting;
				self.progress = 0.0;
			}
		}
	}

	/// Returns true if the toast has completed its exit animation.
	pub(super) fn is_finished(&self) -> bool {
		self.phase == AnimationPhase::Finished
	}

	/// Increments the stack count and resets the dwell timer.
	pub(super) fn increment_stack(&mut self) {
		self.stack_count = self.stack_count.saturating_add(1);
		self.remaining_dwell = self.original_dwell;
	}

	/// Returns true if this toast can be stacked with another having the same content.
	pub(super) fn can_stack(&self) -> bool {
		!matches!(
			self.phase,
			AnimationPhase::Exiting | AnimationPhase::Finished
		)
	}
}
