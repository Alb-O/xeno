//! Command registry with auto-collection via `inventory`.
//!
//! This crate provides trait-based commands through the [`CommandEditorOps`] abstraction.
//! Commands registered here are portable and don't depend on specific editor internals.
//! For commands needing direct editor access (LSP, debugging), see `xeno-editor/src/commands/`.
//!
//! # Example
//!
//! ```ignore
//! command!(write, {
//!     aliases: &["w"],
//!     description: "Save the current buffer",
//! }, |ctx| Box::pin(async move {
//!     ctx.editor.save().await?;
//!     Ok(CommandOutcome::Ok)
//! }));
//! ```

use std::any::Any;
use std::future::Future;
use std::path::PathBuf;
use std::pin::Pin;
use std::sync::LazyLock;

use futures::future::LocalBoxFuture;
use xeno_registry_notifications::Notification;

mod impls;
mod macros;

pub use xeno_registry_core::{
	Capability, CommandError, RegistryBuilder, RegistryEntry, RegistryMeta, RegistryMetadata,
	RegistryReg, RegistrySource, RuntimeRegistry, impl_registry_entry,
};

/// Wrapper for [`inventory`] collection of command definitions.
pub struct CommandReg(pub &'static CommandDef);
inventory::collect!(CommandReg);

impl RegistryReg<CommandDef> for CommandReg {
	fn def(&self) -> &'static CommandDef {
		self.0
	}
}

/// Function signature for async command handlers.
pub type CommandHandler = for<'a> fn(
	&'a mut CommandContext<'a>,
) -> LocalBoxFuture<'a, Result<CommandOutcome, CommandError>>;

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
	fn save(&mut self) -> Pin<Box<dyn Future<Output = Result<(), CommandError>> + '_>>;
	/// Saves the current buffer to a new file path.
	fn save_as(
		&mut self,
		path: PathBuf,
	) -> Pin<Box<dyn Future<Output = Result<(), CommandError>> + '_>>;
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
	) -> Pin<Box<dyn Future<Output = Result<(), CommandError>> + '_>>;
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
	///
	/// # Example
	///
	/// ```ignore
	/// use xeno_registry_notifications::keys;
	/// ctx.emit(keys::BUFFER_READONLY);
	/// ctx.emit(keys::file_saved(&path));
	/// ```
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

/// A registered command definition.
pub struct CommandDef {
	/// Common registry metadata.
	pub meta: RegistryMeta,
	/// Async function that executes the command.
	pub handler: CommandHandler,
	/// Extension-specific data passed to handler.
	pub user_data: Option<&'static (dyn Any + Sync)>,
}

impl CommandDef {
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
	pub fn source(&self) -> RegistrySource {
		self.meta.source
	}

	/// Returns required capabilities.
	pub fn required_caps(&self) -> &'static [Capability] {
		self.meta.required_caps
	}

	/// Returns behavior flags.
	pub fn flags(&self) -> u32 {
		self.meta.flags
	}
}

impl_registry_entry!(CommandDef);

/// Command flags for optional behavior hints.
pub mod flags {
	/// No special flags.
	pub const NONE: u32 = 0;
}

/// Indexed collection of all commands with runtime registration support.
pub static COMMANDS: LazyLock<RuntimeRegistry<CommandDef>> = LazyLock::new(|| {
	let builtins = RegistryBuilder::new("commands")
		.extend_inventory::<CommandReg>()
		.sort_by(|a, b| a.meta.name.cmp(b.meta.name))
		.build();
	RuntimeRegistry::new("commands", builtins)
});

/// Registers an extra command definition at runtime.
///
/// Returns `true` if the command was added, `false` if already registered.
pub fn register_command(def: &'static CommandDef) -> bool {
	COMMANDS.register(def)
}

/// Finds a command by name or alias.
pub fn find_command(name: &str) -> Option<&'static CommandDef> {
	COMMANDS.get(name)
}

/// Returns all registered commands (builtins + runtime), sorted by name.
pub fn all_commands() -> impl Iterator<Item = &'static CommandDef> {
	COMMANDS.all().into_iter()
}
