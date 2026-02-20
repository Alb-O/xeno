use std::any::Any;

use super::def::CommandHandler;
use super::spec::CommandPaletteSpec;
use crate::core::RegistryMeta;

/// Symbolized command entry stored in the registry snapshot.
pub struct CommandEntry {
	/// Common registry metadata (symbolized).
	pub meta: RegistryMeta,
	/// Palette semantics used by command-line completion and commit policy.
	pub palette: CommandPaletteSpec,
	/// Async function that executes the command.
	pub handler: CommandHandler,
	/// Extension-specific data passed to handler.
	pub user_data: Option<&'static (dyn Any + Sync)>,
}

impl CommandEntry {
	/// Returns command-line palette semantics for this command.
	pub fn palette(&self) -> &CommandPaletteSpec {
		&self.palette
	}
}

crate::impl_registry_entry!(CommandEntry);
