//!
//! Panel infrastructure for dock-style split views.
//!
//! Panels are toggleable split views like terminals, debug logs, file trees, etc.
//! This module provides the compile-time registry infrastructure; runtime state
//! management lives in `evildoer-api`.
//!
//! Define panels with the [`panel!`](crate::panel) macro, which registers a
//! [`PanelDef`] and optionally a [`PanelFactoryDef`] for creating instances.

use linkme::distributed_slice;

mod macros;
mod split_buffer;

pub use evildoer_registry_motions::RegistrySource;
pub use split_buffer::{
	SplitAttrs, SplitBuffer, SplitCell, SplitColor, SplitCursor, SplitCursorStyle,
	SplitDockPreference, SplitEventResult, SplitKey, SplitKeyCode, SplitModifiers, SplitMouse,
	SplitMouseAction, SplitMouseButton, SplitSize,
};

/// Unique identifier for a panel instance.
///
/// Panel IDs are assigned by the panel registry when a panel is created.
/// The `kind` field identifies the panel type (e.g., "terminal", "debug"),
/// and `instance` distinguishes multiple panels of the same type.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct PanelId {
	/// Index into the PANELS slice identifying the panel type.
	pub kind: u16,
	/// Instance number for this panel type (0 = first instance).
	pub instance: u16,
}

impl PanelId {
	/// Creates a new panel ID.
	pub const fn new(kind: u16, instance: u16) -> Self {
		Self { kind, instance }
	}

	/// Returns a combined u32 representation for storage.
	pub const fn as_u32(self) -> u32 {
		((self.kind as u32) << 16) | (self.instance as u32)
	}

	/// Creates a panel ID from a u32 representation.
	pub const fn from_u32(val: u32) -> Self {
		Self {
			kind: (val >> 16) as u16,
			instance: (val & 0xFFFF) as u16,
		}
	}
}

impl std::fmt::Display for PanelId {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		write!(f, "Panel({}.{})", self.kind, self.instance)
	}
}

/// Compile-time definition of a panel type.
///
/// Registered via the [`panel!`] macro into the [`PANELS`] distributed slice.
/// At runtime, the panel registry uses these definitions to create and manage
/// panel instances.
pub struct PanelDef {
	/// Unique identifier (e.g., "evildoer-api::terminal").
	pub id: &'static str,
	/// Short name for display (e.g., "terminal", "debug").
	pub name: &'static str,
	/// Human-readable description.
	pub description: &'static str,
	/// Mode name shown in status bar when focused (e.g., "TERMINAL", "DEBUG").
	pub mode_name: &'static str,
	/// Layer index for docking (higher layers overlay lower ones).
	pub layer: usize,
	/// Priority for ordering within a layer.
	pub priority: i16,
	/// Where this panel was defined.
	pub source: RegistrySource,
	/// Whether only one instance of this panel can exist.
	pub singleton: bool,
	/// Whether this panel should be sticky (resist losing focus on mouse hover).
	pub sticky: bool,
	/// Whether this panel captures input instead of the editor.
	pub captures_input: bool,
	/// Whether this panel supports window-mode key routing.
	pub supports_window_mode: bool,
}

/// Registry of all panel definitions.
#[distributed_slice]
pub static PANELS: [PanelDef];

/// Factory function type for creating panel instances.
///
/// Returns a boxed trait object implementing [`SplitBuffer`].
pub type PanelFactory = fn() -> Box<dyn SplitBuffer>;

/// Registration for a panel factory.
///
/// Links a panel type name to its factory function. Registered via the
/// [`panel!`] macro when a `factory:` parameter is provided.
pub struct PanelFactoryDef {
	/// Panel type name (must match a [`PanelDef`] name).
	pub name: &'static str,
	/// Factory function to create new instances.
	pub factory: PanelFactory,
}

/// Registry of all panel factories.
#[distributed_slice]
pub static PANEL_FACTORIES: [PanelFactoryDef];

/// Finds a panel factory by name.
pub fn find_factory(name: &str) -> Option<&'static PanelFactoryDef> {
	PANEL_FACTORIES.iter().find(|f| f.name == name)
}

/// Finds a panel definition by name.
pub fn find_panel(name: &str) -> Option<&'static PanelDef> {
	PANELS.iter().find(|p| p.name == name)
}

/// Finds a panel definition by ID string.
pub fn find_panel_by_id(id: &str) -> Option<&'static PanelDef> {
	PANELS.iter().find(|p| p.id == id)
}

/// Returns the index of a panel in the PANELS slice.
pub fn panel_kind_index(name: &str) -> Option<u16> {
	PANELS.iter().position(|p| p.name == name).map(|i| i as u16)
}

/// Returns all registered panels.
pub fn all_panels() -> impl Iterator<Item = &'static PanelDef> {
	PANELS.iter()
}
