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

bitflags::bitflags! {
	/// A set of editor capabilities.
	#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
	pub struct CapabilitySet: u64 {
		/// Read access to document text.
		const TEXT = 1 << 0;
		/// Access to cursor position.
		const CURSOR = 1 << 1;
		/// Access to selection state.
		const SELECTION = 1 << 2;
		/// Access to editor mode.
		const MODE = 1 << 3;
		/// Ability to display messages and notifications.
		const MESSAGING = 1 << 4;
		/// Ability to modify document text.
		const EDIT = 1 << 5;
		/// Access to search functionality.
		const SEARCH = 1 << 6;
		/// Access to undo/redo history.
		const UNDO = 1 << 7;
		/// Access to file system operations.
		const FILE_OPS = 1 << 8;
		/// Access to UI overlays.
		const OVERLAY = 1 << 9;
	}
}

impl Capability {
	/// Returns the bitflag for this capability.
	pub const fn as_set(self) -> CapabilitySet {
		match self {
			Self::Text => CapabilitySet::TEXT,
			Self::Cursor => CapabilitySet::CURSOR,
			Self::Selection => CapabilitySet::SELECTION,
			Self::Mode => CapabilitySet::MODE,
			Self::Messaging => CapabilitySet::MESSAGING,
			Self::Edit => CapabilitySet::EDIT,
			Self::Search => CapabilitySet::SEARCH,
			Self::Undo => CapabilitySet::UNDO,
			Self::FileOps => CapabilitySet::FILE_OPS,
			Self::Overlay => CapabilitySet::OVERLAY,
		}
	}
}

impl From<Capability> for CapabilitySet {
	fn from(cap: Capability) -> Self {
		cap.as_set()
	}
}

impl FromIterator<Capability> for CapabilitySet {
	fn from_iter<I: IntoIterator<Item = Capability>>(iter: I) -> Self {
		let mut set = CapabilitySet::empty();
		for cap in iter {
			set |= cap.as_set();
		}
		set
	}
}
