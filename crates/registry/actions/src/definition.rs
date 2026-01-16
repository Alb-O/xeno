//! Action definition and handler types.
//!
//! Actions are registered at compile time via [`linkme`] distributed slices
//! and looked up by keybindings.

use crate::{ActionContext, ActionResult, RegistryMeta};

/// Definition of a registered action.
///
/// Actions are the fundamental unit of editor behavior. They're registered
/// at compile time via [`linkme`] distributed slices and looked up by keybindings.
///
/// # Registration
///
/// Use the `action!` macro to register actions:
///
/// ```ignore
/// action!(move_line_down, {
///     description: "Move line down",
///     bindings: r#"normal \"j\" \"down\""#,
/// }, |ctx| cursor_motion(ctx, "line_down"));
/// ```
pub struct ActionDef {
	/// Common registry metadata.
	pub meta: RegistryMeta,
	/// Short description without key-sequence prefix (for which-key HUD).
	///
	/// When actions share a common prefix (e.g., `g` for "Goto"), this field
	/// contains just the suffix (e.g., "Line start" instead of "Goto line start").
	/// The prefix description is shown on the root key, making the tree read
	/// naturally: `g Goto...` → `h Line start`.
	pub short_desc: &'static str,
	/// The function that executes this action.
	pub handler: ActionHandler,
}

impl ActionDef {
	/// Returns the unique identifier.
	pub fn id(&self) -> &'static str {
		self.meta.id
	}

	/// Returns the human-readable name.
	pub fn name(&self) -> &'static str {
		self.meta.name
	}

	/// Returns alternative names for lookup.
	pub fn aliases(&self) -> &'static [&'static str] {
		self.meta.aliases
	}

	/// Returns the description.
	pub fn description(&self) -> &'static str {
		self.meta.description
	}

	/// Returns the priority.
	pub fn priority(&self) -> i16 {
		self.meta.priority
	}

	/// Returns the source.
	pub fn source(&self) -> crate::RegistrySource {
		self.meta.source
	}

	/// Returns required capabilities.
	pub fn required_caps(&self) -> &'static [crate::Capability] {
		self.meta.required_caps
	}

	/// Returns behavior flags.
	pub fn flags(&self) -> u32 {
		self.meta.flags
	}
}

/// Function signature for action handlers.
///
/// Takes an immutable [`ActionContext`] and returns an [`ActionResult`]
/// describing what the editor should do.
pub type ActionHandler = fn(&ActionContext) -> ActionResult;

crate::impl_registry_entry!(ActionDef);
