use std::time::{Duration, Instant};

use ratatui::prelude::*;
use ratatui::widgets::block::Padding;
use ratatui::widgets::{Block, BorderType};
use tome_manifest::notifications::{Animation, AnimationPhase, AutoDismiss, Level, Timing};
use tome_stdlib::notifications::Notification;

use crate::render::notifications::animation::{
	FadeHandler, expand_calculate_rect, fade_calculate_rect, slide_apply_border_effect,
	slide_calculate_rect,
};
use crate::render::notifications::notification::calculate_size;
use crate::render::notifications::types::SlideParams;

#[derive(Debug, Clone, Copy)]
pub(crate) struct ManagerDefaults {
	pub default_entry_duration: Duration,
	pub default_exit_duration: Duration,
	pub default_display_time: Duration,
}

impl Default for ManagerDefaults {
	fn default() -> Self {
		Self {
			default_entry_duration: Duration::from_millis(500),
			default_exit_duration: Duration::from_millis(750),
			default_display_time: Duration::from_secs(4),
		}
	}
}

#[derive(Debug)]
pub(crate) struct NotificationState {
	#[allow(
		dead_code,
		reason = "used by StackableNotification trait impl for identification"
	)]
	pub(crate) id: u64,
	pub(crate) notification: Notification,
	pub(crate) created_at: Instant,
	pub(crate) current_phase: AnimationPhase,
	pub(crate) animation_progress: f32,
	pub(crate) full_rect: Rect,
	pub(crate) remaining_display_time: Option<Duration>,
	pub(crate) actual_entry_duration: Duration,
	pub(crate) actual_exit_duration: Duration,
	pub(crate) custom_entry_pos: Option<(f32, f32)>,
	pub(crate) custom_exit_pos: Option<(f32, f32)>,
}

impl NotificationState {
	pub(crate) fn new(id: u64, notification: Notification, defaults: &ManagerDefaults) -> Self {
		let actual_entry_duration = match notification.slide_in_timing {
			Timing::Fixed(d) => d,
			Timing::Auto => defaults.default_entry_duration,
		};
		let actual_exit_duration = match notification.slide_out_timing {
			Timing::Fixed(d) => d,
			Timing::Auto => defaults.default_exit_duration,
		};
		let remaining_display_time = match notification.auto_dismiss {
			AutoDismiss::Never => None,
			AutoDismiss::After(d) if d > Duration::ZERO => Some(d),
			AutoDismiss::After(_) => Some(defaults.default_display_time),
		};
		let custom_entry_pos = notification
			.custom_entry_position
			.map(|p| (p.x as f32, p.y as f32));
		let custom_exit_pos = notification
			.custom_exit_position
			.map(|p| (p.x as f32, p.y as f32));

		Self {
			id,
			notification,
			created_at: Instant::now(),
			current_phase: AnimationPhase::Pending,
			animation_progress: 0.0,
			full_rect: Rect::default(),
			remaining_display_time,
			actual_entry_duration,
			actual_exit_duration,
			custom_entry_pos,
			custom_exit_pos,
		}
	}

	pub(crate) fn update(&mut self, delta: Duration) {
		if self.current_phase == AnimationPhase::Pending {
			self.current_phase = match self.notification.animation {
				Animation::Slide => AnimationPhase::SlidingIn,
				Animation::ExpandCollapse => AnimationPhase::Expanding,
				Animation::Fade => AnimationPhase::FadingIn,
			};
			self.animation_progress = 0.0;
		}

		let phase_duration = match self.current_phase {
			AnimationPhase::SlidingIn | AnimationPhase::FadingIn | AnimationPhase::Expanding => {
				self.actual_entry_duration
			}
			AnimationPhase::SlidingOut | AnimationPhase::FadingOut | AnimationPhase::Collapsing => {
				self.actual_exit_duration
			}
			_ => Duration::ZERO,
		};

		let mut progress_updated = false;
		if phase_duration > Duration::ZERO
			&& matches!(
				self.current_phase,
				AnimationPhase::SlidingIn
					| AnimationPhase::FadingIn
					| AnimationPhase::Expanding
					| AnimationPhase::SlidingOut
					| AnimationPhase::FadingOut
					| AnimationPhase::Collapsing
			) {
			let delta_progress = delta.as_secs_f32() / phase_duration.as_secs_f32();
			self.animation_progress = (self.animation_progress + delta_progress).min(1.0);
			progress_updated = true;
		}

		if progress_updated && self.animation_progress >= 1.0 {
			match self.current_phase {
				AnimationPhase::SlidingIn
				| AnimationPhase::Expanding
				| AnimationPhase::FadingIn => {
					self.current_phase = AnimationPhase::Dwelling;
					self.animation_progress = 0.0;
				}
				AnimationPhase::SlidingOut
				| AnimationPhase::Collapsing
				| AnimationPhase::FadingOut => {
					self.current_phase = AnimationPhase::Finished;
				}
				_ => {}
			}
		}

		if self.current_phase == AnimationPhase::Dwelling
			&& let Some(remaining) = self.remaining_display_time.as_mut()
		{
			*remaining = remaining.saturating_sub(delta);
			if remaining.is_zero() {
				self.current_phase = match self.notification.animation {
					Animation::Slide => AnimationPhase::SlidingOut,
					Animation::ExpandCollapse => AnimationPhase::Collapsing,
					Animation::Fade => AnimationPhase::FadingOut,
				};
				self.animation_progress = 0.0;
			}
		}
	}
}

