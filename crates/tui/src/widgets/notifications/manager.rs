//! Toast manager for handling multiple notifications.

use std::collections::HashMap;
use std::time::{Duration, Instant};
use std::vec::Vec;

use super::toast::Toast;
use super::types::{Anchor, Animation, AnimationPhase, AutoDismiss, Overflow, Timing};
use crate::animation::{Animatable, Easing};
use crate::buffer::Buffer;
use crate::layout::{Position, Rect};
use crate::style::Color;
use crate::text::Text;
use crate::widgets::paragraph::Wrap;
use crate::widgets::{Clear, Paragraph, Widget};

/// Default duration for toast entry animation.
const DEFAULT_ENTRY_DURATION: Duration = Duration::from_millis(300);
/// Default duration for toast exit animation.
const DEFAULT_EXIT_DURATION: Duration = Duration::from_millis(200);
/// Default dwell time before auto-dismissing a toast.
const DEFAULT_DWELL_DURATION: Duration = Duration::from_secs(4);
/// Vertical spacing between stacked toasts.
const STACK_SPACING: u16 = 1;

/// Internal state for a single toast notification.
#[derive(Debug)]
struct ToastState {
	/// The toast content and configuration.
	toast: Toast,
	/// Current animation phase.
	phase: AnimationPhase,
	/// Animation progress within current phase (0.0 to 1.0).
	progress: f32,
	/// When the toast was created.
	created_at: Instant,
	/// Time remaining before auto-dismiss (None = manual dismiss only).
	remaining_dwell: Option<Duration>,
	/// Duration of entry animation.
	entry_duration: Duration,
	/// Duration of exit animation.
	exit_duration: Duration,
	/// Computed rectangle at full visibility.
	full_rect: Rect,
	/// Number of stacked duplicate notifications (1 = no duplicates).
	stack_count: u32,
	/// Original dwell duration for resetting on stack increment.
	original_dwell: Option<Duration>,
}

