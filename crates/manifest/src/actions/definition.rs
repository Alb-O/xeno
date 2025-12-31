//! Action definition and handler types.
//!
//! Actions are registered at compile time via [`linkme`] distributed slices
//! and looked up by keybindings.

use super::{ActionContext, ActionResult};
use crate::{Capability, RegistryMetadata, RegistrySource};

/// Definition of a registered action.
///
/// Actions are the fundamental unit of editor behavior. They're registered
/// at compile time via [`linkme`] distributed slices and looked up by keybindings.
///
/// # Registration
///
/// Use the `action!` macro in `evildoer-stdlib` to register actions:
///
/// ```ignore
/// action!(move_line_down, {
///     description: "Move line down",
///     bindings: r#"normal "j" "down""#,
/// }, |ctx| cursor_motion(ctx, "line_down"));
/// ```
pub struct ActionDef {
	/// Unique identifier (e.g., "evildoer-stdlib::move_line_down").
	pub id: &'static str,
	/// Human-readable name for UI display.
	pub name: &'static str,
	/// Alternative names for command lookup.
	pub aliases: &'static [&'static str],
	/// Description for help text.
	pub description: &'static str,
	/// The function that executes this action.
	pub handler: ActionHandler,
	/// Priority for conflict resolution (higher wins).
	pub priority: i16,
	/// Where this action was defined.
	pub source: RegistrySource,
	/// Capabilities required to execute this action.
	pub required_caps: &'static [Capability],
	/// Bitflags for additional behavior hints.
	pub flags: u32,
}

/// Function signature for action handlers.
///
/// Takes an immutable [`ActionContext`] and returns an [`ActionResult`]
/// describing what the editor should do.
pub type ActionHandler = fn(&ActionContext) -> ActionResult;

impl RegistryMetadata for ActionDef {
	fn id(&self) -> &'static str {
		self.id
	}
	fn name(&self) -> &'static str {
		self.name
	}
	fn priority(&self) -> i16 {
		self.priority
	}
	fn source(&self) -> RegistrySource {
		self.source
	}
}
