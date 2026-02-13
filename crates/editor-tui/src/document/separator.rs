//! Separator styling for split views.
//!
//! This module handles visual styling of separators between split views,
//! including hover effects, drag highlighting, and junction glyphs.

use xeno_editor::Editor;
use xeno_editor::render_api::SeparatorRenderTarget;
use xeno_editor::test_events::SeparatorAnimationEvent;
use xeno_tui::animation::Animatable;
use xeno_tui::layout::Rect;
use xeno_tui::style::{Color, Style};

/// Precomputed separator colors and state rects for efficient style lookups.
pub struct SeparatorStyle {
	/// Rect → state for separators with active interaction.
	hovered_rect: Option<Rect>,
	dragging_rect: Option<Rect>,
	anim_rect: Option<Rect>,
	anim_intensity: f32,
	/// Base colors per visual priority level (index = priority).
	base_bg: [Color; 2],
	base_fg: [Color; 2],
	hover_fg: Color,
	hover_bg: Color,
	drag_fg: Color,
	drag_bg: Color,
}

impl SeparatorStyle {
	/// Creates a new separator style from editor theme and separator targets.
	pub fn new(editor: &Editor, targets: &[SeparatorRenderTarget]) -> Self {
		let colors = &editor.config().theme.colors;

		let mut hovered_rect = None;
		let mut dragging_rect = None;
		let mut anim_rect = None;
		let mut anim_intensity = 0.0f32;

		for t in targets {
			let rect: Rect = t.rect().into();
			let state = t.state();
			if state.is_hovered() {
				hovered_rect = Some(rect);
			}
			if state.is_dragging() {
				dragging_rect = Some(rect);
			}
			if state.is_animating() {
				anim_rect = Some(rect);
				anim_intensity = state.anim_intensity();
			}
		}

		Self {
			hovered_rect,
			dragging_rect,
			anim_rect,
			anim_intensity,
			base_bg: [colors.ui.bg.into(), colors.popup.bg.into()],
			base_fg: [colors.ui.gutter_fg.into(), colors.popup.fg.into()],
			hover_fg: colors.ui.cursor_fg.into(),
			hover_bg: colors.ui.selection_bg.into(),
			drag_fg: colors.ui.bg.into(),
			drag_bg: colors.ui.fg.into(),
		}
	}

	/// Returns the style for a separator render target.
	pub fn for_target(&self, target: &SeparatorRenderTarget) -> Style {
		let idx = (target.priority() as usize).min(self.base_bg.len() - 1);
		let normal_fg = self.base_fg[idx];
		let normal_bg = self.base_bg[idx];
		let state = target.state();

		if state.is_dragging() {
			Style::default().fg(self.drag_fg).bg(self.drag_bg)
		} else if state.is_animating() {
			let intensity = state.anim_intensity();
			let fg = normal_fg.lerp(&self.hover_fg, intensity);
			let bg = normal_bg.lerp(&self.hover_bg, intensity);
			if let (Some(fg_rgb), Some(bg_rgb)) = (color_to_rgb(fg), color_to_rgb(bg)) {
				SeparatorAnimationEvent::frame(intensity, fg_rgb, bg_rgb);
			}
			Style::default().fg(fg).bg(bg)
		} else if state.is_hovered() {
			Style::default().fg(self.hover_fg).bg(self.hover_bg)
		} else {
			Style::default().fg(normal_fg).bg(normal_bg)
		}
	}

	/// Returns the style for a junction at the given position.
	///
	/// Checks if the position lies on a hovered/dragged/animated separator to maintain
	/// continuous highlight across junctions.
	pub fn for_junction(&self, x: u16, y: u16, priority: u8) -> Style {
		let point_on_rect = |rect: Rect| -> bool { x >= rect.x && x < rect.right() && y >= rect.y && y < rect.bottom() };

		let idx = (priority as usize).min(self.base_bg.len() - 1);
		let normal_fg = self.base_fg[idx];
		let normal_bg = self.base_bg[idx];

		if self.dragging_rect.is_some_and(point_on_rect) {
			Style::default().fg(self.drag_fg).bg(self.drag_bg)
		} else if self.anim_rect.is_some_and(point_on_rect) {
			let fg = normal_fg.lerp(&self.hover_fg, self.anim_intensity);
			let bg = normal_bg.lerp(&self.hover_bg, self.anim_intensity);
			Style::default().fg(fg).bg(bg)
		} else if self.hovered_rect.is_some_and(point_on_rect) {
			Style::default().fg(self.hover_fg).bg(self.hover_bg)
		} else {
			Style::default().fg(normal_fg).bg(normal_bg)
		}
	}
}

/// Extracts RGB components from a color, if it's an RGB color.
fn color_to_rgb(color: Color) -> Option<(u8, u8, u8)> {
	match color {
		Color::Rgb(r, g, b) => Some((r, g, b)),
		_ => None,
	}
}
