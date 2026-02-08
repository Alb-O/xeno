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
	pub keys: Vec<String>,
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
