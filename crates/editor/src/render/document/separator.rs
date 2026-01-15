//! Separator styling for split views.
//!
//! This module handles visual styling of separators between split views,
//! including hover effects, drag highlighting, and junction glyphs.

use xeno_tui::animation::Animatable;
use xeno_tui::layout::Rect;
use xeno_tui::style::{Color, Style};

use crate::Editor;
use crate::test_events::SeparatorAnimationEvent;

/// Extracts RGB components from a color, if it's an RGB color.
fn color_to_rgb(color: Color) -> Option<(u8, u8, u8)> {
	match color {
		Color::Rgb(r, g, b) => Some((r, g, b)),
		_ => None,
	}
}

/// Precomputed separator colors and state for efficient style lookups.
pub struct SeparatorStyle {
	/// Rectangle of the currently hovered separator.
	hovered_rect: Option<Rect>,
	/// Rectangle of the separator being dragged.
	dragging_rect: Option<Rect>,
	/// Rectangle of the separator being animated.
	anim_rect: Option<Rect>,
	/// Animation intensity (0.0 to 1.0) for hover transitions.
	anim_intensity: f32,
	/// Base colors per visual priority level (index = priority).
	base_bg: [Color; 2],
	/// Foreground colors per visual priority level.
	base_fg: [Color; 2],
	/// Foreground color for hovered separators.
	hover_fg: Color,
	/// Background color for hovered separators.
	hover_bg: Color,
	/// Foreground color for actively dragged separators.
	drag_fg: Color,
	/// Background color for actively dragged separators.
	drag_bg: Color,
}

impl SeparatorStyle {
	/// Creates a new separator style from the current editor state.
	pub fn new(editor: &Editor, doc_area: Rect) -> Self {
		Self {
			hovered_rect: editor.layout.hovered_separator.map(|(_, rect)| rect),
			dragging_rect: editor.layout.drag_state().and_then(|ds| {
				editor
					.layout
					.separator_rect(&editor.base_window().layout, doc_area, &ds.id)
			}),
			anim_rect: editor.layout.animation_rect(),
			anim_intensity: editor.layout.animation_intensity(),
			base_bg: [
				editor.config.theme.colors.ui.bg,
				editor.config.theme.colors.popup.bg,
			],
			base_fg: [
				editor.config.theme.colors.ui.gutter_fg,
				editor.config.theme.colors.popup.fg,
			],
			hover_fg: editor.config.theme.colors.ui.cursor_fg,
			hover_bg: editor.config.theme.colors.ui.selection_bg,
			drag_fg: editor.config.theme.colors.ui.bg,
			drag_bg: editor.config.theme.colors.ui.fg,
		}
	}

	/// Returns the style for a separator at the given rectangle and priority.
	pub fn for_rect(&self, rect: Rect, priority: u8) -> Style {
		let is_dragging = self.dragging_rect == Some(rect);
		let is_animating = self.anim_rect == Some(rect);
		let is_hovered = self.hovered_rect == Some(rect);

		let idx = (priority as usize).min(self.base_bg.len() - 1);
		let normal_fg = self.base_fg[idx];
		let normal_bg = self.base_bg[idx];

		if is_dragging {
			Style::default().fg(self.drag_fg).bg(self.drag_bg)
		} else if is_animating {
			let fg = normal_fg.lerp(&self.hover_fg, self.anim_intensity);
			let bg = normal_bg.lerp(&self.hover_bg, self.anim_intensity);
			if let (Some(fg_rgb), Some(bg_rgb)) = (color_to_rgb(fg), color_to_rgb(bg)) {
				SeparatorAnimationEvent::frame(self.anim_intensity, fg_rgb, bg_rgb);
			}
			Style::default().fg(fg).bg(bg)
		} else if is_hovered {
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
		let point_on_rect = |rect: Rect| -> bool {
			x >= rect.x && x < rect.right() && y >= rect.y && y < rect.bottom()
		};

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

/// Returns the box-drawing junction glyph for the given connectivity.
///
/// Connectivity is encoded as a 4-bit mask: up (0x1), down (0x2), left (0x4), right (0x8).
pub fn junction_glyph(connectivity: u8) -> char {
	match connectivity {
		0b1111 => '┼',
		0b1011 => '├',
		0b0111 => '┤',
		0b1110 => '┬',
		0b1101 => '┴',
		0b0011 => '│',
		0b1100 => '─',
		_ => '┼',
	}
}
