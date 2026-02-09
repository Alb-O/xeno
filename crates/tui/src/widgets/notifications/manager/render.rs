//! Rendering logic for toast notifications.

use std::time::Instant;

use super::super::types::{Anchor, Animation, AnimationPhase};
use super::layout::{
	STACK_SPACING, anchor_position, apply_animation, calculate_toast_size, calculate_x, calculate_y,
};
use super::state::ToastState;
use crate::animation::{Animatable, Easing};
use crate::buffer::Buffer;
use crate::layout::{Position, Rect};
use crate::style::Color;
use crate::text::Text;
use crate::widgets::paragraph::Wrap;
use crate::widgets::{Clear, Paragraph, Widget};

/// Renders a single toast to the buffer.
pub(super) fn render_toast(state: &ToastState, rect: Rect, buf: &mut Buffer) {
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
pub(super) fn calculate_opacity(state: &ToastState) -> f32 {
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
pub(super) fn sample_background(rect: Rect, buf: &Buffer) -> Vec<Color> {
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
pub(super) fn apply_opacity(rect: Rect, buf: &mut Buffer, opacity: f32, bg_colors: &[Color]) {
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

impl super::ToastManager {
	/// Renders all toasts for a specific anchor point.
	pub(super) fn render_anchor_group(
		&mut self,
		anchor: Anchor,
		ids: &[u64],
		area: Rect,
		buf: &mut Buffer,
	) {
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
}
