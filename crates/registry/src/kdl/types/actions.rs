use serde::{Deserialize, Serialize};

/// Raw action metadata extracted from KDL, before handler linking.
#[derive(Debug, Serialize, Deserialize)]
pub struct ActionMetaRaw {
	/// Common metadata.
	pub common: super::common::MetaCommonRaw,
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
