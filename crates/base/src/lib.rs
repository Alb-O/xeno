pub mod graphemes;
pub mod key;
pub mod mode;
pub mod pending;
pub mod prelude;
pub mod range;
pub mod selection;
pub mod transaction;

#[cfg(feature = "evildoer-tui")]
pub use evildoer_tui::style::{Color, Modifier, Style};
pub use key::{Key, KeyCode, Modifiers, MouseButton, MouseEvent, ScrollDirection};
pub use mode::Mode;
pub use pending::{ObjectSelectionKind, PendingKind};
pub use range::Range;
pub use ropey::{Rope, RopeSlice};
pub use selection::Selection;
pub use transaction::{ChangeSet, Transaction};
