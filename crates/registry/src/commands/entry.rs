use std::any::Any;

use super::def::CommandHandler;
use crate::core::RegistryMeta;

/// Symbolized command entry stored in the registry snapshot.
pub struct CommandEntry {
	/// Common registry metadata (symbolized).
	pub meta: RegistryMeta,
	/// Async function that executes the command.
	pub handler: CommandHandler,
	/// Extension-specific data passed to handler.
	pub user_data: Option<&'static (dyn Any + Sync)>,
}

crate::impl_registry_entry!(CommandEntry);
