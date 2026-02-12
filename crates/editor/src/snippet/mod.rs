mod render;
mod session;
mod syntax;
pub(crate) mod vars;

pub use render::{RenderedSnippet, RenderedTransform, SnippetVarResolver, render, render_with_resolver};
pub use session::{SnippetChoiceOverlay, SnippetSession, SnippetSessionState};
pub use syntax::{Field, FieldKind, Node, SnippetParseError, SnippetTemplate, Transform, TransformSource, Var, parse_snippet_template};

/// Data-only snippet choice popup row for frontend rendering.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SnippetChoiceRenderItem {
	pub option: String,
	pub selected: bool,
}

/// Data-only snippet choice popup render plan.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SnippetChoiceRenderPlan {
	pub items: Vec<SnippetChoiceRenderItem>,
	pub max_option_width: usize,
	pub target_row_width: usize,
}
