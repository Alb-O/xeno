//! Serializable intermediate types for KDL-to-registry pipeline.
//!
//! These types are used by both the build script (serialization) and runtime
//! (deserialization) to transfer registry metadata through postcard binary blobs.
//! Each domain has a `*MetaRaw` type for individual entries and a `*Blob` type
//! for the top-level container.

use serde::{Deserialize, Serialize};

/// Raw action metadata extracted from KDL, before handler linking.
#[derive(Debug, Serialize, Deserialize)]
pub struct ActionMetaRaw {
	/// Action name (handler linkage key).
	pub name: String,
	/// Human-readable description.
	pub description: String,
	/// Short description for which-key HUD (defaults to description if absent).
	pub short_desc: Option<String>,
	/// Alternative lookup names.
	pub aliases: Vec<String>,
	/// Conflict resolution priority.
	pub priority: i16,
	/// Required capability names (parsed to `Capability` enum at link time).
	pub caps: Vec<String>,
	/// Behavior hint flags.
	pub flags: u32,
	/// Key bindings per mode.
	pub bindings: Vec<KeyBindingRaw>,
	/// Organizational group name (informational only).
	pub group: Option<String>,
}

/// Raw key binding from KDL.
#[derive(Debug, Serialize, Deserialize)]
pub struct KeyBindingRaw {
	/// Mode name: "normal", "insert", "match", "space".
	pub mode: String,
	/// Key sequence string (e.g., "g g", "ctrl-home").
	pub keys: String,
}

/// Raw key prefix definition from KDL.
#[derive(Debug, Serialize, Deserialize)]
pub struct KeyPrefixRaw {
	/// Mode name.
	pub mode: String,
	/// Prefix key sequence.
	pub keys: String,
	/// Which-key HUD label.
	pub description: String,
}

/// Top-level blob containing all action and prefix data.
#[derive(Debug, Serialize, Deserialize)]
pub struct ActionsBlob {
	/// All action definitions.
	pub actions: Vec<ActionMetaRaw>,
	/// All key prefix definitions.
	pub prefixes: Vec<KeyPrefixRaw>,
}

// ── Commands ──────────────────────────────────────────────────────────

/// Raw command metadata extracted from KDL, before handler linking.
#[derive(Debug, Serialize, Deserialize)]
pub struct CommandMetaRaw {
	/// Command name (handler linkage key).
	pub name: String,
	/// Human-readable description.
	pub description: String,
	/// Alternative lookup names (e.g., `"q"` for `"quit"`).
	pub aliases: Vec<String>,
}

/// Top-level blob containing all command metadata.
#[derive(Debug, Serialize, Deserialize)]
pub struct CommandsBlob {
	/// All command definitions.
	pub commands: Vec<CommandMetaRaw>,
}

// ── Motions ───────────────────────────────────────────────────────────

/// Raw motion metadata extracted from KDL, before handler linking.
#[derive(Debug, Serialize, Deserialize)]
pub struct MotionMetaRaw {
	/// Motion name (handler linkage key).
	pub name: String,
	/// Human-readable description.
	pub description: String,
	/// Alternative lookup names.
	pub aliases: Vec<String>,
}

/// Top-level blob containing all motion metadata.
#[derive(Debug, Serialize, Deserialize)]
pub struct MotionsBlob {
	/// All motion definitions.
	pub motions: Vec<MotionMetaRaw>,
}

// ── Text Objects ──────────────────────────────────────────────────────

/// Raw text object metadata extracted from KDL, before handler linking.
#[derive(Debug, Serialize, Deserialize)]
pub struct TextObjectMetaRaw {
	/// Text object name (handler linkage key).
	pub name: String,
	/// Human-readable description.
	pub description: String,
	/// Primary trigger character (e.g., `"w"`, `"("`).
	pub trigger: String,
	/// Alternate trigger characters.
	pub alt_triggers: Vec<String>,
}

/// Top-level blob containing all text object metadata.
#[derive(Debug, Serialize, Deserialize)]
pub struct TextObjectsBlob {
	/// All text object definitions.
	pub text_objects: Vec<TextObjectMetaRaw>,
}

// ── Options ───────────────────────────────────────────────────────────

/// Raw option metadata extracted from KDL.
#[derive(Debug, Serialize, Deserialize)]
pub struct OptionMetaRaw {
	/// Option name (handler linkage key).
	pub name: String,
	/// KDL config key (e.g., `"tab-width"`).
	pub kdl_key: String,
	/// Value type: `"bool"`, `"int"`, `"string"`.
	pub value_type: String,
	/// Default value as a string.
	pub default: String,
	/// Scope: `"buffer"` or `"global"`.
	pub scope: String,
	/// Human-readable description.
	pub description: String,
}

/// Top-level blob containing all option metadata.
#[derive(Debug, Serialize, Deserialize)]
pub struct OptionsBlob {
	/// All option definitions.
	pub options: Vec<OptionMetaRaw>,
}

// ── Gutters ───────────────────────────────────────────────────────────

/// Raw gutter metadata extracted from KDL.
#[derive(Debug, Serialize, Deserialize)]
pub struct GutterMetaRaw {
	/// Gutter name (handler linkage key).
	pub name: String,
	/// Human-readable description.
	pub description: String,
	/// Rendering priority (lower = further left).
	pub priority: i16,
	/// Width: `"dynamic"` or a fixed integer as string.
	pub width: String,
	/// Whether enabled by default.
	pub enabled: bool,
}

/// Top-level blob containing all gutter metadata.
#[derive(Debug, Serialize, Deserialize)]
pub struct GuttersBlob {
	/// All gutter definitions.
	pub gutters: Vec<GutterMetaRaw>,
}

// ── Statusline ────────────────────────────────────────────────────────

/// Raw statusline segment metadata extracted from KDL.
#[derive(Debug, Serialize, Deserialize)]
pub struct StatuslineMetaRaw {
	/// Segment name (handler linkage key).
	pub name: String,
	/// Human-readable description.
	pub description: String,
	/// Position: `"left"` or `"right"`.
	pub position: String,
	/// Rendering priority within position group.
	pub priority: i16,
}

/// Top-level blob containing all statusline segment metadata.
#[derive(Debug, Serialize, Deserialize)]
pub struct StatuslineBlob {
	/// All statusline segment definitions.
	pub segments: Vec<StatuslineMetaRaw>,
}

// ── Hooks ─────────────────────────────────────────────────────────────

/// Raw hook metadata extracted from KDL.
#[derive(Debug, Serialize, Deserialize)]
pub struct HookMetaRaw {
	/// Hook name (handler linkage key).
	pub name: String,
	/// Event name this hook listens to.
	pub event: String,
	/// Execution priority (lower = earlier).
	pub priority: i16,
	/// Human-readable description.
	pub description: String,
}

/// Top-level blob containing all hook metadata.
#[derive(Debug, Serialize, Deserialize)]
pub struct HooksBlob {
	/// All hook definitions.
	pub hooks: Vec<HookMetaRaw>,
}
