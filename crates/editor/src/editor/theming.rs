//! Theme and syntax highlighting.
//!
//! Theme management and highlight span collection.

use xeno_registry::commands::CommandError;
use xeno_registry::themes::SyntaxStyles;

use super::Editor;
use super::extensions::StyleMod;

impl Editor {
	/// Sets the editor's color theme by name.
	pub fn set_theme(&mut self, theme_name: &str) -> Result<(), CommandError> {
		if let Some(theme) = xeno_registry::themes::get_theme(theme_name) {
			self.config.theme = theme;
			Ok(())
		} else {
			let mut err = format!("Theme not found: {}", theme_name);
			if let Some(suggestion) = xeno_registry::themes::suggest_theme(theme_name) {
				err.push_str(&format!(". Did you mean '{}'?", suggestion));
			}
			Err(CommandError::Failed(err))
		}
	}

	/// Collects syntax highlight spans for the visible area of the buffer.
	pub fn collect_highlight_spans(
		&self,
		area: xeno_tui::layout::Rect,
	) -> Vec<(
		xeno_runtime_language::highlight::HighlightSpan,
		xeno_tui::style::Style,
	)> {
		let buffer = self.buffer();
		let scroll_line = buffer.scroll_line;

		buffer.with_doc(|doc| {
			let Some(ref syntax) = doc.syntax() else {
				return Vec::new();
			};

			let start_line = scroll_line;
			let end_line = (start_line + area.height as usize).min(doc.content().len_lines());

			let start_byte = doc.content().line_to_byte(start_line) as u32;
			let end_byte = if end_line < doc.content().len_lines() {
				doc.content().line_to_byte(end_line) as u32
			} else {
				doc.content().len_bytes() as u32
			};

			let highlight_styles = xeno_runtime_language::highlight::HighlightStyles::new(
				SyntaxStyles::scope_names(),
				|scope| self.config.theme.colors.syntax.resolve(scope),
			);

			let highlighter = syntax.highlighter(
				doc.content().slice(..),
				&self.config.language_loader,
				start_byte..end_byte,
			);

			highlighter
				.map(|span| {
					let abstract_style = highlight_styles.style_for_highlight(span.highlight);
					let xeno_tui_style: xeno_tui::style::Style = abstract_style;
					(span, xeno_tui_style)
				})
				.collect()
		})
	}

	/// Finds the style for a given byte position from precomputed highlight spans.
	pub fn style_for_byte_pos(
		&self,
		byte_pos: usize,
		spans: &[(
			xeno_runtime_language::highlight::HighlightSpan,
			xeno_tui::style::Style,
		)],
	) -> Option<xeno_tui::style::Style> {
		for (span, style) in spans.iter().rev() {
			if byte_pos >= span.start as usize && byte_pos < span.end as usize {
				return Some(*style);
			}
		}
		None
	}

	/// Applies any active style overlay (e.g., dimming) at the given byte position.
	pub fn apply_style_overlay(
		&self,
		byte_pos: usize,
		style: Option<xeno_tui::style::Style>,
	) -> Option<xeno_tui::style::Style> {
		use xeno_tui::animation::Animatable;

		let Some(modification) = self.style_overlays.modification_at(byte_pos) else {
			return style;
		};

		let style = style.unwrap_or_default();
		let modified = match modification {
			StyleMod::Dim(factor) => {
				// Convert theme bg color to xeno_tui color for blending
				let bg: xeno_tui::style::Color = self.config.theme.colors.ui.bg;
				if let Some(fg) = style.fg {
					// Blend fg toward bg using Animatable::lerp
					// factor=1.0 means no dimming (full fg), factor=0.0 means full bg
					let dimmed = bg.lerp(&fg, factor);
					style.fg(dimmed)
				} else {
					style.fg(xeno_tui::style::Color::DarkGray)
				}
			}
			StyleMod::Fg(color) => style.fg(color),
			StyleMod::Bg(color) => style.bg(color),
		};

		Some(modified)
	}
}
