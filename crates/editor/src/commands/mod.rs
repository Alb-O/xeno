//! Editor-direct command registry.
//!
//! Commands that need direct [`Editor`] access are registered here instead of
//! in `xeno-registry-commands`. This avoids bloating [`CommandEditorOps`] with
//! editor-specific methods.
//!
//! [`CommandEditorOps`]: xeno_registry::commands::CommandEditorOps

#[cfg(feature = "lsp")]
mod lsp;

use std::any::Any;

use futures::future::LocalBoxFuture;
use linkme::distributed_slice;
pub use xeno_registry::RegistrySource;
pub use xeno_registry::commands::{CommandError, CommandOutcome, CommandResult};

use crate::editor::Editor;

/// Context provided to editor-direct command handlers.
pub struct EditorCommandContext<'a> {
	/// Direct access to the editor.
	pub editor: &'a mut Editor,
	/// Command arguments (space-separated tokens after command name).
	pub args: &'a [&'a str],
	/// Numeric prefix count (e.g., `3:w` has count=3).
	pub count: usize,
	/// Register specified with command (e.g., `"a:w`).
	pub register: Option<char>,
	/// Extension-specific data attached to the command.
	pub user_data: Option<&'static (dyn Any + Sync)>,
}

/// Function signature for async editor-direct command handlers.
pub type EditorCommandHandler =
	for<'a> fn(
		&'a mut EditorCommandContext<'a>,
	) -> LocalBoxFuture<'a, Result<CommandOutcome, CommandError>>;

/// A registered editor-direct command definition.
pub struct EditorCommandDef {
	/// Unique identifier for the command.
	pub id: &'static str,
	/// Primary command name (used in command line).
	pub name: &'static str,
	/// Alternative names for the command.
	pub aliases: &'static [&'static str],
	/// Human-readable description for help text.
	pub description: &'static str,
	/// Async function that executes the command.
	pub handler: EditorCommandHandler,
	/// Extension-specific data passed to handler.
	pub user_data: Option<&'static (dyn Any + Sync)>,
	/// Sort priority (higher = listed first).
	pub priority: i16,
	/// Where this command was registered from.
	pub source: RegistrySource,
}

/// Distributed slice for compile-time editor command registration.
#[distributed_slice]
pub static EDITOR_COMMANDS: [EditorCommandDef];

/// Finds an editor command by name or alias.
pub fn find_editor_command(name: &str) -> Option<&'static EditorCommandDef> {
	EDITOR_COMMANDS
		.iter()
		.find(|c| c.name == name || c.aliases.contains(&name))
}

/// Returns an iterator over all registered editor commands, sorted by name.
pub fn all_editor_commands() -> impl Iterator<Item = &'static EditorCommandDef> {
	let mut commands: Vec<_> = EDITOR_COMMANDS.iter().collect();
	commands.sort_by_key(|c| c.name);
	commands.into_iter()
}

/// Registers an editor-direct command in [`EDITOR_COMMANDS`].
#[macro_export]
macro_rules! editor_command {
	($name:ident, {
		$(aliases: $aliases:expr,)?
		description: $desc:expr
		$(, priority: $priority:expr)?
		$(,)?
	}, handler: $handler:expr) => {
		paste::paste! {
			#[allow(non_upper_case_globals)]
			#[::linkme::distributed_slice($crate::commands::EDITOR_COMMANDS)]
			static [<EDITOR_CMD_ $name>]: $crate::commands::EditorCommandDef =
				$crate::commands::EditorCommandDef {
					id: concat!(env!("CARGO_PKG_NAME"), "::", stringify!($name)),
					name: stringify!($name),
					aliases: $crate::__editor_cmd_opt_slice!($({$aliases})?),
					description: $desc,
					handler: $handler,
					user_data: None,
					priority: $crate::__editor_cmd_opt!($({$priority})?, 0),
					source: $crate::commands::RegistrySource::Crate(env!("CARGO_PKG_NAME")),
				};
		}
	};
}

#[macro_export]
#[doc(hidden)]
macro_rules! __editor_cmd_opt_slice {
	() => {
		&[]
	};
	({$val:expr}) => {
		$val
	};
}

#[macro_export]
#[doc(hidden)]
macro_rules! __editor_cmd_opt {
	(, $default:expr) => {
		$default
	};
	({$val:expr}, $default:expr) => {
		$val
	};
}
