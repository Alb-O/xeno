use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct ActionMetaRaw {
	pub name: String,
	pub description: String,
	pub short_desc: Option<String>,
	pub keys: Vec<String>,
	pub priority: i16,
	pub caps: Vec<String>,
	pub flags: u32,
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
	pub name: String,
	pub description: String,
	pub keys: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CommandsBlob {
	pub commands: Vec<CommandMetaRaw>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct MotionMetaRaw {
	pub name: String,
	pub description: String,
	pub keys: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct MotionsBlob {
	pub motions: Vec<MotionMetaRaw>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct TextObjectMetaRaw {
	pub name: String,
	pub description: String,
	pub trigger: String,
	pub alt_triggers: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct TextObjectsBlob {
	pub text_objects: Vec<TextObjectMetaRaw>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct OptionMetaRaw {
	pub name: String,
	pub keys: Vec<String>,
	pub priority: i16,
	pub flags: u32,
	pub kdl_key: String,
	pub value_type: String,
	pub default: String,
	pub scope: String,
	pub description: String,
	pub validator: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct OptionsBlob {
	pub options: Vec<OptionMetaRaw>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct GutterMetaRaw {
	pub name: String,
	pub description: String,
	pub priority: i16,
	pub width: String,
	pub enabled: bool,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct GuttersBlob {
	pub gutters: Vec<GutterMetaRaw>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct StatuslineMetaRaw {
	pub name: String,
	pub description: String,
	pub position: String,
	pub priority: i16,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct StatuslineBlob {
	pub segments: Vec<StatuslineMetaRaw>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct HookMetaRaw {
	pub name: String,
	pub event: String,
	pub priority: i16,
	pub description: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct HooksBlob {
	pub hooks: Vec<HookMetaRaw>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct NotificationMetaRaw {
	pub name: String,
	pub level: String,
	pub auto_dismiss: String,
	pub dismiss_ms: Option<u64>,
	pub description: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct NotificationsBlob {
	pub notifications: Vec<NotificationMetaRaw>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ThemeMetaRaw {
	pub name: String,
	pub keys: Vec<String>,
	pub description: String,
	pub priority: i16,
	pub variant: String,
	pub palette: std::collections::HashMap<String, String>,
	pub ui: std::collections::HashMap<String, String>,
	pub mode: std::collections::HashMap<String, String>,
	pub semantic: std::collections::HashMap<String, String>,
	pub popup: std::collections::HashMap<String, String>,
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
