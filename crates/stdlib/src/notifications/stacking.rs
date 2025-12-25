use std::collections::HashMap;
use std::time::Instant;

use ratatui::prelude::*;

use crate::notifications::layout::{calculate_anchor_position, calculate_rect};
use crate::notifications::types::{Anchor, AnimationPhase};

const STACKING_VERTICAL_SPACING: u16 = 1;

#[derive(Debug, Clone)]
pub struct StackedNotification {
	pub id: u64,
	pub rect: Rect,
}

pub trait StackableNotification {
	fn id(&self) -> u64;
	fn current_phase(&self) -> AnimationPhase;
	fn created_at(&self) -> Instant;
	fn full_rect(&self) -> Rect;
	fn exterior_padding(&self) -> u16;
	fn calculate_content_size(&self, frame_area: Rect) -> (u16, u16);
}

pub fn calculate_stacking_positions<T: StackableNotification>(
	notifications: &HashMap<u64, T>,
	anchor: Anchor,
	ids_at_anchor: &[u64],
	frame_area: Rect,
	max_concurrent: Option<usize>,
) -> Vec<StackedNotification> {
	let mut visible_states_data: Vec<(u64, Instant, u16, u16)> = ids_at_anchor
		.iter()
		.filter_map(|id| {
			notifications.get(id).and_then(|state| {
				let phase = state.current_phase();
				if phase != AnimationPhase::Finished && phase != AnimationPhase::Pending {
					let rect = state.full_rect();
					let (width, height) = if rect.height > 0 && rect.width > 0 {
						(rect.width, rect.height)
					} else {
						state.calculate_content_size(frame_area)
					};
					if height > 0 {
						Some((*id, state.created_at(), height, width))
					} else {
						None
					}
				} else {
					None
				}
			})
		})
		.collect();

	visible_states_data.sort_unstable_by_key(|&(_, created_at, _, _)| created_at);

	let max_concurrent = max_concurrent.unwrap_or(usize::MAX);
	let num_to_render = visible_states_data.len().min(max_concurrent);
	let candidate_data = &visible_states_data[visible_states_data.len() - num_to_render..];

	let is_stacking_up = matches!(
		anchor,
		Anchor::BottomLeft | Anchor::BottomCenter | Anchor::BottomRight
	);
	let anchor_pos = calculate_anchor_position(anchor, frame_area);
	let available_height = if is_stacking_up {
		anchor_pos.y.saturating_sub(frame_area.y)
	} else {
		frame_area.bottom().saturating_sub(anchor_pos.y)
	};

	let mut accumulated_height: u16 = 0;
	let mut result_list: Vec<StackedNotification> = Vec::with_capacity(num_to_render);

	let iter_order: Box<dyn Iterator<Item = &(u64, Instant, u16, u16)>> = if is_stacking_up {
		Box::new(candidate_data.iter().rev())
	} else {
		Box::new(candidate_data.iter())
	};

	for &(id, _, height, width) in iter_order {
		let spacing = if accumulated_height > 0 {
			STACKING_VERTICAL_SPACING
		} else {
			0
		};
		let offset = accumulated_height.saturating_add(spacing);
		let needed_height = height.saturating_add(spacing);

		if accumulated_height.saturating_add(needed_height) <= available_height {
			if let Some(state) = notifications.get(&id) {
				let base_full_rect = calculate_rect(
					anchor,
					anchor_pos,
					width,
					height,
					frame_area,
					state.exterior_padding(),
				);

				let stacked_y = if is_stacking_up {
					base_full_rect.y.saturating_sub(offset)
				} else {
					base_full_rect.y.saturating_add(offset)
				};

				let final_stacked_rect = Rect {
					x: base_full_rect.x,
					y: stacked_y
						.max(frame_area.y)
						.min(frame_area.bottom().saturating_sub(height)),
					width: base_full_rect.width,
					height,
				}
				.intersection(frame_area);

				if final_stacked_rect.width > 0 && final_stacked_rect.height > 0 {
					result_list.push(StackedNotification {
						id,
						rect: final_stacked_rect,
					});
					accumulated_height = accumulated_height.saturating_add(needed_height);
				} else {
					break;
				}
			}
		} else {
			break;
		}
	}

	result_list
}
