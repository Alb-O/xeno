//! Style composition layers for buffer rendering.
//!
//! This module provides a unified system for computing line-level backgrounds
//! by composing multiple style layers (diff, cursorline, selection) with
//! well-defined blend constants.

use xeno_primitives::Color;

/// Named blend constants for style composition.
///
/// These constants define how different style layers blend together.
/// All use the `blend(other, alpha)` convention where alpha=1 means 100% self.
pub mod blend {
	/// Cursorline blends 92% background, 8% mode color.
	pub const CURSORLINE_ALPHA: f32 = 0.92;
	/// Selection background blends 78% background, 22% mode color.
	pub const SELECTION_MODE_ALPHA: f32 = 0.78;
	/// Selection then blends 88% of above, 12% syntax foreground tint.
	pub const SELECTION_SYNTAX_ALPHA: f32 = 0.88;
	/// Gutter dim text blends 50% toward background.
	pub const GUTTER_DIM_ALPHA: f32 = 0.5;
	/// Minimum contrast ratio for selection backgrounds.
	pub const SELECTION_MIN_CONTRAST: f32 = 1.5;
}

/// Line-level style context for computing backgrounds.
///
/// This provides a single source of truth for fill backgrounds, replacing
/// the duplicated fill logic scattered throughout buffer rendering.
#[derive(Debug, Clone, Copy)]
pub struct LineStyleContext {
	/// Base background color from theme.
	pub base_bg: Color,
	/// Diff line background if this is a diff addition/deletion/hunk.
	pub diff_bg: Option<Color>,
	/// Mode color for cursorline/selection blending.
	pub mode_color: Color,
	/// Whether this is the cursor line.
	pub is_cursor_line: bool,
	/// Whether cursorline highlighting is enabled globally.
	pub cursorline_enabled: bool,
	/// Cursor line index.
	pub cursor_line: usize,
	/// Whether this line is in the nontext area.
	pub is_nontext: bool,
}

impl LineStyleContext {
	/// Computes the fill background for empty space on this line.
	///
	/// This is the single source of truth for fill backgrounds, replacing
	/// the 4+ duplicated fill blocks in the original render code.
	///
	/// Priority:
	/// 1. If cursorline: blend diff_bg (or base_bg) with mode color
	/// 2. If diff line: use diff_bg directly
	/// 3. Otherwise: no fill background
	pub fn fill_bg(&self) -> Option<Color> {
		if self.should_highlight_cursorline() {
			let bg = self.diff_bg.unwrap_or(self.base_bg);
			Some(bg.blend(self.mode_color, blend::CURSORLINE_ALPHA))
		} else {
			self.diff_bg
		}
	}

	/// Computes the cursorline background color.
	///
	/// If there's a diff background, blends mode color into that.
	/// Otherwise blends mode color into base background.
	pub fn cursorline_bg(&self) -> Color {
		let bg = self.diff_bg.unwrap_or(self.base_bg);
		bg.blend(self.mode_color, blend::CURSORLINE_ALPHA)
	}

	/// Computes the cell background for a character with syntax/selection state.
	///
	/// # Arguments
	/// * `syntax_bg` - Background from syntax highlighting (if any)
	/// * `in_selection` - Whether this character is in a selection range
	/// * `syntax_fg` - Foreground from syntax highlighting (for selection tint)
	///
	/// # Returns
	/// The computed background color, or None if default should be used.
	#[allow(dead_code, reason = "utility method, cell_style module handles full style resolution")]
	pub fn cell_bg(&self, syntax_bg: Option<Color>, in_selection: bool, syntax_fg: Option<Color>) -> Option<Color> {
		if in_selection {
			let fg = syntax_fg.unwrap_or(self.base_bg);
			let selection_bg = self
				.base_bg
				.blend(self.mode_color, blend::SELECTION_MODE_ALPHA)
				.blend(fg, blend::SELECTION_SYNTAX_ALPHA)
				.ensure_min_contrast(self.base_bg, blend::SELECTION_MIN_CONTRAST);
			Some(selection_bg)
		} else if self.should_highlight_cursorline() {
			Some(
				syntax_bg
					.map(|bg| bg.blend(self.mode_color, blend::CURSORLINE_ALPHA))
					.unwrap_or_else(|| self.cursorline_bg()),
			)
		} else {
			syntax_bg
		}
	}

	/// Returns the cursorline background color.
	pub fn gutter_cursorline_bg(&self) -> Color {
		self.cursorline_bg()
	}

	/// Returns whether cursorline highlighting should apply.
	pub fn should_highlight_cursorline(&self) -> bool {
		self.cursorline_enabled && self.is_cursor_line
	}
}

#[cfg(test)]
mod tests;
