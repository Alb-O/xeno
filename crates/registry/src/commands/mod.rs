//! Command registry.

use std::any::Any;
use std::path::PathBuf;

use xeno_primitives::BoxFutureLocal;

use crate::core::index::{BuildEntry, RegistryMetaRef};
use crate::notifications::Notification;
use crate::{CapabilitySet, FrozenInterner, Symbol, SymbolList};

pub mod builtins;
mod macros;

pub use builtins::register_builtins;

use crate::error::RegistryError;

pub fn register_plugin(
	db: &mut crate::db::builder::RegistryDbBuilder,
) -> Result<(), RegistryError> {
	register_builtins(db);
	Ok(())
}

// Re-export macros
pub use crate::command;
pub use crate::core::{
	Capability, CommandError, RegistryBuilder, RegistryEntry, RegistryMeta, RegistryMetaStatic,
	RegistryMetadata, RegistryRef, RegistrySource, RuntimeRegistry,
};

/// Function signature for async command handlers.
pub type CommandHandler =
	for<'a> fn(
		&'a mut CommandContext<'a>,
	) -> xeno_primitives::BoxFutureLocal<'a, Result<CommandOutcome, CommandError>>;

/// Simplified result type for command operations.
pub type CommandResult = Result<(), CommandError>;

/// Outcome of a successfully executed command.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommandOutcome {
	/// Command completed normally.
	Ok,
	/// Command requests editor quit (may prompt for unsaved changes).
	Quit,
	/// Command requests immediate quit (no prompts).
	ForceQuit,
}

/// Editor operations available to commands.
pub trait CommandEditorOps {
	/// Emits a type-safe notification.
	fn emit(&mut self, notification: Notification);
	/// Clears all visible notifications.
	fn clear_notifications(&mut self);
	/// Returns whether the current buffer has unsaved changes.
	fn is_modified(&self) -> bool;
	/// Returns whether the current buffer is read-only.
	fn is_readonly(&self) -> bool;
	/// Sets the read-only flag for the current buffer.
	fn set_readonly(&mut self, readonly: bool);
	/// Saves the current buffer to its file path.
	fn save(&mut self) -> BoxFutureLocal<'_, Result<(), CommandError>>;
	/// Saves the current buffer to a new file path.
	fn save_as(&mut self, path: PathBuf) -> BoxFutureLocal<'_, Result<(), CommandError>>;
	/// Changes the active color theme.
	fn set_theme(&mut self, name: &str) -> Result<(), CommandError>;
	/// Sets a global option value by KDL key.
	fn set_option(&mut self, kdl_key: &str, value: &str) -> Result<(), CommandError>;
	/// Sets a buffer-local option value by KDL key.
	fn set_local_option(&mut self, kdl_key: &str, value: &str) -> Result<(), CommandError>;
	/// Opens an info popup with the given content and optional file type for syntax highlighting.
	fn open_info_popup(&mut self, content: &str, file_type: Option<&str>);
	/// Closes all open info popups.
	fn close_all_info_popups(&mut self);

	/// Opens a file and navigates to a specific line and column.
	///
	/// If the file is already open, switches to it. Line and column are 0-indexed.
	fn goto_file(
		&mut self,
		path: PathBuf,
		line: usize,
		column: usize,
	) -> BoxFutureLocal<'_, Result<(), CommandError>>;
}

/// Context provided to command handlers.
pub struct CommandContext<'a> {
	/// Editor operations interface.
	pub editor: &'a mut dyn CommandEditorOps,
	/// Command arguments (space-separated tokens after command name).
	pub args: &'a [&'a str],
	/// Numeric prefix count (e.g., `3:w` has count=3).
	pub count: usize,
	/// Register specified with command (e.g., `"a:w`).
	pub register: Option<char>,
	/// Extension-specific data attached to the command.
	pub user_data: Option<&'static (dyn Any + Sync)>,
}

impl<'a> CommandContext<'a> {
	/// Emits a type-safe notification.
	pub fn emit(&mut self, notification: impl Into<Notification>) {
		self.editor.emit(notification.into());
	}

