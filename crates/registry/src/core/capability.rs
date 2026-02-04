/// Represents an editor capability required by a registry item.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Capability {
	/// Read access to document text.
	Text,
	/// Access to cursor position.
	Cursor,
	/// Access to selection state.
	Selection,
	/// Access to editor mode (normal, insert, visual).
	Mode,
	/// Ability to display messages and notifications.
	Messaging,
	/// Ability to modify document text.
	Edit,
	/// Access to search functionality.
	Search,
	/// Access to undo/redo history.
	Undo,
	/// Access to file system operations.
	FileOps,
	/// Access to UI overlays and modal interactions.
	Overlay,
}
