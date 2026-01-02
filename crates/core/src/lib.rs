//! Core infrastructure
//!
//! This crate provides the glue layer between `evildoer-registry` types and the
//! editor's infrastructure, including:
//!
//! - [`ActionId`] for action dispatch
//! - [`KeymapRegistry`] for trie-based keybinding lookup
//! - Movement functions for cursor/selection manipulation
//! - Notification system infrastructure
//! - Result handlers for action dispatch
//!
//! # Import Guidelines
//!
//! - Base types (`Mode`, `Range`, `Selection`, `Key`, etc.): use `evildoer_base`
//! - Registry types (actions, commands, hooks, panels, etc.): use `evildoer_registry`
//! - Core types (`ActionId`, keymap, movement, completion): use this crate

pub mod completion;
pub mod editor_ctx;
pub mod index;
pub mod keymap_registry;
pub mod macros;
#[cfg(feature = "host")]
pub mod movement;
pub mod notifications;
pub mod terminal_config;

/// Theme completion source.
pub mod theme {
	use evildoer_registry::themes::{THEMES, ThemeVariant, runtime_themes};

	use super::completion::{
		CompletionContext, CompletionItem, CompletionKind, CompletionResult, CompletionSource,
		PROMPT_COMMAND,
	};

	/// Completion source for theme names.
	pub struct ThemeSource;

	impl CompletionSource for ThemeSource {
		fn complete(&self, ctx: &CompletionContext) -> CompletionResult {
			if ctx.prompt != PROMPT_COMMAND {
				return CompletionResult::empty();
			}

			let parts: Vec<&str> = ctx.input.split_whitespace().collect();
			if !matches!(parts.first(), Some(&"theme") | Some(&"colorscheme")) {
				return CompletionResult::empty();
			}

			let prefix = parts.get(1).copied().unwrap_or("");
			if parts.len() == 1 && !ctx.input.ends_with(' ') {
				return CompletionResult::empty();
			}

			let cmd_name = parts.first().unwrap();
			let arg_start = cmd_name.len() + 1;

			let mut items: Vec<_> = runtime_themes()
				.iter()
				.copied()
				.chain(THEMES.iter())
				.filter(|t| {
					t.name.starts_with(prefix) || t.aliases.iter().any(|a| a.starts_with(prefix))
				})
				.map(|t| CompletionItem {
					label: t.name.to_string(),
					insert_text: t.name.to_string(),
					detail: Some(format!(
						"{} theme",
						match t.variant {
							ThemeVariant::Dark => "dark",
							ThemeVariant::Light => "light",
						}
					)),
					filter_text: None,
					kind: CompletionKind::Theme,
				})
				.collect();

			items.dedup_by(|a, b| a.label == b.label);
			CompletionResult::new(arg_start, items)
		}
	}
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ActionId(pub u32);

impl ActionId {
	/// Represents an invalid action ID.
	pub const INVALID: ActionId = ActionId(u32::MAX);

	/// Returns true if this action ID is valid.
	#[inline]
	pub fn is_valid(self) -> bool {
		self != Self::INVALID
	}

	/// Returns the underlying u32 value.
	#[inline]
	pub fn as_u32(self) -> u32 {
		self.0
	}
}

impl std::fmt::Display for ActionId {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		if *self == Self::INVALID {
			write!(f, "ActionId(INVALID)")
		} else {
			write!(f, "ActionId({})", self.0)
		}
	}
}

// Core's own types
pub use completion::{CompletionContext, CompletionItem, CompletionKind, CompletionSource};
pub use editor_ctx::{EditorCapabilities, EditorContext, EditorOps, HandleOutcome};
// Convenience namespace for registry access
pub use evildoer_registry as registry;
pub use index::{
	all_actions, all_commands, all_motions, all_text_objects, find_action, find_action_by_id,
	find_command, find_motion, find_text_object_by_trigger, resolve_action_id,
};
pub use keymap_registry::{BindingEntry, KeymapRegistry, LookupResult, get_keymap_registry};
#[cfg(feature = "host")]
pub use movement::WordType;
pub use notifications::{
	NotifyDEBUGExt, NotifyERRORExt, NotifyINFOExt, NotifySUCCESSExt, NotifyWARNExt,
};
pub use terminal_config::{TerminalConfig, TerminalSequence};
pub use theme::ThemeSource;
