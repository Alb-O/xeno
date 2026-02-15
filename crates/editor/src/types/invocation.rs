//! Invocation types for unified action/command dispatch.

use xeno_registry::Capability;

/// A user-invoked operation routed through capability gating.
///
/// All entry points (keymap, palette, command queue) convert requests into
/// `Invocation` variants before dispatch via [`Editor::run_invocation`].
///
/// [`Editor::run_invocation`]: crate::Editor::run_invocation
#[derive(Debug, Clone)]
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
		}
	}
}

/// Policy for capability enforcement during invocation dispatch.
///
/// Controls whether violations block execution or just log warnings.
/// Use log-only mode during migration, then flip to enforcing.
#[derive(Debug, Clone, Copy)]
pub struct InvocationPolicy {
	/// Whether to check and enforce required capabilities.
	///
	/// * `true`: Block execution if capabilities are missing (enforcement mode)
	/// * `false`: Log violations but continue (log-only mode)
	pub enforce_caps: bool,

	/// Whether to check and enforce readonly buffer status.
	///
	/// * `true`: Block edits to readonly buffers
	/// * `false`: Log but allow (useful for testing)
	pub enforce_readonly: bool,
}

impl Default for InvocationPolicy {
	fn default() -> Self {
		Self::log_only()
	}
}

impl InvocationPolicy {
	/// Creates a policy that logs violations but doesn't block execution.
	///
	/// Use this during migration to identify capability gaps.
	pub const fn log_only() -> Self {
		Self {
			enforce_caps: false,
			enforce_readonly: false,
		}
	}

	/// Creates a policy that enforces all checks.
	///
	/// Use this once capability gating is fully wired.
	pub const fn enforcing() -> Self {
		Self {
			enforce_caps: true,
			enforce_readonly: true,
		}
	}

	/// Creates a policy that enforces capabilities but not readonly.
	pub const fn enforce_caps_only() -> Self {
		Self {
			enforce_caps: true,
			enforce_readonly: false,
		}
	}
}

/// Result of an invocation attempt.
#[derive(Debug)]
pub enum InvocationResult {
	/// Invocation executed successfully.
	Ok,
	/// Invocation requested application quit.
	Quit,
	/// Invocation requested force quit (no prompts).
	ForceQuit,
	/// The invocation target was not found.
	NotFound(String),
	/// Capability check failed.
	CapabilityDenied(Capability),
	/// Buffer is readonly.
	ReadonlyDenied,
	/// Command execution failed with error.
	CommandError(String),
}

impl InvocationResult {
	/// Returns true if this result indicates a quit request.
	pub fn is_quit(&self) -> bool {
		matches!(self, Self::Quit | Self::ForceQuit)
	}

	/// Returns true if this result indicates successful execution.
	pub fn is_ok(&self) -> bool {
		matches!(self, Self::Ok)
	}
}
