//! Registry-first organization extensions.
//!
//! This crate aggregates all registry sub-crates. Depend on this crate to get
//! access to all registries, rather than depending on individual registry crates.
//!
//! # Sub-crates
//!
//! - [`gutter`] - Gutter columns (line numbers, signs, diagnostics)
//! - [`motions`] - Cursor movement primitives
//! - [`options`] - Configuration options
//! - [`notifications`] - Notification types
//! - [`commands`] - Ex-mode command definitions
//! - [`actions`] - Action definitions and handlers
//! - [`hooks`] - Event lifecycle observers
//! - [`statusline`] - Statusline segment definitions
//! - [`textobj`] - Text object selection (inner/around)
//!
//! # Infrastructure
//!
//! - [`index`] - Registry indexing and lookup
//! - [`keymap_registry`] - Keybinding resolution
//!
//! # Adding a New Registry
//!
//! 1. Create `crates/registry/{name}/` with Cargo.toml and src/
//! 2. Add to root `Cargo.toml` members and workspace.dependencies
//! 3. Add dependency and re-export here

/// Explicit registry builder for plugin-style registration.
pub mod builder;
/// Built-in registry registrations.
pub mod builtins;
/// Registry indexing and lookup for editor extensions.
pub mod index;
/// Unified keymap registry using trie-based matching.
pub mod keymap_registry;
/// Plugin registration trait for explicit wiring.
pub mod plugin;

// Re-export commonly used items at the crate root for convenience
// Data-oriented edit operations
pub use actions::editor_ctx::{
	CommandQueueAccess, CursorAccess, EditAccess, EditorCapabilities, EditorContext, EditorOps,
	FileOpsAccess, FocusOps, HandleOutcome, JumpAccess, MacroAccess, ModeAccess, MotionAccess,
	NotificationAccess, OptionAccess, PaletteAccess, ResultHandler, SearchAccess, SelectionAccess,
	SplitOps, TextAccess, ThemeAccess, UndoAccess, ViewportAccess,
};
pub use actions::{
	ActionArgs, ActionContext, ActionDef, ActionEffects, ActionHandler, ActionResult, AppEffect,
	BindingMode, EditEffect, Effect, KEY_PREFIXES, KEYBINDINGS, KeyBindingDef, KeyPrefixDef, Mode,
	ObjectSelectionKind, PendingAction, PendingKind, RESULT_EFFECTS_HANDLERS,
	RESULT_EXTENSION_HANDLERS, ScreenPosition, ScrollAmount, UiEffect, ViewEffect, action,
	dispatch_result, edit_op, find_prefix, key_prefix, result_extension_handler, result_handler,
};
// Re-export direction types (via actions which re-exports from xeno-base)
pub use actions::{Axis, SeqDirection, SpatialDirection};
pub use builder::{RegistryBuilder, RegistryError};
pub use commands::{
	COMMANDS, CommandContext, CommandDef, CommandEditorOps, CommandError, CommandHandler,
	CommandOutcome, CommandResult, all_commands, command, find_command,
};
pub use gutter::{
	GUTTERS, GutterAnnotations, GutterCell, GutterDef, GutterLineContext, GutterStyle, GutterWidth,
	GutterWidthContext, all as all_gutters, column_width, column_widths, enabled_gutters,
	find as find_gutter, gutter, total_width as gutter_total_width,
};
pub use hooks::{
	Bool, BoxFuture, HOOKS, HookAction, HookContext, HookDef, HookEvent, HookEventData,
	HookHandler, HookMutability, HookPriority, HookResult, HookScheduler, MutableHookContext,
	OptionViewId, OwnedHookContext, SplitDirection, Str, ViewId, WindowId, WindowKind, all_hooks,
	async_hook, emit, emit_mutable, emit_sync, emit_sync_with, find_hooks, hook,
};
// Re-export index lookup functions (excluding duplicates from commands)
pub use index::{
	all_actions, all_motions, all_text_objects, find_action, find_action_by_id, find_motion,
	find_text_object_by_trigger, resolve_action_id, resolve_action_key,
};
// Re-export keymap registry
pub use keymap_registry::{BindingEntry, KeymapRegistry, LookupResult, get_keymap_registry};
pub use motions::{Capability, MOTIONS, MotionDef, MotionHandler, flags, motion, movement};
pub use notifications::{
	AutoDismiss, IntoNotification, Level, NOTIFICATIONS, Notification, NotificationDef,
	NotificationKey, keys as notification_keys,
};
pub use plugin::XenoPlugin;
pub use statusline::{
	RenderedSegment, STATUSLINE_SEGMENTS, SegmentPosition, SegmentStyle, StatuslineContext,
	StatuslineSegmentDef, all_segments, find_segment, render_position, segment,
	segments_for_position,
};
pub use textobj::{
	TEXT_OBJECTS, TextObjectDef, TextObjectHandler, bracket_pair_object, symmetric_text_object,
	text_object,
};
// Re-export shared types from registry core (canonical source)
pub use xeno_registry_core::{ActionId, RegistryMetadata, RegistrySource};
pub use {
	xeno_registry_actions as actions, xeno_registry_commands as commands,
	xeno_registry_gutter as gutter, xeno_registry_hooks as hooks, xeno_registry_motions as motions,
	xeno_registry_notifications as notifications, xeno_registry_options as options,
	xeno_registry_statusline as statusline, xeno_registry_textobj as textobj,
	xeno_registry_themes as themes,
};

#[cfg(test)]
mod tests {
	use std::collections::{HashMap, HashSet};

	use super::*;

