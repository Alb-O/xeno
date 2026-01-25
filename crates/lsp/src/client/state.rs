//! LSP server lifecycle state management.

/// LSP server lifecycle state.
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum ServerState {
	/// Process spawned, initialize in progress.
	Starting,
	/// initialize/initialized complete, ready for requests.
	Ready,
	/// Failed or exited.
	Dead,
}
