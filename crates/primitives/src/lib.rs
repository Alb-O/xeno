//! Core types for text editing: ranges, selections, transactions, and modes.

mod direction;
mod edit;
mod future;
mod geometry;
mod graphemes;
mod ids;
mod key;
mod lsp;
mod mode;
/// Movement helper functions for cursor manipulation.
pub mod movement;
mod pending;
mod prelude;
mod range;
mod rope;
mod selection;
mod style;
mod transaction;

pub use direction::{Axis, SeqDirection, SpatialDirection};
pub use edit::{CommitResult, EditCommit, EditError, EditOrigin, ReadOnlyReason, ReadOnlyScope, SyntaxPolicy, UndoPolicy};
pub use future::{BoxFutureLocal, BoxFutureSend, BoxFutureStatic, poll_once};
pub use geometry::{Position, Rect};
pub use graphemes::{next_grapheme_boundary, prev_grapheme_boundary};
pub use ids::{DocumentId, MotionId, ViewId, motion_ids};
pub use key::{Key, KeyCode, Modifiers, MouseButton, MouseEvent, ScrollDirection};
pub use lsp::{LspChangeSet, LspDocumentChange, LspPosition, LspRange};
pub use mode::Mode;
pub use pending::{ObjectSelectionKind, PendingKind};
pub use range::{CharIdx, Direction, Range};
pub use rope::{clamp_to_cell, max_cell_pos, max_cursor_pos, visible_line_count};
pub use ropey::{Rope, RopeSlice};
pub use selection::Selection;
pub use style::{Color, Modifier, Style, UnderlineStyle};
pub use transaction::{Bias, Change, ChangeSet, Operation, Tendril, Transaction};
