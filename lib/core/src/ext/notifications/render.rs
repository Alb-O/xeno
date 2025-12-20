use std::collections::HashMap;

use ratatui::layout::Alignment;
use ratatui::prelude::*;
use ratatui::symbols::border;
use ratatui::widgets::block::Padding;
use ratatui::widgets::{Block, BorderType, Borders, Clear};

use crate::ext::notifications::stacking::{StackableNotification, calculate_stacking_positions};
use crate::ext::notifications::types::{Anchor, AnimationPhase, Level};
use crate::ext::notifications::ui::{
	get_level_icon, gutter_layout, render_body, render_icon_gutter, resolve_styles, split_inner,
};

pub trait RenderableNotification: StackableNotification {
	fn level(&self) -> Option<Level>;
	fn title(&self) -> Option<Line<'static>>;
	fn content(&self) -> Text<'static>;
	fn border_type(&self) -> BorderType;
	fn fade_effect(&self) -> bool;
	fn animation_type(&self) -> crate::ext::notifications::types::Animation;
	fn animation_progress(&self) -> f32;
	fn block_style(&self) -> Option<Style>;
	fn border_style(&self) -> Option<Style>;
	fn title_style(&self) -> Option<Style>;
	fn padding(&self) -> Padding;
	fn set_full_rect(&mut self, rect: Rect);

	fn calculate_animation_rect(&self, frame_area: Rect) -> Rect;
	fn apply_animation_block_effect<'a>(
		&self,
		block: Block<'a>,
		frame_area: Rect,
		base_set: &border::Set<'a>,
	) -> Block<'a>;
	fn interpolate_frame_foreground(
		&self,
		base_fg: Option<Color>,
		phase: AnimationPhase,
		progress: f32,
	) -> Option<Color>;
	fn interpolate_content_foreground(
		&self,
		base_fg: Option<Color>,
		phase: AnimationPhase,
		progress: f32,
	) -> Option<Color>;
}

pub fn render_notifications<T: RenderableNotification>(
	notifications: &mut HashMap<u64, T>,
	notifications_by_anchor: &HashMap<Anchor, Vec<u64>>,
	frame: &mut Frame<'_>,
	area: Rect,
	max_concurrent: Option<usize>,
) {
	let frame_area = area;

	for (anchor, ids_at_anchor) in notifications_by_anchor.iter() {
		if ids_at_anchor.is_empty() {
			continue;
		}

		let stacked_notifications = calculate_stacking_positions(
			notifications,
			*anchor,
			ids_at_anchor,
			frame_area,
			max_concurrent,
		);

		for stacked in stacked_notifications {
			if let Some(state) = notifications.get_mut(&stacked.id) {
				state.set_full_rect(stacked.rect);

				let current_rect = state.calculate_animation_rect(frame_area);
				if current_rect.width == 0 || current_rect.height == 0 {
					continue;
				}

				let (base_block_style, base_border_style, base_title_style) = resolve_styles(
					state.level(),
					state.block_style(),
					state.border_style(),
					state.title_style(),
				);

				let (final_block_style, final_border_style, final_title_style, final_content_style) =
					apply_fade_if_needed(
						state,
						base_block_style,
						base_border_style,
						base_title_style,
					);

				let mut block = Block::default()
					.style(final_block_style)
					.borders(Borders::ALL)
					.border_type(state.border_type())
					.border_style(final_border_style)
					.padding(state.padding());

				if state.border_type() == BorderType::Padded {
					block = block.border_set(ratatui::symbols::border::Set {
						top_left: "▏",
						vertical_left: "▏",
						bottom_left: "▏",
						..ratatui::symbols::border::EMPTY
					});
				}

				if let Some(title_line) = state.title() {
					block = block.title(
						title_line
							.clone()
							.alignment(Alignment::Center)
							.style(final_title_style),
					);
				}

				let border_set = get_border_set(state.border_type());
				block = state.apply_animation_block_effect(block, frame_area, &border_set);

				if stacked.rect.width > 0 && stacked.rect.height > 0 {
					frame.render_widget(Clear, stacked.rect.intersection(frame_area));
				}

				let inner_area = block.inner(current_rect);
				frame.render_widget(block, current_rect);

				let gutter = gutter_layout(state.level());
				if let (Some(g), Some(icon)) = (gutter, get_level_icon(state.level())) {
					let (gutter_area, content_area) = split_inner(inner_area, g);
					if gutter_area.width > 0 && content_area.width > 0 {
						render_icon_gutter(frame, gutter_area, g, icon, final_border_style);
						render_body(frame, content_area, state.content(), final_content_style);
					} else {
						render_body(frame, inner_area, state.content(), final_content_style);
					}
				} else {
					render_body(frame, inner_area, state.content(), final_content_style);
				}
			}
		}
	}
}

fn apply_fade_if_needed<T: RenderableNotification>(
	state: &T,
	base_block_style: Style,
	base_border_style: Style,
	base_title_style: Style,
) -> (Style, Style, Style, Style) {
	use crate::ext::notifications::types::Animation;

	let apply_fade = state.fade_effect() || matches!(state.animation_type(), Animation::Fade);
	let is_in_anim_phase = matches!(
		state.current_phase(),
		AnimationPhase::FadingIn
			| AnimationPhase::FadingOut
			| AnimationPhase::SlidingIn
			| AnimationPhase::SlidingOut
			| AnimationPhase::Expanding
			| AnimationPhase::Collapsing
	);
	let is_dwelling = matches!(state.current_phase(), AnimationPhase::Dwelling);

	if apply_fade && (is_in_anim_phase || is_dwelling) {
		let phase = state.current_phase();
		let progress = if is_dwelling {
			1.0
		} else {
			state.animation_progress()
		};
		let effective_phase = if is_dwelling {
			AnimationPhase::FadingIn
		} else {
			phase
		};

		let effective_base_frame_fg = base_title_style
			.fg
			.or(base_border_style.fg)
			.or(base_block_style.fg);

		let frame_fg =
			state.interpolate_frame_foreground(effective_base_frame_fg, effective_phase, progress);
		let content_fg = state.interpolate_content_foreground(None, effective_phase, progress);

		let frame_fade_override = Style::default().fg(frame_fg.unwrap_or(Color::Reset));
		let content_fade_override = Style::default().fg(content_fg.unwrap_or(Color::Reset));

		(
			base_block_style.patch(frame_fade_override),
			base_border_style.patch(frame_fade_override),
			base_title_style.patch(frame_fade_override),
			base_block_style.patch(content_fade_override),
		)
	} else {
		(
			base_block_style,
			base_border_style,
			base_title_style,
			base_block_style,
		)
	}
}

fn get_border_set(border_type: BorderType) -> border::Set<'static> {
	match border_type {
		BorderType::Plain => border::PLAIN,
		BorderType::Rounded => border::ROUNDED,
		BorderType::Double => border::DOUBLE,
		BorderType::Thick => border::THICK,
		BorderType::Padded => border::EMPTY,
		_ => border::PLAIN,
	}
}
