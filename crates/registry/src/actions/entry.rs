use super::def::ActionHandler;
use super::keybindings::KeyBindingDef;
use crate::core::{RegistryMeta, Symbol};

/// Symbolized action entry stored in the registry snapshot.
#[derive(Clone, Copy)]
pub struct ActionEntry {
	/// Common registry metadata (symbolized).
	pub meta: RegistryMeta,
	/// Short description (symbolized).
	pub short_desc: Symbol,
	/// The function that executes this action.
	pub handler: ActionHandler,
	/// Keybindings associated with the action.
	pub bindings: &'static [KeyBindingDef],
}

crate::impl_registry_entry!(ActionEntry);