	/// Clears all visible notifications.
	pub fn clear_notifications(&mut self) {
		self.editor.clear_notifications();
	}

	/// Returns whether the current buffer is read-only.
	pub fn is_readonly(&self) -> bool {
		self.editor.is_readonly()
	}

	/// Sets the read-only flag for the current buffer.
	pub fn set_readonly(&mut self, readonly: bool) {
		self.editor.set_readonly(readonly);
	}

	/// Extracts and downcasts user data to the expected type.
	pub fn require_user_data<T: Any + Sync>(&self) -> Result<&'static T, CommandError> {
		self.user_data
			.and_then(|d| {
				let any: &dyn Any = d;
				any.downcast_ref::<T>()
			})
			.ok_or_else(|| {
				CommandError::Other(format!(
					"Missing or invalid user data for command (expected {})",
					std::any::type_name::<T>()
				))
			})
	}
}

/// A registered command definition (static input for builder).
#[derive(Clone)]
pub struct CommandDef {
	/// Common registry metadata (static).
	pub meta: RegistryMetaStatic,
	/// Async function that executes the command.
	pub handler: CommandHandler,
	/// Extension-specific data passed to handler.
	pub user_data: Option<&'static (dyn Any + Sync)>,
}

impl crate::core::RegistryEntry for CommandDef {
	fn meta(&self) -> &RegistryMeta {
		panic!("Called meta() on static CommandDef")
	}
}

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

impl BuildEntry<CommandEntry> for CommandDef {
	fn meta_ref(&self) -> RegistryMetaRef<'_> {
		RegistryMetaRef {
			id: self.meta.id,
			name: self.meta.name,
			aliases: self.meta.aliases,
			description: self.meta.description,
			priority: self.meta.priority,
			source: self.meta.source,
			required_caps: self.meta.required_caps,
			flags: self.meta.flags,
		}
	}

	fn short_desc_str(&self) -> &str {
		self.meta.name // Commands don't have short_desc currently
	}

	fn collect_strings<'a>(&'a self, sink: &mut Vec<&'a str>) {
		let meta = self.meta_ref();
		sink.push(meta.id);
		sink.push(meta.name);
		sink.push(meta.description);
		for &alias in meta.aliases {
			sink.push(alias);
		}
	}

	fn build(&self, interner: &FrozenInterner, alias_pool: &mut Vec<Symbol>) -> CommandEntry {
		let meta_ref = self.meta_ref();
		let start = alias_pool.len() as u32;
		for &alias in meta_ref.aliases {
			alias_pool.push(interner.get(alias).expect("missing interned alias"));
		}
		let len = (alias_pool.len() as u32 - start) as u16;

		let meta = RegistryMeta {
			id: interner.get(meta_ref.id).expect("missing interned id"),
			name: interner.get(meta_ref.name).expect("missing interned name"),
			description: interner
				.get(meta_ref.description)
				.expect("missing interned description"),
			aliases: SymbolList { start, len },
			priority: meta_ref.priority,
			source: meta_ref.source,
			required_caps: CapabilitySet::from_iter(meta_ref.required_caps.iter().cloned()),
			flags: meta_ref.flags,
		};

		CommandEntry {
			meta,
			handler: self.handler,
			user_data: self.user_data,
		}
	}
}

/// Command flags for optional behavior hints.
pub mod flags {
	/// No special flags.
	pub const NONE: u32 = 0;
}

#[cfg(feature = "db")]
pub use crate::db::COMMANDS;

/// Finds a command by name or alias.
#[cfg(feature = "db")]
pub fn find_command(name: &str) -> Option<RegistryRef<CommandEntry, crate::core::CommandId>> {
	COMMANDS.get(name)
}

/// Returns all registered commands (builtins + runtime), sorted by name.
#[cfg(feature = "db")]
pub fn all_commands() -> Vec<RegistryRef<CommandEntry, crate::core::CommandId>> {
	COMMANDS.all()
}
