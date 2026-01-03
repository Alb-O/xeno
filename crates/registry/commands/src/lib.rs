//! Command registry
//!
//! Defines command types and compile-time registrations.

use std::any::Any;
use std::future::Future;
use std::path::PathBuf;
use std::pin::Pin;

use futures::future::LocalBoxFuture;
use linkme::distributed_slice;
use thiserror::Error;

mod impls;
/// Internal macro helpers for command registration.
mod macros;

pub use evildoer_registry_core::{RegistryMetadata, RegistrySource, impl_registry_metadata};
pub use evildoer_registry_motions::Capability;

/// Function signature for async command handlers.
pub type CommandHandler = for<'a> fn(
	&'a mut CommandContext<'a>,
) -> LocalBoxFuture<'a, Result<CommandOutcome, CommandError>>;

/// Simplified result type for command operations.
pub type CommandResult = Result<(), CommandError>;

/// Errors that can occur during command execution.
#[derive(Error, Debug, Clone)]
pub enum CommandError {
	/// General command failure with message.
	#[error("{0}")]
	Failed(String),
	/// A required argument was not provided.
	#[error("missing argument: {0}")]
	MissingArgument(&'static str),
	/// An argument was provided but invalid.
	#[error("invalid argument: {0}")]
	InvalidArgument(String),
	/// File I/O operation failed.
	#[error("I/O error: {0}")]
	Io(String),
	/// Command name was not found in registry.
	#[error("command not found: {0}")]
	NotFound(String),
	/// Command requires a capability the context doesn't provide.
	#[error("missing capability: {0:?}")]
	MissingCapability(Capability),
	/// Operation not supported in current context.
	#[error("unsupported operation: {0}")]
	Unsupported(&'static str),
	/// Catch-all for other errors.
	#[error("{0}")]
	Other(String),
}

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
	/// Displays a notification with the given type and message.
	fn notify(&mut self, type_id: &str, msg: &str);
	/// Clears any displayed status message.
	fn clear_message(&mut self);
	/// Returns whether the current buffer has unsaved changes.
	fn is_modified(&self) -> bool;
	/// Saves the current buffer to its file path.
	fn save(&mut self) -> Pin<Box<dyn Future<Output = Result<(), CommandError>> + '_>>;
	/// Saves the current buffer to a new file path.
	fn save_as(
		&mut self,
		path: PathBuf,
	) -> Pin<Box<dyn Future<Output = Result<(), CommandError>> + '_>>;
	/// Changes the active color theme.
	fn set_theme(&mut self, name: &str) -> Result<(), CommandError>;
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
	/// Displays a notification with the given type and message.
	pub fn notify(&mut self, type_id: &str, msg: &str) {
		self.editor.notify(type_id, msg);
	}

	/// Clears any displayed status message.
	pub fn clear_message(&mut self) {
		self.editor.clear_message();
	}

	/// Displays an info notification.
	pub fn info(&mut self, msg: &str) {
		self.notify("info", msg);
	}

	/// Displays a warning notification.
	pub fn warn(&mut self, msg: &str) {
		self.notify("warn", msg);
	}

	/// Displays an error notification.
	pub fn error(&mut self, msg: &str) {
		self.notify("error", msg);
	}

	/// Displays a success notification.
	pub fn success(&mut self, msg: &str) {
		self.notify("success", msg);
	}

	/// Displays a debug notification.
	pub fn debug(&mut self, msg: &str) {
		self.notify("debug", msg);
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
	/// Unique identifier for the command.
	pub id: &'static str,
	/// Primary command name (used in command line).
	pub name: &'static str,
	/// Alternative names for the command.
	pub aliases: &'static [&'static str],
	/// Human-readable description for help text.
	pub description: &'static str,
	/// Async function that executes the command.
	pub handler: CommandHandler,
	/// Extension-specific data passed to handler.
	pub user_data: Option<&'static (dyn Any + Sync)>,
	/// Sort priority (higher = listed first).
	pub priority: i16,
	/// Where this command was registered from.
	pub source: RegistrySource,
	/// Capabilities required to run this command.
	pub required_caps: &'static [Capability],
	/// Optional behavior flags.
	pub flags: u32,
}

/// Command flags for optional behavior hints.
pub mod flags {
	/// No special flags.
	pub const NONE: u32 = 0;
}

/// Distributed slice for compile-time command registration.
#[distributed_slice]
pub static COMMANDS: [CommandDef];

/// Finds a command by name or alias.
pub fn find_command(name: &str) -> Option<&'static CommandDef> {
	COMMANDS
		.iter()
		.find(|c| c.name == name || c.aliases.contains(&name))
}

/// Returns an iterator over all registered commands, sorted by name.
pub fn all_commands() -> impl Iterator<Item = &'static CommandDef> {
	let mut commands: Vec<_> = COMMANDS.iter().collect();
	commands.sort_by_key(|c| c.name);
	commands.into_iter()
}

impl_registry_metadata!(CommandDef);
