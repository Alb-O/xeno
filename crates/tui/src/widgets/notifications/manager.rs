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

const DEFAULT_ENTRY_DURATION: Duration = Duration::from_millis(300);
const DEFAULT_EXIT_DURATION: Duration = Duration::from_millis(200);
const DEFAULT_DWELL_DURATION: Duration = Duration::from_secs(4);
const STACK_SPACING: u16 = 1;

#[derive(Debug)]
struct ToastState {
	toast: Toast,
	phase: AnimationPhase,
	progress: f32,
	created_at: Instant,
	remaining_dwell: Option<Duration>,
	entry_duration: Duration,
	exit_duration: Duration,
	full_rect: Rect,
}

impl ToastState {
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

		Self {
			toast,
			phase: AnimationPhase::Pending,
			progress: 0.0,
			created_at: Instant::now(),
			remaining_dwell,
			entry_duration,
			exit_duration,
			full_rect: Rect::default(),
		}
	}

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

		if self.phase == AnimationPhase::Dwelling {
			if let Some(remaining) = self.remaining_dwell.as_mut() {
				*remaining = remaining.saturating_sub(delta);
				if remaining.is_zero() {
					self.phase = AnimationPhase::Exiting;
					self.progress = 0.0;
				}
			}
		}
	}

	fn is_finished(&self) -> bool {
		self.phase == AnimationPhase::Finished
	}
}

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

fn calculate_toast_size(toast: &Toast, area: Rect) -> (u16, u16) {
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

	let content_lines = toast.content.lines().count().max(1) as u16;
	let content_width = toast
		.content
		.lines()
		.map(|l| l.chars().count())
		.max()
		.unwrap_or(0) as u16;

	let padding_h = toast.padding.left + toast.padding.right;
	let padding_v = toast.padding.top + toast.padding.bottom;
	let icon_width = toast.icon_column_width();

	let width = (content_width + icon_width + 2 + padding_h)
		.max(3)
		.min(max_width);
	let height = (content_lines + 2 + padding_v).max(3).min(max_height);

	(width, height)
}

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

fn render_toast(state: &ToastState, rect: Rect, buf: &mut Buffer) {
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

	let opacity = calculate_opacity(state);
	if opacity < 1.0 {
		apply_opacity(rect, buf, opacity);
	}
}

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

fn apply_opacity(rect: Rect, buf: &mut Buffer, opacity: f32) {
	let black = Color::Black;
	for y in rect.y..rect.bottom() {
		for x in rect.x..rect.right() {
			if let Some(cell) = buf.cell_mut(Position::new(x, y)) {
				cell.fg = black.lerp(&cell.fg, opacity);
				cell.bg = black.lerp(&cell.bg, opacity);
			}
		}
	}
}

/// Manages multiple toast notifications with lifecycle, animations, and stacking.
#[derive(Debug)]
pub struct ToastManager {
	states: HashMap<u64, ToastState>,
	next_id: u64,
	max_visible: Option<usize>,
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
	pub fn push(&mut self, toast: Toast) -> u64 {
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
				let (width, height) = calculate_toast_size(&state.toast, area);
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

	fn oldest_id(&self) -> Option<u64> {
		self.states
			.iter()
			.min_by_key(|(_, s)| s.created_at)
			.map(|(&id, _)| id)
	}

	fn newest_id(&self) -> Option<u64> {
		self.states
			.iter()
			.max_by_key(|(_, s)| s.created_at)
			.map(|(&id, _)| id)
	}
}
