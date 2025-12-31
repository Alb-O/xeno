//! Theme and syntax highlighting.
//!
//! Theme management and highlight span collection.

use evildoer_manifest::syntax::SyntaxStyles;

use super::Editor;
use super::extensions::StyleMod;

impl Editor {
	pub fn set_theme(&mut self, theme_name: &str) -> Result<(), evildoer_manifest::CommandError> {
		if let Some(theme) = evildoer_manifest::get_theme(theme_name) {
			self.theme = theme;
			Ok(())
		} else {
			let mut err = format!("Theme not found: {}", theme_name);
			if let Some(suggestion) = evildoer_manifest::suggest_theme(theme_name) {
				err.push_str(&format!(". Did you mean '{}'?", suggestion));
			}
			Err(evildoer_manifest::CommandError::Failed(err))
		}
	}

	pub fn collect_highlight_spans(
		&self,
		area: evildoer_tui::layout::Rect,
	) -> Vec<(
		evildoer_language::highlight::HighlightSpan,
		evildoer_tui::style::Style,
	)> {
		let buffer = self.buffer();
		let doc = buffer.doc();

		let Some(ref syntax) = doc.syntax else {
			return Vec::new();
		};

		let start_line = buffer.scroll_line;
		let end_line = (start_line + area.height as usize).min(doc.content.len_lines());

		let start_byte = doc.content.line_to_byte(start_line) as u32;
		let end_byte = if end_line < doc.content.len_lines() {
			doc.content.line_to_byte(end_line) as u32
		} else {
			doc.content.len_bytes() as u32
		};

		let highlight_styles = evildoer_language::highlight::HighlightStyles::new(
			SyntaxStyles::scope_names(),
			|scope| self.theme.colors.syntax.resolve(scope),
		);

		let highlighter = syntax.highlighter(
			doc.content.slice(..),
			&self.language_loader,
			start_byte..end_byte,
		);

		highlighter
			.map(|span| {
				let abstract_style = highlight_styles.style_for_highlight(span.highlight);
				let evildoer_tui_style: evildoer_tui::style::Style = abstract_style;
				(span, evildoer_tui_style)
			})
			.collect()
	}

	pub fn style_for_byte_pos(
		&self,
		byte_pos: usize,
		spans: &[(
			evildoer_language::highlight::HighlightSpan,
			evildoer_tui::style::Style,
		)],
	) -> Option<evildoer_tui::style::Style> {
		for (span, style) in spans.iter().rev() {
			if byte_pos >= span.start as usize && byte_pos < span.end as usize {
				return Some(*style);
			}
		}
		None
	}

	pub fn apply_style_overlay(
		&self,
		byte_pos: usize,
		style: Option<evildoer_tui::style::Style>,
	) -> Option<evildoer_tui::style::Style> {
		use evildoer_tui::animation::Animatable;

		let Some(modification) = self.style_overlays.modification_at(byte_pos) else {
			return style;
		};

		let style = style.unwrap_or_default();
		let modified = match modification {
			StyleMod::Dim(factor) => {
				// Convert theme bg color to evildoer_tui color for blending
				let bg: evildoer_tui::style::Color = self.theme.colors.ui.bg;
				if let Some(fg) = style.fg {
					// Blend fg toward bg using Animatable::lerp
					// factor=1.0 means no dimming (full fg), factor=0.0 means full bg
					let dimmed = bg.lerp(&fg, factor);
					style.fg(dimmed)
				} else {
					style.fg(evildoer_tui::style::Color::DarkGray)
				}
			}
			StyleMod::Fg(color) => style.fg(color),
			StyleMod::Bg(color) => style.bg(color),
		};

		Some(modified)
	}
}
