//!
//! Keybindings map key sequences to actions in different modes. Uses a trie-based
//! registry for efficient sequence matching (e.g., `g g` for document_start).
//!
//! All keybindings are colocated with their action definitions using the `action!`
//! macro with `bindings:` syntax:
//!
//! ```ignore
//! action!(
//!     document_start,
//!     {
//!         description: "Move to document start",
//!         bindings: r#"
//!             normal "g g" "ctrl-home"
//!             insert "ctrl-home"
//!         "#
//!     },
//!     |_ctx| { ... }
//! );
//! ```

use std::sync::LazyLock;

use crate::impls;
use xeno_primitives::Mode;

static KEYBINDING_SETS: &[&[KeyBindingDef]] = &[
	impls::editing::KEYBINDINGS_delete,
	impls::editing::KEYBINDINGS_delete_no_yank,
	impls::editing::KEYBINDINGS_change,
	impls::editing::KEYBINDINGS_change_no_yank,
	impls::editing::KEYBINDINGS_yank,
	impls::editing::KEYBINDINGS_paste_after,
	impls::editing::KEYBINDINGS_paste_before,
	impls::editing::KEYBINDINGS_paste_all_after,
	impls::editing::KEYBINDINGS_paste_all_before,
	impls::editing::KEYBINDINGS_undo,
	impls::editing::KEYBINDINGS_redo,
	impls::editing::KEYBINDINGS_indent,
	impls::editing::KEYBINDINGS_deindent,
	impls::editing::KEYBINDINGS_to_lowercase,
	impls::editing::KEYBINDINGS_to_uppercase,
	impls::editing::KEYBINDINGS_swap_case,
	impls::editing::KEYBINDINGS_join_lines,
	impls::editing::KEYBINDINGS_open_below,
	impls::editing::KEYBINDINGS_open_above,
	impls::editing::KEYBINDINGS_delete_back,
	impls::editing::KEYBINDINGS_delete_word_back,
	impls::editing::KEYBINDINGS_delete_word_forward,
	impls::editing::KEYBINDINGS_replace_char,
	impls::find::KEYBINDINGS_find_char,
	impls::find::KEYBINDINGS_find_char_to,
	impls::find::KEYBINDINGS_find_char_reverse,
	impls::find::KEYBINDINGS_find_char_to_reverse,
	impls::insert::KEYBINDINGS_insert_mode,
	impls::insert::KEYBINDINGS_insert_line_start,
	impls::insert::KEYBINDINGS_insert_line_end,
	impls::insert::KEYBINDINGS_insert_after,
	impls::insert::KEYBINDINGS_insert_newline,
	impls::misc::KEYBINDINGS_add_line_below,
	impls::misc::KEYBINDINGS_add_line_above,
	impls::misc::KEYBINDINGS_use_selection_as_search,
	impls::modes::KEYBINDINGS_normal_mode,
	impls::motions::KEYBINDINGS_move_left,
	impls::motions::KEYBINDINGS_move_right,
	impls::motions::KEYBINDINGS_move_line_start,
	impls::motions::KEYBINDINGS_move_line_end,
	impls::motions::KEYBINDINGS_next_word_start,
	impls::motions::KEYBINDINGS_prev_word_start,
	impls::motions::KEYBINDINGS_next_word_end,
	impls::motions::KEYBINDINGS_next_long_word_start,
	impls::motions::KEYBINDINGS_prev_long_word_start,
	impls::motions::KEYBINDINGS_next_long_word_end,
	impls::motions::KEYBINDINGS_select_word_forward,
	impls::motions::KEYBINDINGS_select_word_backward,
	impls::motions::KEYBINDINGS_select_word_end,
	impls::motions::KEYBINDINGS_next_paragraph,
	impls::motions::KEYBINDINGS_prev_paragraph,
	impls::motions::KEYBINDINGS_document_start,
	impls::motions::KEYBINDINGS_document_end,
	impls::motions::KEYBINDINGS_goto_line_start,
	impls::motions::KEYBINDINGS_goto_line_end,
	impls::motions::KEYBINDINGS_goto_first_nonwhitespace,
	impls::motions::KEYBINDINGS_move_top_screen,
	impls::motions::KEYBINDINGS_move_middle_screen,
	impls::motions::KEYBINDINGS_move_bottom_screen,
	impls::palette::KEYBINDINGS_open_palette,
	impls::scroll::KEYBINDINGS_scroll_up,
	impls::scroll::KEYBINDINGS_scroll_down,
	impls::scroll::KEYBINDINGS_scroll_half_page_up,
	impls::scroll::KEYBINDINGS_scroll_half_page_down,
	impls::scroll::KEYBINDINGS_scroll_page_up,
	impls::scroll::KEYBINDINGS_scroll_page_down,
	impls::scroll::KEYBINDINGS_move_up_visual,
	impls::scroll::KEYBINDINGS_move_down_visual,
	impls::selection_ops::KEYBINDINGS_collapse_selection,
	impls::selection_ops::KEYBINDINGS_flip_selection,
	impls::selection_ops::KEYBINDINGS_ensure_forward,
	impls::selection_ops::KEYBINDINGS_select_line,
	impls::selection_ops::KEYBINDINGS_select_all,
	impls::selection_ops::KEYBINDINGS_expand_to_line,
	impls::selection_ops::KEYBINDINGS_remove_primary_selection,
	impls::selection_ops::KEYBINDINGS_remove_selections_except_primary,
	impls::selection_ops::KEYBINDINGS_rotate_selections_forward,
	impls::selection_ops::KEYBINDINGS_rotate_selections_backward,
	impls::selection_ops::KEYBINDINGS_split_lines,
	impls::selection_ops::KEYBINDINGS_duplicate_selections_down,
	impls::selection_ops::KEYBINDINGS_duplicate_selections_up,
	impls::selection_ops::KEYBINDINGS_merge_selections,
	impls::text_objects::KEYBINDINGS_select_object_inner,
	impls::text_objects::KEYBINDINGS_select_object_around,
	impls::text_objects::KEYBINDINGS_select_object_to_start,
	impls::text_objects::KEYBINDINGS_select_object_to_end,
	impls::window::KEYBINDINGS_split_horizontal,
	impls::window::KEYBINDINGS_split_vertical,
	impls::window::KEYBINDINGS_focus_left,
	impls::window::KEYBINDINGS_focus_down,
	impls::window::KEYBINDINGS_focus_up,
	impls::window::KEYBINDINGS_focus_right,
	impls::window::KEYBINDINGS_buffer_next,
	impls::window::KEYBINDINGS_buffer_prev,
	impls::window::KEYBINDINGS_close_split,
	impls::window::KEYBINDINGS_close_other_buffers,
];

