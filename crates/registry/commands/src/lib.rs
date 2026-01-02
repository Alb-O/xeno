//! Command registry for Evildoer editor.
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
mod macros;

pub use evildoer_registry_motions::{Capability, RegistrySource};

pub type CommandHandler = for<'a> fn(
	&'a mut CommandContext<'a>,
) -> LocalBoxFuture<'a, Result<CommandOutcome, CommandError>>;

pub type CommandResult = Result<(), CommandError>;

#[derive(Error, Debug, Clone)]
pub enum CommandError {
	#[error("{0}")]
	Failed(String),
	#[error("missing argument: {0}")]
	MissingArgument(&'static str),
	#[error("invalid argument: {0}")]
	InvalidArgument(String),
	#[error("I/O error: {0}")]
	Io(String),
	#[error("command not found: {0}")]
	NotFound(String),
	#[error("missing capability: {0:?}")]
	MissingCapability(Capability),
	#[error("unsupported operation: {0}")]
	Unsupported(&'static str),
	#[error("{0}")]
	Other(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommandOutcome {
	Ok,
	Quit,
	ForceQuit,
}

pub trait CommandEditorOps {
	fn notify(&mut self, type_id: &str, msg: &str);
	fn clear_message(&mut self);
	fn is_modified(&self) -> bool;
	fn save(&mut self) -> Pin<Box<dyn Future<Output = Result<(), CommandError>> + '_>>;
	fn save_as(
		&mut self,
		path: PathBuf,
	) -> Pin<Box<dyn Future<Output = Result<(), CommandError>> + '_>>;
	fn set_theme(&mut self, name: &str) -> Result<(), CommandError>;
}

pub struct CommandContext<'a> {
	pub editor: &'a mut dyn CommandEditorOps,
	pub args: &'a [&'a str],
	pub count: usize,
	pub register: Option<char>,
	pub user_data: Option<&'static (dyn Any + Sync)>,
}

impl<'a> CommandContext<'a> {
	pub fn notify(&mut self, type_id: &str, msg: &str) {
		self.editor.notify(type_id, msg);
	}

	pub fn clear_message(&mut self) {
		self.editor.clear_message();
	}

	pub fn info(&mut self, msg: &str) {
		self.notify("info", msg);
	}

	pub fn warn(&mut self, msg: &str) {
		self.notify("warn", msg);
	}

	pub fn error(&mut self, msg: &str) {
		self.notify("error", msg);
	}

	pub fn success(&mut self, msg: &str) {
		self.notify("success", msg);
	}

	pub fn debug(&mut self, msg: &str) {
		self.notify("debug", msg);
	}

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

pub struct CommandDef {
	pub id: &'static str,
	pub name: &'static str,
	pub aliases: &'static [&'static str],
	pub description: &'static str,
	pub handler: CommandHandler,
	pub user_data: Option<&'static (dyn Any + Sync)>,
	pub priority: i16,
	pub source: RegistrySource,
	pub required_caps: &'static [Capability],
	pub flags: u32,
}

/// Command flags for optional behavior hints.
pub mod flags {
	/// No special flags.
	pub const NONE: u32 = 0;
}

#[distributed_slice]
pub static COMMANDS: [CommandDef];

pub fn find_command(name: &str) -> Option<&'static CommandDef> {
	COMMANDS
		.iter()
		.find(|c| c.name == name || c.aliases.contains(&name))
}

pub fn all_commands() -> impl Iterator<Item = &'static CommandDef> {
	let mut commands: Vec<_> = COMMANDS.iter().collect();
	commands.sort_by_key(|c| c.name);
	commands.into_iter()
}