impl ToastState {
	/// Creates a new toast state with default timings.
	fn new(toast: Toast) -> Self {
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
	fn update(&mut self, delta: Duration) {
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
	fn is_finished(&self) -> bool {
		self.phase == AnimationPhase::Finished
	}

	/// Increments the stack count and resets the dwell timer.
	fn increment_stack(&mut self) {
		self.stack_count = self.stack_count.saturating_add(1);
		self.remaining_dwell = self.original_dwell;
	}

	/// Returns true if this toast can be stacked with another having the same content.
	fn can_stack(&self) -> bool {
		!matches!(
			self.phase,
			AnimationPhase::Exiting | AnimationPhase::Finished
		)
	}
}

/// Converts an anchor to its screen position within the given area.
fn anchor_position(anchor: Anchor, area: Rect) -> Position {
	match anchor {
		Anchor::TopLeft => Position::new(area.x, area.y),
		Anchor::TopCenter => Position::new(area.x + area.width / 2, area.y),
		Anchor::TopRight => Position::new(area.right().saturating_sub(1), area.y),
		Anchor::MiddleLeft => Position::new(area.x, area.y + area.height / 2),
		Anchor::MiddleCenter => Position::new(area.x + area.width / 2, area.y + area.height / 2),
		Anchor::MiddleRight => {
			Position::new(area.right().saturating_sub(1), area.y + area.height / 2)
		}
		Anchor::BottomLeft => Position::new(area.x, area.bottom().saturating_sub(1)),
		Anchor::BottomCenter => {
			Position::new(area.x + area.width / 2, area.bottom().saturating_sub(1))
		}
		Anchor::BottomRight => Position::new(
			area.right().saturating_sub(1),
			area.bottom().saturating_sub(1),
		),
	}
}

/// Calculates the X position for a toast given anchor and dimensions.
fn calculate_x(anchor: Anchor, anchor_x: u16, width: u16, margin: u16, area: Rect) -> u16 {
	let x = match anchor {
		Anchor::TopCenter | Anchor::MiddleCenter | Anchor::BottomCenter => {
			anchor_x.saturating_sub(width / 2)
		}
		Anchor::TopRight | Anchor::MiddleRight | Anchor::BottomRight => {
			anchor_x.saturating_sub(width).saturating_sub(margin)
		}
		_ => anchor_x.saturating_add(margin),
	};
	x.clamp(area.x, area.right().saturating_sub(width))
}

/// Calculates the Y position for a toast given anchor and dimensions.
fn calculate_y(anchor: Anchor, anchor_y: u16, height: u16, margin: u16, area: Rect) -> u16 {
	let y = match anchor {
		Anchor::MiddleLeft | Anchor::MiddleCenter | Anchor::MiddleRight => {
			anchor_y.saturating_sub(height / 2)
		}
		Anchor::BottomLeft | Anchor::BottomCenter | Anchor::BottomRight => {
			anchor_y.saturating_sub(height).saturating_sub(margin)
		}
		_ => anchor_y.saturating_add(margin),
	};
	y.clamp(area.y, area.bottom().saturating_sub(height))
}

/// Returns the width needed to display the stack counter (e.g., "⨯12").
fn stack_counter_width(stack_count: u32) -> u16 {
	if stack_count <= 1 {
		return 0;
	}
	let digits = stack_count.checked_ilog10().unwrap_or(0) + 1;
	1 + digits as u16
}

/// Computes the toast dimensions based on content and constraints.
fn calculate_toast_size(toast: &Toast, area: Rect, stack_count: u32) -> (u16, u16) {
	use super::types::SizeConstraint;

	let max_width = toast
		.max_width
		.map(|c| match c {
			SizeConstraint::Cells(w) => w.min(area.width),
			SizeConstraint::Percent(p) => {
				((area.width as f32 * p.clamp(0.0, 1.0)).ceil() as u16).max(1)
			}
		})
		.unwrap_or(area.width);

	let max_height = toast
		.max_height
		.map(|c| match c {
			SizeConstraint::Cells(h) => h.min(area.height),
			SizeConstraint::Percent(p) => {
				((area.height as f32 * p.clamp(0.0, 1.0)).ceil() as u16).max(1)
			}
		})
		.unwrap_or(area.height);

	let padding_h = toast.padding.left + toast.padding.right;
	let padding_v = toast.padding.top + toast.padding.bottom;
	let icon_width = toast.icon_column_width();
	let counter_width = stack_counter_width(stack_count);

	let content_width = toast
		.content
		.lines()
		.map(|l| l.chars().count())
		.max()
		.unwrap_or(0) as u16;

	let width = (content_width.max(counter_width) + icon_width + 2 + padding_h)
		.max(3)
		.min(max_width);

	// Account for text wrapping when calculating height
	let inner_width = width.saturating_sub(2 + padding_h + icon_width);
	let wrapped_lines: u16 = if inner_width > 0 {
		toast
			.content
			.lines()
			.map(|line| {
				let len = line.chars().count() as u16;
				if len == 0 {
					1
				} else {
					len.div_ceil(inner_width)
				}
			})
			.sum::<u16>()
			.max(1)
	} else {
		1
	};

	let extra_lines = if stack_count > 1 { 1 } else { 0 };
	let height = (wrapped_lines + extra_lines + 2 + padding_v)
		.max(3)
		.min(max_height);

	(width, height)
}

/// Applies animation transforms to compute the current visible rect.
fn apply_animation(state: &ToastState, full_rect: Rect, area: Rect) -> Rect {
	let progress = Easing::EaseOut.apply(state.progress);

	match state.phase {
		AnimationPhase::Pending => Rect::default(),
		AnimationPhase::Dwelling | AnimationPhase::Finished => full_rect,
		AnimationPhase::Entering => match state.toast.animation {
			Animation::Fade => full_rect,
			Animation::Slide => offscreen_rect(state, full_rect, area).lerp(&full_rect, progress),
			Animation::ExpandCollapse => {
				let cx = full_rect.x + full_rect.width / 2;
				let cy = full_rect.y + full_rect.height / 2;
				Rect::new(cx, cy, 1, 1).lerp(&full_rect, progress)
			}
		},
		AnimationPhase::Exiting => {
			let progress = Easing::EaseIn.apply(state.progress);
			match state.toast.animation {
				Animation::Fade => full_rect,
				Animation::Slide => {
					full_rect.lerp(&offscreen_rect(state, full_rect, area), progress)
				}
				Animation::ExpandCollapse => {
					let cx = full_rect.x + full_rect.width / 2;
					let cy = full_rect.y + full_rect.height / 2;
					full_rect.lerp(&Rect::new(cx, cy, 1, 1), progress)
				}
			}
		}
	}
}

/// Calculates the offscreen position for slide animations.
fn offscreen_rect(state: &ToastState, full_rect: Rect, area: Rect) -> Rect {
	use super::types::SlideDirection;

	let direction = match state.toast.slide_direction {
		SlideDirection::Auto => default_slide_direction(state.toast.anchor),
		other => other,
	};

	let (dx, dy): (i32, i32) = match direction {
		SlideDirection::FromTop => (0, -(full_rect.height as i32 + 2)),
		SlideDirection::FromBottom => (0, area.height as i32 + 2),
		SlideDirection::FromLeft => (-(full_rect.width as i32 + 2), 0),
		SlideDirection::FromRight => (area.width as i32 + 2, 0),
		SlideDirection::FromTopLeft => (
			-(full_rect.width as i32 + 2),
			-(full_rect.height as i32 + 2),
		),
		SlideDirection::FromTopRight => (area.width as i32 + 2, -(full_rect.height as i32 + 2)),
		SlideDirection::FromBottomLeft => (-(full_rect.width as i32 + 2), area.height as i32 + 2),
		SlideDirection::FromBottomRight => (area.width as i32 + 2, area.height as i32 + 2),
		SlideDirection::Auto => (0, 0),
	};

	Rect::new(
		(full_rect.x as i32 + dx).max(0) as u16,
		(full_rect.y as i32 + dy).max(0) as u16,
		full_rect.width,
		full_rect.height,
	)
}

/// Determines the default slide direction for a given anchor.
fn default_slide_direction(anchor: Anchor) -> super::types::SlideDirection {
	use super::types::SlideDirection;
	match anchor {
		Anchor::TopLeft => SlideDirection::FromTopLeft,
		Anchor::TopCenter => SlideDirection::FromTop,
		Anchor::TopRight => SlideDirection::FromTopRight,
		Anchor::MiddleLeft => SlideDirection::FromLeft,
		Anchor::MiddleCenter => SlideDirection::FromLeft,
		Anchor::MiddleRight => SlideDirection::FromRight,
		Anchor::BottomLeft => SlideDirection::FromBottomLeft,
		Anchor::BottomCenter => SlideDirection::FromBottom,
		Anchor::BottomRight => SlideDirection::FromBottomRight,
	}
}

/// Renders a single toast to the buffer.
fn render_toast(state: &ToastState, rect: Rect, buf: &mut Buffer) {
	let opacity = calculate_opacity(state);
	let bg_colors = if opacity < 1.0 {
		Some(sample_background(rect, buf))
	} else {
		None
	};

	Clear.render(rect, buf);

	let block = state.toast.to_block();
	let inner = block.inner(rect);
	block.render(rect, buf);

	let content_area = if let Some(ref icon) = state.toast.icon {
		let icon_width = state.toast.icon_column_width();
		if inner.width > icon_width {
			buf.set_string(inner.x, inner.y, &icon.glyph, icon.style);
			Rect::new(
				inner.x + icon_width,
				inner.y,
				inner.width - icon_width,
				inner.height,
			)
		} else {
			inner
		}
	} else {
		inner
	};

	Paragraph::new(Text::raw(&state.toast.content))
		.wrap(Wrap { trim: true })
		.render(content_area, buf);

	if state.stack_count > 1 && inner.height > 0 && inner.width > 0 {
		let count_str = format!("\u{2a2f}{}", state.stack_count);
		let count_width = count_str.chars().count() as u16;
		buf.set_string(
			inner.right().saturating_sub(count_width),
			inner.bottom().saturating_sub(1),
			&count_str,
			state.toast.border_style,
		);
	}

	if let Some(bg) = bg_colors {
		apply_opacity(rect, buf, opacity, &bg);
	}
}

/// Computes the current opacity based on animation phase and progress.
fn calculate_opacity(state: &ToastState) -> f32 {
	if !state.toast.fade_effect && !matches!(state.toast.animation, Animation::Fade) {
		return 1.0;
	}

	match state.phase {
		AnimationPhase::Entering => Easing::EaseOut.apply(state.progress),
		AnimationPhase::Exiting => 1.0 - Easing::EaseIn.apply(state.progress),
		AnimationPhase::Dwelling => 1.0,
		_ => 0.0,
	}
}

/// Captures background colors for opacity blending.
fn sample_background(rect: Rect, buf: &Buffer) -> Vec<Color> {
	let mut colors = Vec::with_capacity((rect.width as usize) * (rect.height as usize));
	for y in rect.y..rect.bottom() {
		for x in rect.x..rect.right() {
			let color = buf
				.cell(Position::new(x, y))
				.map(|c| c.bg)
				.unwrap_or(Color::Reset);
			colors.push(color);
		}
	}
	colors
}

/// Blends toast colors with background based on opacity.
fn apply_opacity(rect: Rect, buf: &mut Buffer, opacity: f32, bg_colors: &[Color]) {
	let width = rect.width as usize;
	for y in rect.y..rect.bottom() {
		for x in rect.x..rect.right() {
			let idx = ((y - rect.y) as usize) * width + ((x - rect.x) as usize);
			let bg = bg_colors.get(idx).copied().unwrap_or(Color::Reset);
			if let Some(cell) = buf.cell_mut(Position::new(x, y)) {
				cell.fg = bg.lerp(&cell.fg, opacity);
				cell.bg = bg.lerp(&cell.bg, opacity);
			}
		}
	}
}

/// Manages multiple toast notifications with lifecycle, animations, and stacking.
#[derive(Debug)]
pub struct ToastManager {
	/// Active toast states keyed by ID.
	states: HashMap<u64, ToastState>,
	/// Next ID to assign to a new toast.
	next_id: u64,
	/// Maximum number of visible toasts per anchor (None = unlimited).
	max_visible: Option<usize>,
	/// Behavior when max_visible is exceeded.
	overflow: Overflow,
}

impl Default for ToastManager {
	fn default() -> Self {
		Self::new()
	}
}

impl ToastManager {
	/// Creates a new empty toast manager.
	pub fn new() -> Self {
		Self {
			states: HashMap::new(),
			next_id: 0,
			max_visible: None,
			overflow: Overflow::default(),
		}
	}

	/// Sets the maximum number of visible toasts.
	#[must_use]
	pub fn max_visible(mut self, max: Option<usize>) -> Self {
		self.max_visible = max;
		self
	}

	/// Sets the overflow behavior when the limit is reached.
	#[must_use]
	pub fn overflow(mut self, overflow: Overflow) -> Self {
		self.overflow = overflow;
		self
	}

	/// Adds a toast and returns its ID.
	///
	/// If a toast with identical content and anchor already exists (and is not
	/// exiting), increments its stack count and resets the dismiss timer.
	pub fn push(&mut self, toast: Toast) -> u64 {
		if let Some((&id, state)) = self.states.iter_mut().find(|(_, s)| {
			s.can_stack() && s.toast.anchor == toast.anchor && s.toast.content == toast.content
		}) {
			state.increment_stack();
			return id;
		}

		let id = self.next_id;
		self.next_id = self.next_id.wrapping_add(1);

		if let Some(max) = self.max_visible {
			while self.states.len() >= max {
				let to_remove = match self.overflow {
					Overflow::DropOldest => self.oldest_id(),
					Overflow::DropNewest => self.newest_id(),
				};
				if let Some(remove_id) = to_remove {
					self.states.remove(&remove_id);
				} else {
					break;
				}
			}
		}

		self.states.insert(id, ToastState::new(toast));
		id
	}

	/// Removes a toast by ID. Returns true if it existed.
	pub fn remove(&mut self, id: u64) -> bool {
		self.states.remove(&id).is_some()
	}

	/// Clears all toasts.
	pub fn clear(&mut self) {
		self.states.clear();
	}

	/// Returns true if there are no toasts.
	pub fn is_empty(&self) -> bool {
		self.states.is_empty()
	}

	/// Returns the number of active toasts.
	pub fn len(&self) -> usize {
		self.states.len()
	}

	/// Advances all toast animations and removes finished toasts.
	pub fn tick(&mut self, delta: Duration) {
		for state in self.states.values_mut() {
			state.update(delta);
		}
		self.states.retain(|_, state| !state.is_finished());
	}

	/// Renders all toasts to the buffer.
	pub fn render(&mut self, area: Rect, buf: &mut Buffer) {
		if self.states.is_empty() {
			return;
		}

		let mut by_anchor: HashMap<Anchor, Vec<u64>> = HashMap::new();
		for (&id, state) in &self.states {
			by_anchor.entry(state.toast.anchor).or_default().push(id);
		}

		for (anchor, ids) in by_anchor {
			self.render_anchor_group(anchor, &ids, area, buf);
		}
	}

	/// Renders all toasts for a specific anchor point.
	fn render_anchor_group(&mut self, anchor: Anchor, ids: &[u64], area: Rect, buf: &mut Buffer) {
		let mut sorted_ids: Vec<u64> = ids.to_vec();
		sorted_ids.sort_by_key(|id| {
			self.states
				.get(id)
				.map(|s| s.created_at)
				.unwrap_or(Instant::now())
		});

		let stacks_up = matches!(
			anchor,
			Anchor::BottomLeft | Anchor::BottomCenter | Anchor::BottomRight
		);
		let anchor_pos = anchor_position(anchor, area);

		let ordered: Vec<u64> = if stacks_up {
			sorted_ids.into_iter().rev().collect()
		} else {
			sorted_ids
		};

		let mut offset: u16 = 0;
		let mut render_data: Vec<(u64, Rect, Rect)> = Vec::new();

		for &id in &ordered {
			if let Some(state) = self.states.get(&id) {
				let (width, height) = calculate_toast_size(&state.toast, area, state.stack_count);
				if height == 0 {
					continue;
				}

				let x = calculate_x(anchor, anchor_pos.x, width, state.toast.margin, area);
				let base_y = calculate_y(anchor, anchor_pos.y, height, state.toast.margin, area);
				let y = if stacks_up {
					base_y.saturating_sub(offset)
				} else {
					base_y.saturating_add(offset)
				};

				let full_rect = Rect::new(x, y, width, height).intersection(area);
				let display_rect = apply_animation(state, full_rect, area);

				if display_rect.width > 0 && display_rect.height > 0 {
					render_data.push((id, full_rect, display_rect));
				}

				offset = offset.saturating_add(height).saturating_add(STACK_SPACING);
			}
		}

		for (id, full_rect, display_rect) in render_data {
			if let Some(state) = self.states.get_mut(&id) {
				state.full_rect = full_rect;
			}
			if let Some(state) = self.states.get(&id) {
				render_toast(state, display_rect, buf);
			}
		}
	}

	/// Returns the ID of the oldest toast.
	fn oldest_id(&self) -> Option<u64> {
		self.states
			.iter()
			.min_by_key(|(_, s)| s.created_at)
			.map(|(&id, _)| id)
	}

	/// Returns the ID of the newest toast.
	fn newest_id(&self) -> Option<u64> {
		self.states
			.iter()
			.max_by_key(|(_, s)| s.created_at)
			.map(|(&id, _)| id)
	}
}