/// List of key sequence bindings.
///
/// Populated at compile time by the `action!` macro's `bindings:` syntax.
pub static KEYBINDINGS: LazyLock<Vec<KeyBindingDef>> = LazyLock::new(|| {
	let mut bindings = Vec::new();
	for defs in KEYBINDING_SETS {
		bindings.extend_from_slice(defs);
	}
	bindings
});

/// List of key sequence prefix descriptions.
///
/// Used by the which-key HUD to show a description for the pressed prefix key.
/// For example, pressing `g` shows "Goto..." as the prefix description.
pub static KEY_PREFIXES: LazyLock<Vec<KeyPrefixDef>> = LazyLock::new(|| {
	vec![
		impls::prefixes::KEY_PREFIX_NORMAL_G,
		impls::prefixes::KEY_PREFIX_NORMAL_Z,
		impls::prefixes::KEY_PREFIX_NORMAL_CTRL_W,
		impls::prefixes::KEY_PREFIX_NORMAL_CTRL_W_S,
		impls::prefixes::KEY_PREFIX_NORMAL_CTRL_W_F,
		impls::prefixes::KEY_PREFIX_NORMAL_CTRL_W_C,
	]
});

/// Definition of a key sequence prefix with its description.
///
/// Registered via the `key_prefix!` macro:
///
/// ```ignore
/// key_prefix!(normal "g" => "Goto");
/// key_prefix!(normal "z" => "View");
/// ```
#[derive(Debug, Clone, Copy)]
pub struct KeyPrefixDef {
	/// Mode this prefix is active in.
	pub mode: BindingMode,
	/// The prefix key sequence (e.g., `"g"`, `"z"`).
	pub keys: &'static str,
	/// Human-readable description (e.g., "Goto", "View").
	pub description: &'static str,
}

/// Finds a prefix definition for the given mode and key sequence.
pub fn find_prefix(mode: BindingMode, keys: &str) -> Option<&'static KeyPrefixDef> {
	KEY_PREFIXES
		.iter()
		.find(|p| p.mode == mode && p.keys == keys)
}

/// Key sequence binding definition.
///
/// Maps a key sequence (e.g., `"g g"`, `"ctrl-home"`) to an action in a mode.
#[derive(Clone, Copy)]
pub struct KeyBindingDef {
	/// Mode this binding is active in.
	pub mode: BindingMode,
	/// Key sequence string (e.g., `"g g"`, `"ctrl-home"`).
	/// Parsed with `parse_seq()` at registry initialization.
	pub keys: &'static str,
	/// Action to execute (looked up by name in the action registry).
	pub action: &'static str,
	/// Priority for conflict resolution (lower wins).
	/// Default bindings use 100; user overrides should use lower values.
	pub priority: i16,
}

impl std::fmt::Debug for KeyBindingDef {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		f.debug_struct("KeyBindingDef")
			.field("mode", &self.mode)
			.field("keys", &self.keys)
			.field("action", &self.action)
			.field("priority", &self.priority)
			.finish()
	}
}

/// Mode in which a keybinding is active.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BindingMode {
	/// Normal mode (default editing mode).
	Normal,
	/// Insert mode (text input).
	Insert,
	/// Match mode (m prefix).
	Match,
	/// Space mode (space prefix).
	Space,
}

impl From<Mode> for BindingMode {
	fn from(mode: Mode) -> Self {
		match mode {
			Mode::Normal => BindingMode::Normal,
			Mode::Insert => BindingMode::Insert,
			Mode::PendingAction(_) => BindingMode::Normal,
		}
	}
}
