//! Keymap preset specification schema.
//!
//! Defines the declarative format for keymap preset files (e.g., `vim.nuon`,
//! `emacs.nuon`). Each preset declares a set of key-to-target bindings and
//! named prefix groups, compiled at build time into binary blobs for O(1)
//! runtime access.


use serde::{Deserialize, Serialize};

/// A complete keymap preset specification.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KeymapPresetSpec {
	/// Human-readable name (e.g., `"vim"`, `"emacs"`).
	pub name: String,
	/// Initial editor mode (`"normal"` or `"insert"`). Defaults to `"normal"`.
	#[serde(default = "default_initial_mode")]
	pub initial_mode: String,
	/// Behavioral tuning knobs for this preset.
	#[serde(default)]
	pub behavior: PresetBehaviorSpec,
	/// Key-to-target bindings.
	#[serde(default)]
	pub bindings: Vec<PresetBindingSpec>,
	/// Named prefix groups for which-key HUD.
	#[serde(default)]
	pub prefixes: Vec<PresetPrefixSpec>,
}

fn default_initial_mode() -> String {
	"normal".to_string()
}

/// Behavioral flags that control input handling semantics per preset.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PresetBehaviorSpec {
	/// Shift+letter casefolds to uppercase for keymap lookup (vim semantics).
	/// When false, Shift is kept as a modifier (emacs semantics).
	#[serde(default = "default_true")]
	pub vim_shift_letter_casefold: bool,
	/// Bare digits in Normal mode accumulate a count prefix.
	#[serde(default = "default_true")]
	pub normal_digit_prefix_count: bool,
}

impl Default for PresetBehaviorSpec {
	fn default() -> Self {
		Self {
			vim_shift_letter_casefold: true,
			normal_digit_prefix_count: true,
		}
	}
}

fn default_true() -> bool {
	true
}

/// A single binding in a preset: maps a mode + key sequence to a target spec.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PresetBindingSpec {
	/// Binding mode (e.g., `"normal"`, `"insert"`).
	pub mode: String,
	/// Key sequence string (e.g., `"g g"`, `"ctrl-home"`).
	pub keys: String,
	/// Invocation spec string (e.g., `"action:move_left"`, `"command:write"`).
	pub target: String,
}

/// A named prefix group for which-key display.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PresetPrefixSpec {
	/// Binding mode.
	pub mode: String,
	/// Prefix key sequence (e.g., `"g"`, `"ctrl-w"`).
	pub keys: String,
	/// Human-readable description (e.g., `"Goto"`, `"Window"`).
	pub description: String,
}
