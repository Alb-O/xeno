//! Registry infrastructure using compile-time distributed slices.
//!
//! This crate contains type definitions, traits, and distributed slices for the
//! evildoer editor's extension system. It does NOT contain implementations - those
//! live in evildoer-stdlib.
//!
//! # Core Types
//!
//! - [`RegistrySource`] - Where a registry item was defined
//! - [`Capability`] - Editor capabilities required by registry items
//! - [`ActionId`] - Unique identifier for actions
//! - [`CommandError`] / [`CommandResult`] - Command execution errors
//!
//! # Modules
//!
//! - [`actions`] - Action definitions and handlers
//! - [`commands`] - Ex-mode command definitions
//! - [`hooks`] - Event lifecycle observers
//! - [`keybindings`] - Key to action mappings
//! - [`notifications`] - UI notification system
//!
//! # Distributed Slices
//!
//! Compile-time registries using [`linkme`]:
//! - [`ACTIONS`] - All registered actions
//! - [`COMMANDS`] - All registered commands
//! - [`MOTIONS`] - All registered motions
//! - [`TEXT_OBJECTS`] - All registered text objects
//!
//! Note: Languages are loaded at runtime from `languages.kdl` via `evildoer-language`.

pub use evildoer_base::range::CharIdx;
pub use evildoer_base::{Range, Selection};
use linkme::distributed_slice;

pub mod motions;

pub use evildoer_registry::{
	bracket_pair_object, motion, option, statusline_segment, symmetric_text_object, text_object,
	Capability, RegistrySource,
};
pub mod registry;
pub mod syntax;
pub mod text_objects;

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

/// Common semantic color identifiers.
pub const SEMANTIC_INFO: &str = "info";
pub const SEMANTIC_WARNING: &str = "warning";
pub const SEMANTIC_ERROR: &str = "error";
pub const SEMANTIC_SUCCESS: &str = "success";
pub const SEMANTIC_DIM: &str = "dim";
pub const SEMANTIC_NORMAL: &str = "normal";

pub trait RegistryMetadata {
	fn id(&self) -> &'static str;
	fn name(&self) -> &'static str;
	fn priority(&self) -> i16;
	fn source(&self) -> RegistrySource;
}

pub use evildoer_registry::commands::{CommandError, CommandOutcome, CommandResult};

pub mod actions;
pub mod commands;
pub mod completion;
pub mod editor_ctx;
pub mod hooks;
pub mod index;
pub mod keybindings;
pub mod keymap_registry;
pub mod macros;
pub mod mode;
pub mod notifications;
pub mod panels;
pub mod split_buffer;
pub mod statusline;
pub mod terminal_config;
pub mod theme;

#[distributed_slice]
pub static ACTIONS: [actions::ActionDef];

pub use evildoer_registry::commands::COMMANDS;

// Re-export motion and text object types at crate root for backward compatibility
pub use actions::{
	ActionArgs, ActionContext, ActionDef, ActionHandler, ActionMode, ActionResult, EditAction,
	ObjectSelectionKind, PendingAction, PendingKind, ScrollAmount, ScrollDir, VisualDirection,
	cursor_motion, dispatch_result, insert_with_motion, selection_motion,
};
pub use evildoer_registry::commands::{CommandContext, CommandDef, flags};
pub use completion::{CompletionContext, CompletionItem, CompletionKind, CompletionSource};
pub use editor_ctx::{EditorCapabilities, EditorContext, EditorOps, HandleOutcome};
pub use evildoer_registry::hooks::{
	BoxFuture as HookBoxFuture, HOOKS, HookAction, HookContext, HookDef, HookEvent, HookEventData,
	HookHandler, HookMutability, HookResult, HookScheduler, MutableHookContext, OwnedHookContext,
	all_hooks, emit as emit_hook, emit_mutable as emit_mutable_hook, emit_sync as emit_hook_sync,
	emit_sync_with as emit_hook_sync_with, find_hooks,
};
pub use index::{
	all_actions, all_commands, all_motions, all_text_objects, find_action, find_action_by_id,
	find_command, find_motion, find_text_object_by_trigger, resolve_action_id,
};
pub use keybindings::{BindingMode, KEYBINDINGS, KeyBindingDef};
pub use keymap_registry::{BindingEntry, KeymapRegistry, LookupResult, get_keymap_registry};
pub use mode::Mode;
pub use evildoer_registry::{MOTIONS, MotionDef};
pub use evildoer_registry::notifications::{
	Animation, AutoDismiss, Level, NOTIFICATION_TYPES, NotificationTypeDef, Timing,
	find_notification_type,
};
pub use evildoer_registry::options::{OPTIONS, OptionDef, OptionScope, OptionType, OptionValue};
pub use evildoer_registry::panels::{
	all_panels, find_factory, find_panel, find_panel_by_id, panel_kind_index, PanelDef,
	PanelFactory, PanelFactoryDef, PanelId, PANEL_FACTORIES, PANELS,
};
pub use split_buffer::{
	SplitAttrs, SplitBuffer, SplitCell, SplitColor, SplitCursor, SplitCursorStyle,
	SplitDockPreference, SplitEventResult, SplitKey, SplitKeyCode, SplitModifiers, SplitMouse,
	SplitMouseAction, SplitMouseButton, SplitSize,
};
pub use evildoer_registry::statusline::{
	RenderedSegment, STATUSLINE_SEGMENTS, SegmentPosition, SegmentStyle, StatuslineContext,
	StatuslineSegmentDef, all_segments, find_segment, render_position, segments_for_position,
};
pub use terminal_config::{TerminalConfig, TerminalSequence};
pub use evildoer_registry::text_objects::{TEXT_OBJECTS, TextObjectDef, TextObjectHandler};
pub use theme::{
	DEFAULT_THEME, DEFAULT_THEME_ID, NotificationColors, OwnedTheme, PopupColors,
	SemanticColorPair, StatusColors, THEMES, Theme, ThemeColors, ThemeSource, ThemeVariant,
	UiColors, blend_colors, get_theme, register_runtime_themes, runtime_themes, suggest_theme,
};