impl crate::render::notifications::stacking::StackableNotification for NotificationState {
	fn id(&self) -> u64 {
		self.id
	}
	fn current_phase(&self) -> AnimationPhase {
		self.current_phase
	}
	fn created_at(&self) -> Instant {
		self.created_at
	}
	fn full_rect(&self) -> Rect {
		self.full_rect
	}
	fn exterior_padding(&self) -> u16 {
		self.notification.exterior_margin
	}
	fn calculate_content_size(&self, frame_area: Rect) -> (u16, u16) {
		calculate_size(&self.notification, frame_area)
	}
}

impl crate::render::notifications::render::RenderableNotification for NotificationState {
	fn level(&self) -> Option<Level> {
		self.notification.level
	}
	fn title(&self) -> Option<Line<'static>> {
		self.notification
			.title
			.as_ref()
			.map(|t| Line::raw(t.clone()))
	}
	fn content(&self) -> Text<'static> {
		Text::raw(self.notification.content.clone())
	}
	fn border_type(&self) -> BorderType {
		self.notification.border_kind.into()
	}
	fn fade_effect(&self) -> bool {
		self.notification.fade_effect
	}
	fn animation_type(&self) -> Animation {
		self.notification.animation
	}
	fn animation_progress(&self) -> f32 {
		self.animation_progress
	}
	fn block_style(&self) -> Option<Style> {
		self.notification.block_style.map(Into::into)
	}
	fn border_style(&self) -> Option<Style> {
		self.notification.border_style.map(Into::into)
	}
	fn title_style(&self) -> Option<Style> {
		self.notification.title_style.map(Into::into)
	}
	fn padding(&self) -> Padding {
		self.notification.padding.into()
	}
	fn set_full_rect(&mut self, rect: Rect) {
		self.full_rect = rect;
	}
	fn calculate_animation_rect(&self, frame_area: Rect) -> Rect {
		match self.notification.animation {
			Animation::Slide => slide_calculate_rect(SlideParams {
				full_rect: self.full_rect,
				frame_area,
				progress: self.animation_progress,
				phase: self.current_phase,
				anchor: self.notification.anchor,
				slide_direction: self.notification.slide_direction,
				custom_slide_in_start_pos: self.custom_entry_pos,
				custom_slide_out_end_pos: self.custom_exit_pos,
			}),
			Animation::ExpandCollapse => expand_calculate_rect(
				self.full_rect,
				frame_area,
				self.current_phase,
				self.animation_progress,
			),
			Animation::Fade => fade_calculate_rect(
				self.full_rect,
				frame_area,
				self.current_phase,
				self.animation_progress,
			),
		}
	}
	fn apply_animation_block_effect<'a>(
		&self,
		block: Block<'a>,
		frame_area: Rect,
		base_set: &ratatui::symbols::border::Set<'a>,
	) -> Block<'a> {
		match self.notification.animation {
			Animation::Slide => slide_apply_border_effect(
				block,
				SlideParams {
					full_rect: self.full_rect,
					frame_area,
					progress: self.animation_progress,
					phase: self.current_phase,
					anchor: self.notification.anchor,
					slide_direction: self.notification.slide_direction,
					custom_slide_in_start_pos: self.custom_entry_pos,
					custom_slide_out_end_pos: self.custom_exit_pos,
				},
				base_set,
			),
			_ => block,
		}
	}
	fn interpolate_frame_foreground(
		&self,
		base_fg: Option<Color>,
		phase: AnimationPhase,
		progress: f32,
	) -> Option<Color> {
		match self.notification.animation {
			Animation::Fade => FadeHandler.interpolate_frame_foreground(base_fg, phase, progress),
			_ if self.notification.fade_effect => {
				FadeHandler.interpolate_frame_foreground(base_fg, phase, progress)
			}
			_ => base_fg,
		}
	}
	fn interpolate_content_foreground(
		&self,
		base_fg: Option<Color>,
		phase: AnimationPhase,
		progress: f32,
	) -> Option<Color> {
		match self.notification.animation {
			Animation::Fade => FadeHandler.interpolate_content_foreground(base_fg, phase, progress),
			_ if self.notification.fade_effect => {
				FadeHandler.interpolate_content_foreground(base_fg, phase, progress)
			}
			_ => base_fg.or(Some(Color::White)),
		}
	}
}

#[cfg(test)]
mod tests {
	use tome_manifest::notifications::{AutoDismiss, Timing};

	use super::*;

	fn create_test_notification() -> Notification {
		Notification {
			content: "Test notification".to_string(),
			..Default::default()
		}
	}

