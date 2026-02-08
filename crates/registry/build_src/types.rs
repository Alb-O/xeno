use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct MetaCommonRaw {
	pub name: String,
	pub description: String,
	pub short_desc: Option<String>,
	pub keys: Vec<String>,
	pub priority: i16,
	pub caps: Vec<String>,
	pub flags: u32,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ActionMetaRaw {
	pub common: MetaCommonRaw,
	pub bindings: Vec<KeyBindingRaw>,
	pub group: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct KeyBindingRaw {
	pub mode: String,
	pub keys: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct KeyPrefixRaw {
	pub mode: String,
	pub keys: String,
	pub description: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ActionsBlob {
	pub actions: Vec<ActionMetaRaw>,
	pub prefixes: Vec<KeyPrefixRaw>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CommandMetaRaw {
	pub common: MetaCommonRaw,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CommandsBlob {
	pub commands: Vec<CommandMetaRaw>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct MotionMetaRaw {
	pub common: MetaCommonRaw,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct MotionsBlob {
	pub motions: Vec<MotionMetaRaw>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct TextObjectMetaRaw {
	pub common: MetaCommonRaw,
	pub trigger: String,
	pub alt_triggers: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct TextObjectsBlob {
	pub text_objects: Vec<TextObjectMetaRaw>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct OptionMetaRaw {
	/// Common metadata.
	pub common: MetaCommonRaw,
	/// KDL config key (e.g., `"tab-width"`).
	pub kdl_key: String,
	/// Value type: `"bool"`, `"int"`, `"string"`.
	pub value_type: String,
	/// Default value as a string.
	pub default: String,
	/// Scope: `"buffer"` or `"global"`.
	pub scope: String,
	/// Optional validator name.
	pub validator: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct OptionsBlob {
	pub options: Vec<OptionMetaRaw>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct GutterMetaRaw {
	pub common: MetaCommonRaw,
	pub width: String,
	pub enabled: bool,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct GuttersBlob {
	pub gutters: Vec<GutterMetaRaw>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct StatuslineMetaRaw {
	pub common: MetaCommonRaw,
	pub position: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct StatuslineBlob {
	pub segments: Vec<StatuslineMetaRaw>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct HookMetaRaw {
	pub common: MetaCommonRaw,
	pub event: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct HooksBlob {
	pub hooks: Vec<HookMetaRaw>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct NotificationMetaRaw {
	pub common: MetaCommonRaw,
	pub level: String,
	pub auto_dismiss: String,
	pub dismiss_ms: Option<u64>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct NotificationsBlob {
	pub notifications: Vec<NotificationMetaRaw>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ThemeMetaRaw {
	/// Common metadata.
	pub common: MetaCommonRaw,
	/// Whether it's a "dark" or "light" theme.
	pub variant: String,
	/// Resolved color palette: Map of name -> hex string.
	pub palette: std::collections::HashMap<String, String>,
	/// UI colors: Map of field -> color name or hex.
	pub ui: std::collections::HashMap<String, String>,
	/// Mode colors: Map of field -> color name or hex.
	pub mode: std::collections::HashMap<String, String>,
	/// Semantic colors: Map of field -> color name or hex.
	pub semantic: std::collections::HashMap<String, String>,
	/// Popup colors: Map of field -> color name or hex.
	pub popup: std::collections::HashMap<String, String>,
	/// Syntax styles: Map of scope -> raw style.
	pub syntax: std::collections::HashMap<String, RawStyle>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct RawStyle {
	pub fg: Option<String>,
	pub bg: Option<String>,
	pub modifiers: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ThemesBlob {
	pub themes: Vec<ThemeMetaRaw>,
}
