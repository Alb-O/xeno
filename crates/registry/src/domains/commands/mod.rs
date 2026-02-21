//! Command registry.

use std::any::Any;
use std::path::PathBuf;

use xeno_primitives::BoxFutureLocal;

use crate::notifications::Notification;

#[path = "compile/builtins/mod.rs"]
pub mod builtins;
#[path = "contract/def.rs"]
pub mod def;
mod domain;
#[path = "contract/entry.rs"]
pub mod entry;
#[path = "exec/handler.rs"]
pub mod handler;
#[path = "compile/link.rs"]
pub mod link;
#[path = "compile/loader.rs"]
pub mod loader;
#[path = "exec/macros.rs"]
mod macros;
#[path = "contract/spec.rs"]
pub mod spec;

pub use builtins::register_builtins;
pub use def::{CommandDef, CommandHandler, CommandInput};
pub use domain::Commands;
pub use entry::CommandEntry;
pub use handler::{CommandHandlerReg, CommandHandlerStatic};
pub use spec::{CommandPaletteSpec, PaletteArgKind, PaletteArgSpec, PaletteCommitPolicy};

/// Registers compiled commands from the embedded spec.
pub fn register_compiled(db: &mut crate::db::builder::RegistryDbBuilder) {
	let spec = loader::load_commands_spec();
	let handlers = inventory::iter::<handler::CommandHandlerReg>.into_iter().map(|r| r.0);

	let linked = link::link_commands(&spec, handlers);

	for def in linked {
		db.push_domain::<Commands>(def::CommandInput::Linked(def));
	}
}

// Re-export macros
pub use crate::command_handler;
pub use crate::core::{CommandError, RegistryBuilder, RegistryEntry, RegistryMeta, RegistryMetaStatic, RegistryRef, RegistrySource, RuntimeRegistry};

/// Typed reference to a runtime command entry.
pub type CommandRef = RegistryRef<CommandEntry, crate::core::CommandId>;

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
	/// Sets a global option value by config key.
	fn set_option(&mut self, key: &str, value: &str) -> Result<(), CommandError>;
	/// Sets a buffer-local option value by config key.
	fn set_local_option(&mut self, key: &str, value: &str) -> Result<(), CommandError>;
	/// Opens an info popup with the given content and optional file type for syntax highlighting.
	fn open_info_popup(&mut self, content: &str, file_type: Option<&str>);
	/// Closes all open info popups.
	fn close_all_info_popups(&mut self);
	/// Inserts a snippet body at the current cursor/selection.
	///
	/// Returns `true` when insertion succeeds.
	fn insert_snippet_body(&mut self, body: &str) -> bool;

	/// Opens a file and navigates to a specific line and column.
	///
	/// If the file is already open, switches to it. Line and column are 0-indexed.
	fn goto_file(&mut self, path: PathBuf, line: usize, column: usize) -> BoxFutureLocal<'_, Result<(), CommandError>>;
	/// Queues an invocation request for execution on the editor runtime loop.
	fn queue_invocation(&mut self, request: crate::actions::DeferredInvocationRequest);
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
			.ok_or_else(|| CommandError::Other(format!("Missing or invalid user data for command (expected {})", std::any::type_name::<T>())))
	}
}

/// Command flags for optional behavior hints.
pub mod flags {
	/// No special flags.
	pub const NONE: u32 = 0;
}

#[cfg(feature = "minimal")]
pub use crate::db::COMMANDS;

/// Finds a command by name or key.
#[cfg(feature = "minimal")]
pub fn find_command(name: &str) -> Option<RegistryRef<CommandEntry, crate::core::CommandId>> {
	COMMANDS.get(name)
}

/// Returns all registered commands (builtins + runtime), sorted by name.
#[cfg(feature = "minimal")]
pub fn all_commands() -> Vec<RegistryRef<CommandEntry, crate::core::CommandId>> {
	COMMANDS.snapshot_guard().iter_refs().collect()
}
