//! Async message bus for background task hydration.
//!
//! Background tasks send [`crate::msg::EditorMsg`] variants to update editor state after
//! first frame renders. The main loop drains messages before each draw,
//! aggregating [`crate::msg::Dirty`] flags to determine redraw needs.
//!
//! # Architecture
//!
//! ```text
//! Background Task ─┐
//!                  ├──► crate::msg::EditorMsg ──► drain_messages() ──► Editor state update
//! Background Task ─┘
//! ```
//!
//! Domain-specific messages wrap their payloads:
//! - [`crate::msg::ThemeMsg`] - Theme registry and active theme updates
//! - [`crate::msg::IoMsg`] - File load completion
//! - [`crate::msg::LspMsg`] - LSP catalog and server lifecycle

mod dirty;
mod io;
mod lsp;
mod overlay;
mod theme;

pub use dirty::Dirty;
pub use io::IoMsg;
pub use lsp::LspMsg;
pub use overlay::OverlayMsg;
pub use theme::ThemeMsg;
use tokio::sync::mpsc;

use crate::Editor;

/// Channel sender for background tasks.
pub type MsgSender = mpsc::UnboundedSender<EditorMsg>;

/// Channel receiver for the main loop.
pub type MsgReceiver = mpsc::UnboundedReceiver<EditorMsg>;

/// Creates a new message channel pair.
pub fn channel() -> (MsgSender, MsgReceiver) {
	mpsc::unbounded_channel()
}

/// Top-level message enum dispatched to editor state.
#[derive(Debug)]
pub enum EditorMsg {
	Theme(ThemeMsg),
	Io(IoMsg),
	Lsp(LspMsg),
	Overlay(OverlayMsg),
}

impl EditorMsg {
	/// Applies this message to the editor, returning dirty flags.
	pub fn apply(self, editor: &mut Editor) -> Dirty {
		match self {
			Self::Theme(msg) => msg.apply(editor),
			Self::Io(msg) => msg.apply(editor),
			Self::Lsp(msg) => msg.apply(editor),
			Self::Overlay(msg) => msg.apply(editor),
		}
	}
}

impl From<ThemeMsg> for EditorMsg {
	fn from(msg: ThemeMsg) -> Self {
		Self::Theme(msg)
	}
}

impl From<IoMsg> for EditorMsg {
	fn from(msg: IoMsg) -> Self {
		Self::Io(msg)
	}
}

impl From<LspMsg> for EditorMsg {
	fn from(msg: LspMsg) -> Self {
		Self::Lsp(msg)
	}
}

impl From<OverlayMsg> for EditorMsg {
	fn from(msg: OverlayMsg) -> Self {
		Self::Overlay(msg)
	}
}
