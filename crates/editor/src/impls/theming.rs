//! Theme and syntax highlighting.
//!
//! Theme management and highlight span collection.

use xeno_registry::commands::CommandError;
use xeno_registry::themes::SyntaxStyles;

use super::Editor;

impl Editor {
	/// Resolves and applies the configured theme after themes are registered.
	///
	/// Called by [`crate::msg::ThemeMsg::ThemesReady`] after background theme loading completes.
	/// Falls back to current theme if resolution fails.
	pub(crate) fn resolve_configured_theme(&mut self) {
		use xeno_registry::options::keys;
		let theme_id = self
			.state
			.config
			.global_options
			.get(
				xeno_registry::db::OPTIONS
					.get_key(&keys::THEME.untyped())
					.expect("theme option missing from registry")
					.dense_id(),
			)
			.and_then(|v| v.as_str())
			.map(|s| s.to_string())
			.unwrap_or_else(|| xeno_registry::themes::DEFAULT_THEME_ID.to_string());
		if let Err(e) = self.set_theme(&theme_id) {
			tracing::warn!(theme = %theme_id, error = %e, "Failed to resolve configured theme");
		}
	}

	/// Sets the editor's color theme by name.
	pub fn set_theme(&mut self, theme_name: &str) -> Result<(), CommandError> {
		if let Some(theme_ref) = xeno_registry::themes::get_theme(theme_name) {
			// Leak the name for RegistryMetaStatic since themes are rarely changed
			let name: &'static str = Box::leak(theme_name.to_string().into_boxed_str());
			self.state.config.theme = xeno_registry::themes::Theme {
				meta: xeno_registry::RegistryMetaStatic::minimal(name, name, ""),
				variant: theme_ref.variant,
				colors: theme_ref.colors,
			};
			// Increment theme epoch to invalidate highlight cache
			let new_epoch = self.state.render_cache.theme_epoch.wrapping_add(1);
			self.state.render_cache.set_theme_epoch(new_epoch);
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
	pub fn collect_highlight_spans(&self, area: xeno_tui::layout::Rect) -> Vec<(xeno_runtime_language::highlight::HighlightSpan, xeno_tui::style::Style)> {
		let buffer = self.buffer();
		let scroll_line = buffer.scroll_line;
		let doc_id = buffer.document_id();

		let Some(syntax) = self.state.syntax_manager.syntax_for_doc(doc_id) else {
			return Vec::new();
		};

		buffer.with_doc(|doc| {
			let content = doc.content();
			let total_lines = content.len_lines();
			let start_line = scroll_line.min(total_lines);
			let end_line = start_line.saturating_add(area.height as usize).min(total_lines);

			let start_byte = if start_line < total_lines {
				content.line_to_byte(start_line) as u32
			} else {
				content.len_bytes() as u32
			};
			let end_byte = if end_line < total_lines {
				content.line_to_byte(end_line) as u32
			} else {
				content.len_bytes() as u32
			};

			let highlight_styles = xeno_runtime_language::highlight::HighlightStyles::new(SyntaxStyles::scope_names(), |scope| {
				self.state.config.theme.colors.syntax.resolve(scope)
			});

			let highlighter = syntax.highlighter(content.slice(..), &self.state.config.language_loader, start_byte..end_byte);

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
		spans: &[(xeno_runtime_language::highlight::HighlightSpan, xeno_tui::style::Style)],
	) -> Option<xeno_tui::style::Style> {
		for (span, style) in spans.iter().rev() {
			if byte_pos >= span.start as usize && byte_pos < span.end as usize {
				return Some(*style);
			}
		}
		None
	}
}
