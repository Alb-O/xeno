mod render;
mod session;
mod syntax;
pub(crate) mod vars;

pub use render::{RenderedSnippet, RenderedTransform, SnippetVarResolver, render, render_with_resolver};
pub use session::{SnippetChoiceOverlay, SnippetSession, SnippetSessionState};
pub use syntax::{Field, FieldKind, Node, SnippetParseError, SnippetTemplate, Transform, TransformSource, Var, parse_snippet_template};
