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
//! * [`crate::msg::ThemeMsg`] - Theme registry and active theme updates
//! * [`crate::msg::IoMsg`] - File load completion
//! * [`crate::msg::LspMsg`] - LSP catalog and server lifecycle

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

/// Result of a completed async Nu hook evaluation.
#[derive(Debug)]
pub struct NuHookEvalDoneMsg {
	/// Matches the eval token assigned when the hook job is scheduled.
	pub token: crate::nu::coordinator::NuEvalToken,
	/// Produced effects or executor error (already retried internally).
	pub result: Result<crate::nu::NuEffectBatch, crate::nu::executor::NuExecError>,
}

/// Top-level message enum dispatched to editor state.
#[derive(Debug)]
pub enum EditorMsg {
	Theme(ThemeMsg),
	Io(IoMsg),
	Lsp(LspMsg),
	Overlay(OverlayMsg),
	/// Async Nu hook evaluation completed.
	NuHookEvalDone(NuHookEvalDoneMsg),
	/// A scheduled Nu macro timer fired.
	NuScheduleFired(crate::nu::coordinator::NuScheduleFiredMsg),
}

impl EditorMsg {
	/// Applies this message to the editor, returning dirty flags.
	pub fn apply(self, editor: &mut Editor) -> Dirty {
		match self {
			Self::Theme(msg) => msg.apply(editor),
			Self::Io(msg) => msg.apply(editor),
			Self::Lsp(msg) => msg.apply(editor),
			Self::Overlay(msg) => msg.apply(editor),
			Self::NuHookEvalDone(msg) => editor.apply_nu_hook_eval_done(msg),
			Self::NuScheduleFired(msg) => {
				if let Some(invocation) = editor.state.integration.nu.apply_schedule_fired(msg) {
					editor.enqueue_runtime_nu_invocation(invocation, crate::runtime::work_queue::RuntimeWorkSource::NuScheduledMacro);
				}
				Dirty::NONE
			}
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
