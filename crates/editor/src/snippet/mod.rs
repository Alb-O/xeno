//! Snippet subsystem entry point.
//!
//! Exposes snippet parsing, rendering with variable resolution, and active
//! snippet-session state used for tabstop traversal and choice UI.

mod render;
mod session;
mod syntax;
pub(crate) mod vars;

pub use render::{SnippetVarResolver, render_with_resolver};
pub use session::SnippetChoiceOverlay;
pub use syntax::{TransformSource, parse_snippet_template};

/// Data-only snippet choice popup row for frontend rendering.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SnippetChoiceRenderItem {
	pub(crate) option: String,
	pub(crate) selected: bool,
}

impl SnippetChoiceRenderItem {
	pub fn new(option: String, selected: bool) -> Self {
		Self { option, selected }
	}

	pub fn option(&self) -> &str {
		&self.option
	}
	pub fn selected(&self) -> bool {
		self.selected
	}
}

/// Data-only snippet choice popup render plan.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SnippetChoiceRenderPlan {
	pub(crate) items: Vec<SnippetChoiceRenderItem>,
	pub(crate) max_option_width: usize,
	pub(crate) target_row_width: usize,
}

impl SnippetChoiceRenderPlan {
	pub fn new(items: Vec<SnippetChoiceRenderItem>, max_option_width: usize, target_row_width: usize) -> Self {
		Self {
			items,
			max_option_width,
			target_row_width,
		}
	}

	pub fn items(&self) -> &[SnippetChoiceRenderItem] {
		&self.items
	}
	pub fn max_option_width(&self) -> usize {
		self.max_option_width
	}
	pub fn target_row_width(&self) -> usize {
		self.target_row_width
	}
}
