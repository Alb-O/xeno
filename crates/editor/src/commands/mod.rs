//! Editor-direct command registry.
//!
//! Xeno has two command systems. Registry commands in `xeno-registry` use
//! the [`CommandEditorOps`] trait abstraction, making them portable across editor
//! implementations. They handle standard operations like `:write`, `:quit`, and `:set`.
//!
//! Editor commands (this module) have direct [`Editor`] access, providing full access
//! to editor internals and the async runtime. Use these for LSP operations, debugging,
//! and editor introspection (`:hover`, `:definition`, `:registry-list`).
//!
//! Prefer registry commands when the operation fits within [`CommandEditorOps`].
//! Use editor commands when you need the async runtime, LSP client, or other
//! editor-specific features.
//!
//! [`CommandEditorOps`]: xeno_registry::commands::CommandEditorOps

mod config;
mod debug;
#[cfg(feature = "lsp")]
mod lsp;
mod nu;

use std::any::Any;
use std::collections::HashMap;
use std::sync::LazyLock;

use xeno_primitives::BoxFutureLocal;
pub use xeno_registry::RegistrySource;
pub use xeno_registry::commands::{CommandError, CommandOutcome};

use crate::Editor;

/// Registry wrapper for editor-direct command definitions.
///
/// Only actively-consumed `inventory` collection in the workspace.
pub struct EditorCommandReg(pub &'static EditorCommandDef);
inventory::collect!(EditorCommandReg);

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
pub type EditorCommandHandler = for<'a> fn(&'a mut EditorCommandContext<'a>) -> BoxFutureLocal<'a, Result<CommandOutcome, CommandError>>;

/// A registered editor-direct command definition.
pub struct EditorCommandDef {
	/// Unique identifier for the command.
	pub id: &'static str,
	/// Primary command name (used in command line).
	pub name: &'static str,
	/// Alternative names for the command.
	pub keys: &'static [&'static str],
	/// Human-readable description for help text.
	pub description: &'static str,
	/// Whether this command mutates buffer text (used for readonly gating).
	pub mutates_buffer: bool,
	/// Async function that executes the command.
	pub handler: EditorCommandHandler,
	/// Extension-specific data passed to handler.
	pub user_data: Option<&'static (dyn Any + Sync)>,
	/// Sort priority (higher = listed first).
	pub priority: i16,
	/// Where this command was registered from.
	pub source: RegistrySource,
}

/// O(1) editor command lookup index by name and keys.
static EDITOR_CMD_INDEX: LazyLock<HashMap<&'static str, &'static EditorCommandDef>> = LazyLock::new(|| {
	let mut map = HashMap::new();
	for reg in inventory::iter::<EditorCommandReg> {
		map.insert(reg.0.name, reg.0);
		for &key in reg.0.keys {
			map.insert(key, reg.0);
		}
	}
	map
});

/// Lazy reference to all editor commands for iteration.
pub static EDITOR_COMMANDS: LazyLock<Vec<&'static EditorCommandDef>> = LazyLock::new(|| {
	let mut commands: Vec<_> = inventory::iter::<EditorCommandReg>().map(|r| r.0).collect();
	commands.sort_by_key(|c| c.name);
	commands
});

/// Finds an editor command by name or key.
pub fn find_editor_command(name: &str) -> Option<&'static EditorCommandDef> {
	EDITOR_CMD_INDEX.get(name).copied()
}

/// Returns an iterator over all registered editor commands, sorted by name.
pub fn all_editor_commands() -> impl Iterator<Item = &'static EditorCommandDef> {
	EDITOR_COMMANDS.iter().copied()
}

/// Registers an editor-direct command via `inventory`.
#[macro_export]
macro_rules! editor_command {
	($name:ident, {
		$(keys: $keys:expr,)?
		description: $desc:expr
		$(, mutates_buffer: $mutates:expr)?
		$(, priority: $priority:expr)?
		$(,)?
	}, handler: $handler:expr) => {
		paste::paste! {
			#[allow(non_upper_case_globals)]
			pub static [<EDITOR_CMD_ $name>]: $crate::commands::EditorCommandDef =
				$crate::commands::EditorCommandDef {
					id: concat!(env!("CARGO_PKG_NAME"), "::", stringify!($name)),
					name: stringify!($name),
					keys: $crate::__editor_cmd_opt_slice!($({$keys})?),
					description: $desc,
					mutates_buffer: $crate::__editor_cmd_opt!($({$mutates})?, false),
					handler: $handler,
					user_data: None,
					priority: $crate::__editor_cmd_opt!($({$priority})?, 0),
					source: $crate::commands::RegistrySource::Crate(env!("CARGO_PKG_NAME")),
				};

			inventory::submit! { $crate::commands::EditorCommandReg(&[<EDITOR_CMD_ $name>]) }
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
