//! Core types for text editing: ranges, selections, transactions, and modes.

/// Directional types for navigation and layout operations.
pub mod direction;
/// Edit operation types: errors, policies, and results.
pub mod edit;
/// Async future aliases.
pub mod future;
/// Grapheme cluster boundary detection.
pub mod graphemes;
/// Identifier types for editor entities.
pub mod ids;
/// Key and mouse event types.
pub mod key;
/// LSP sync primitives (position/range/change).
pub mod lsp;
/// Editor mode definitions.
pub mod mode;
/// Movement helper functions for cursor manipulation.
pub mod movement;
/// Pending action state types.
pub mod pending;
/// Common re-exports for convenience.
pub mod prelude;
/// Text range types: byte, char, and line indices.
pub mod range;
/// Rope utilities and extensions.
pub mod rope;
/// Selection types for single and multi-cursor editing.
pub mod selection;
/// Undo/redo transaction primitives.
pub mod transaction;

// Shared style types are re-exported to avoid duplicating xeno-tui deps
// across multiple crates that parse themes and syntax styles.
pub use direction::{Axis, SeqDirection, SpatialDirection};
pub use edit::{
	CommitResult, EditCommit, EditError, EditOrigin, ReadOnlyReason, ReadOnlyScope, SyntaxOutcome,
	SyntaxPolicy, UndoPolicy,
};
pub use future::{BoxFutureLocal, BoxFutureSend, BoxFutureStatic};
pub use ids::{MotionId, ViewId, motion_ids};
pub use key::{Key, KeyCode, Modifiers, MouseButton, MouseEvent, ScrollDirection};
pub use lsp::{LspChangeSet, LspDocumentChange, LspPosition, LspRange};
pub use mode::Mode;
pub use pending::{ObjectSelectionKind, PendingKind};
pub use range::Range;
pub use rope::{max_cursor_pos, visible_line_count};
pub use ropey::{Rope, RopeSlice};
pub use selection::Selection;
pub use transaction::{ChangeSet, Transaction};
#[cfg(feature = "xeno-tui")]
pub use xeno_tui::style::{Color, Modifier, Style};