	fn assert_unique_ids<T: RegistryMetadata + 'static>(
		label: &str,
		items: impl IntoIterator<Item = &'static T>,
	) {
		let mut seen = HashSet::new();
		let mut duplicates = Vec::new();

		for item in items {
			let id = item.id();
			if !seen.insert(id) {
				duplicates.push(id);
			}
		}

		assert!(
			duplicates.is_empty(),
			"{label} duplicate ids: {}",
			duplicates.join(", ")
		);
	}

	fn is_namespaced_id(id: &str) -> bool {
		if id.is_empty() {
			return false;
		}

		let bytes = id.as_bytes();
		let mut has_separator = false;
		let mut last_was_separator = false;
		let mut i = 0;

		while i < bytes.len() {
			match bytes[i] {
				b'a'..=b'z' | b'A'..=b'Z' | b'0'..=b'9' | b'-' | b'_' => {
					last_was_separator = false;
					i += 1;
				}
				b'.' => {
					has_separator = true;
					if last_was_separator {
						return false;
					}
					last_was_separator = true;
					i += 1;
				}
				b':' => {
					if i + 1 >= bytes.len() || bytes[i + 1] != b':' {
						return false;
					}
					has_separator = true;
					if last_was_separator {
						return false;
					}
					last_was_separator = true;
					i += 2;
				}
				_ => return false,
			}
		}

		has_separator && !last_was_separator
	}

	fn assert_namespaced_ids<T: RegistryMetadata + 'static>(
		label: &str,
		items: impl IntoIterator<Item = &'static T>,
	) {
		let mut invalid = Vec::new();

		for item in items {
			let id = item.id();
			if !is_namespaced_id(id) {
				invalid.push(id);
			}
		}

		assert!(
			invalid.is_empty(),
			"{label} ids should be namespaced: {}",
			invalid.join(", ")
		);
	}

	/// Sanity check to catch registry list regressions.
	#[test]
	fn registry_sanity_check() {
		let action_count = all_actions().count();
		assert!(
			action_count >= 50,
			"Expected at least 50 actions registered, got {}",
			action_count
		);
		assert!(
			MOTIONS.len() >= 20,
			"Expected at least 20 motions registered, got {}",
			MOTIONS.len()
		);
		assert!(
			COMMANDS.len() >= 10,
			"Expected at least 10 commands registered, got {}",
			COMMANDS.len()
		);
		assert!(
			GUTTERS.len() >= 2,
			"Expected at least 2 gutters registered, got {}",
			GUTTERS.len()
		);
		assert!(
			TEXT_OBJECTS.len() >= 10,
			"Expected at least 10 text objects registered, got {}",
			TEXT_OBJECTS.len()
		);
	}

	#[test]
	fn registry_id_uniqueness_by_kind() {
		assert_unique_ids("actions", all_actions());
		assert_unique_ids("commands", COMMANDS.iter().copied());
		assert_unique_ids("motions", MOTIONS.iter());
		assert_unique_ids("gutters", GUTTERS.iter());
		assert_unique_ids("hooks", HOOKS.iter().copied());
		assert_unique_ids("statusline", STATUSLINE_SEGMENTS.iter());
		assert_unique_ids("text_objects", TEXT_OBJECTS.iter());
		assert_unique_ids("options", options::OPTIONS.iter());
		assert_unique_ids("themes", themes::THEMES.iter());
	}

	#[test]
	fn registry_id_namespacing() {
		assert_namespaced_ids("actions", all_actions());
		assert_namespaced_ids("commands", COMMANDS.iter().copied());
		assert_namespaced_ids("motions", MOTIONS.iter());
		assert_namespaced_ids("gutters", GUTTERS.iter());
		assert_namespaced_ids("hooks", HOOKS.iter().copied());
		assert_namespaced_ids("text_objects", TEXT_OBJECTS.iter());
	}

	#[test]
	fn action_ids_resolve() {
		for action in all_actions() {
			let action_id = resolve_action_id(action.name()).unwrap_or_else(|| {
				panic!("action name missing from index: {}", action.name());
			});
			assert!(
				find_action_by_id(action_id).is_some(),
				"action id missing from index: {}",
				action.name()
			);
		}
	}

	#[test]
	fn command_names_and_aliases_resolve() {
		for &command in COMMANDS.iter() {
			assert!(
				find_command(command.meta.name).is_some(),
				"command name missing from index: {}",
				command.meta.name
			);

			for &alias in command.meta.aliases {
				assert!(
					find_command(alias).is_some(),
					"command alias missing from index: {}",
					alias
				);
			}
		}
	}

	#[test]
	fn command_aliases_unique() {
		let mut names: HashMap<&'static str, &'static CommandDef> = HashMap::new();
		for &command in COMMANDS.iter() {
			names.insert(command.meta.name, command);
		}

		let mut aliases: HashMap<&'static str, &'static CommandDef> = HashMap::new();
		let mut duplicates = HashSet::new();

		for &command in COMMANDS.iter() {
			for &alias in command.meta.aliases {
				if let Some(existing) = names.get(alias) {
					if !std::ptr::eq(*existing, command) {
						duplicates.insert(alias);
						continue;
					}
				}

				if let Some(existing) = aliases.get(alias) {
					if !std::ptr::eq(*existing, command) {
						duplicates.insert(alias);
					}
					continue;
				}

				aliases.insert(alias, command);
			}
		}

		let mut duplicates: Vec<_> = duplicates.into_iter().collect();
		duplicates.sort();

		assert!(
			duplicates.is_empty(),
			"duplicate command aliases: {}",
			duplicates.join(", ")
		);
	}

	#[test]
	fn keybindings_resolve_actions() {
		for binding in KEYBINDINGS.iter() {
			assert!(
				resolve_action_id(binding.action).is_some(),
				"keybinding refers to unknown action: {}",
				binding.action
			);
		}
	}
}