	#[test]
	fn test_new_state_starts_in_pending_phase() {
		let defaults = ManagerDefaults::default();
		let notification = create_test_notification();
		let state = NotificationState::new(1, notification, &defaults);
		assert_eq!(state.current_phase, AnimationPhase::Pending);
	}

	#[test]
	fn test_progress_starts_at_zero() {
		let defaults = ManagerDefaults::default();
		let notification = create_test_notification();
		let state = NotificationState::new(1, notification, &defaults);
		assert_eq!(state.animation_progress, 0.0);
	}

	#[test]
	fn test_timing_fixed_duration_resolved_correctly() {
		let defaults = ManagerDefaults::default();
		let mut notification = create_test_notification();
		notification.slide_in_timing = Timing::Fixed(Duration::from_millis(300));

		let state = NotificationState::new(1, notification, &defaults);
		assert_eq!(state.actual_entry_duration, Duration::from_millis(300));
	}

	#[test]
	fn test_timing_auto_uses_default_duration() {
		let defaults = ManagerDefaults {
			default_entry_duration: Duration::from_millis(600),
			default_exit_duration: Duration::from_millis(800),
			default_display_time: Duration::from_secs(5),
		};
		let mut notification = create_test_notification();
		notification.slide_in_timing = Timing::Auto;

		let state = NotificationState::new(1, notification, &defaults);
		assert_eq!(state.actual_entry_duration, Duration::from_millis(600));
	}

	#[test]
	fn test_auto_dismiss_never_sets_none() {
		let defaults = ManagerDefaults::default();
		let mut notification = create_test_notification();
		notification.auto_dismiss = AutoDismiss::Never;

		let state = NotificationState::new(1, notification, &defaults);
		assert!(state.remaining_display_time.is_none());
	}

	#[test]
	fn test_auto_dismiss_after_sets_duration() {
		let defaults = ManagerDefaults::default();
		let mut notification = create_test_notification();
		notification.auto_dismiss = AutoDismiss::After(Duration::from_secs(10));

		let state = NotificationState::new(1, notification, &defaults);
		assert_eq!(state.remaining_display_time, Some(Duration::from_secs(10)));
	}

	#[test]
	fn test_auto_dismiss_zero_uses_default() {
		let defaults = ManagerDefaults {
			default_entry_duration: Duration::from_millis(500),
			default_exit_duration: Duration::from_millis(750),
			default_display_time: Duration::from_secs(7),
		};
		let mut notification = create_test_notification();
		notification.auto_dismiss = AutoDismiss::After(Duration::ZERO);

		let state = NotificationState::new(1, notification, &defaults);
		assert_eq!(state.remaining_display_time, Some(Duration::from_secs(7)));
	}

	#[test]
	fn test_created_at_timestamp_is_set() {
		let defaults = ManagerDefaults::default();
		let notification = create_test_notification();
		let before = Instant::now();
		let state = NotificationState::new(1, notification, &defaults);
		let after = Instant::now();

		assert!(state.created_at >= before);
		assert!(state.created_at <= after);
	}

	#[test]
	fn test_manager_defaults_provides_sensible_values() {
		let defaults = ManagerDefaults::default();

		assert_eq!(defaults.default_entry_duration, Duration::from_millis(500));
		assert_eq!(defaults.default_exit_duration, Duration::from_millis(750));
		assert_eq!(defaults.default_display_time, Duration::from_secs(4));
	}

	#[test]
	fn test_id_is_stored_correctly() {
		let defaults = ManagerDefaults::default();
		let notification = create_test_notification();
		let state = NotificationState::new(42, notification, &defaults);

		assert_eq!(state.id, 42);
	}

	#[test]
	fn test_custom_positions_none_when_notification_has_none() {
		let defaults = ManagerDefaults::default();
		let notification = create_test_notification();
		let state = NotificationState::new(1, notification, &defaults);

		assert!(state.custom_entry_pos.is_none());
		assert!(state.custom_exit_pos.is_none());
	}

	#[test]
	fn test_custom_positions_copied_from_notification() {
		use tome_base::Position;

		let defaults = ManagerDefaults::default();
		let mut notification = create_test_notification();
		notification.custom_entry_position = Some(Position::new(10, 20));
		notification.custom_exit_position = Some(Position::new(100, 50));

		let state = NotificationState::new(1, notification, &defaults);

		assert_eq!(state.custom_entry_pos, Some((10.0, 20.0)));
		assert_eq!(state.custom_exit_pos, Some((100.0, 50.0)));
	}

	#[test]
	fn test_all_timing_fields_resolved() {
		let defaults = ManagerDefaults::default();
		let mut notification = create_test_notification();
		notification.slide_in_timing = Timing::Fixed(Duration::from_millis(100));
		notification.dwell_timing = Timing::Fixed(Duration::from_millis(200));
		notification.slide_out_timing = Timing::Fixed(Duration::from_millis(300));

		let state = NotificationState::new(1, notification, &defaults);

		assert_eq!(state.actual_entry_duration, Duration::from_millis(100));
		assert_eq!(state.actual_exit_duration, Duration::from_millis(300));
	}
}
