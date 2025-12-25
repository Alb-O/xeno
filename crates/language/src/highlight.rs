//! Syntax highlighting types and utilities.
//!
//! This module bridges tree-sitter highlighting with Tome's theme system.

use ratatui::style::Style;

/// Maps highlight captures to styles.
///
/// This is the bridge between tree-sitter capture names (from .scm files)
/// and Tome's theme system.
pub struct HighlightStyles {
	/// Ordered list of scope names that we recognize.
	/// The index in this list corresponds to the Highlight index.
	scopes: Vec<String>,

	/// Resolver function that maps scope name to style.
	resolver: Box<dyn Fn(&str) -> Style + Send + Sync>,
}

impl HighlightStyles {
	/// Creates a new highlight styles mapper.
	///
	/// # Parameters
	/// - `scopes`: List of recognized scope names in order
	/// - `resolver`: Function that resolves a scope name to a style
	pub fn new<F>(scopes: Vec<String>, resolver: F) -> Self
	where
		F: Fn(&str) -> Style + Send + Sync + 'static,
	{
		Self {
			scopes,
			resolver: Box::new(resolver),
		}
	}

	/// Returns the list of recognized scopes.
	pub fn scopes(&self) -> &[String] {
		&self.scopes
	}

	/// Resolves a highlight index to a style.
	pub fn style_for_highlight(&self, index: u32) -> Style {
		self.scopes
			.get(index as usize)
			.map(|scope| (self.resolver)(scope))
			.unwrap_or_default()
	}

	/// Resolves a scope name to a style.
	pub fn style_for_scope(&self, scope: &str) -> Style {
		(self.resolver)(scope)
	}
}

impl std::fmt::Debug for HighlightStyles {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		f.debug_struct("HighlightStyles")
			.field("scopes", &self.scopes)
			.field("resolver", &"<fn>")
			.finish()
	}
}

#[cfg(test)]
mod tests {
	use ratatui::style::Color;

	use super::*;

	#[test]
	fn test_highlight_styles() {
		let scopes = vec!["keyword".to_string(), "string".to_string()];

		let styles = HighlightStyles::new(scopes, |scope| match scope {
			"keyword" => Style::default().fg(Color::Red),
			"string" => Style::default().fg(Color::Green),
			_ => Style::default(),
		});

		assert_eq!(styles.scopes().len(), 2);
		assert_eq!(
			styles.style_for_highlight(0),
			Style::default().fg(Color::Red)
		);
		assert_eq!(
			styles.style_for_highlight(1),
			Style::default().fg(Color::Green)
		);
	}
}
