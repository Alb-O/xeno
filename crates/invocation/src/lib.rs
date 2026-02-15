/// Canonical invocation types for unified action/command dispatch.
///
/// All entry points (keymap, palette, command queue, Nu macros/hooks) convert
/// requests into [`Invocation`] variants before dispatch.

#[cfg(feature = "nu")]
pub mod nu;

/// A user-invoked operation routed through capability gating.
///
/// All entry points (keymap, palette, command queue) convert requests into
/// `Invocation` variants before dispatch via `Editor::run_invocation`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[cfg_attr(feature = "nu", derive(serde::Serialize, serde::Deserialize))]
pub enum Invocation {
	/// Execute a named action from the registry.
	Action {
		/// Action name (looked up via `find_action`).
		name: String,
		/// Repeat count (e.g., `3j` has count=3).
		count: usize,
		/// Whether to extend selection (shift-modified motions).
		extend: bool,
		/// Optional register to use.
		register: Option<char>,
	},
	/// Execute an action with an additional character argument (e.g., `f` motion).
	ActionWithChar {
		/// Action name.
		name: String,
		/// Repeat count.
		count: usize,
		/// Whether to extend selection.
		extend: bool,
		/// Optional register.
		register: Option<char>,
		/// The character argument (e.g., `f` motion takes a char).
		char_arg: char,
	},
	/// Execute a registry command.
	Command {
		/// Command name (looked up via `find_command`).
		name: String,
		/// Command arguments.
		args: Vec<String>,
	},
	/// Execute an editor-direct command.
	EditorCommand {
		/// Command name (looked up via `find_editor_command`).
		name: String,
		/// Command arguments.
		args: Vec<String>,
	},
	/// Execute a Nu macro function from the loaded runtime.
	Nu {
		/// Exported Nu function name.
		name: String,
		/// String arguments passed to the function.
		args: Vec<String>,
	},
}

impl Invocation {
	/// Creates an action invocation with default options.
	pub fn action(name: impl Into<String>) -> Self {
		Self::Action {
			name: name.into(),
			count: 1,
			extend: false,
			register: None,
		}
	}

	/// Creates an action invocation with count.
	pub fn action_with_count(name: impl Into<String>, count: usize) -> Self {
		Self::Action {
			name: name.into(),
			count,
			extend: false,
			register: None,
		}
	}

	/// Creates a command invocation.
	pub fn command(name: impl Into<String>, args: Vec<String>) -> Self {
		Self::Command { name: name.into(), args }
	}

	/// Creates an editor command invocation.
	pub fn editor_command(name: impl Into<String>, args: Vec<String>) -> Self {
		Self::EditorCommand { name: name.into(), args }
	}

	/// Creates a Nu macro invocation.
	pub fn nu(name: impl Into<String>, args: Vec<String>) -> Self {
		Self::Nu { name: name.into(), args }
	}

	/// Short description for tracing/logging.
	pub fn describe(&self) -> String {
		match self {
			Self::Action { name, count, .. } if *count > 1 => format!("action:{name}x{count}"),
			Self::Action { name, .. } => format!("action:{name}"),
			Self::ActionWithChar { name, char_arg, .. } => format!("action:{name}('{char_arg}')"),
			Self::Command { name, args } if args.is_empty() => format!("cmd:{name}"),
			Self::Command { name, args } => format!("cmd:{name} {}", args.join(" ")),
			Self::EditorCommand { name, args } if args.is_empty() => format!("editor_cmd:{name}"),
			Self::EditorCommand { name, args } => format!("editor_cmd:{name} {}", args.join(" ")),
			Self::Nu { name, args } if args.is_empty() => format!("nu:{name}"),
			Self::Nu { name, args } => format!("nu:{name} {}", args.join(" ")),
		}
	}
}
