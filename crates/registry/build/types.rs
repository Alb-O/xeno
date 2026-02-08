use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct MetaCommonSpec {
	pub name: String,
	pub description: String,
	pub short_desc: Option<String>,
	pub keys: Vec<String>,
	pub priority: i16,
	pub caps: Vec<String>,
	pub flags: u32,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ActionSpec {
	pub common: MetaCommonSpec,
	pub bindings: Vec<KeyBindingSpec>,
	pub group: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct KeyBindingSpec {
	pub mode: String,
	pub keys: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct KeyPrefixSpec {
	pub mode: String,
	pub keys: String,
	pub description: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ActionsSpec {
	pub actions: Vec<ActionSpec>,
	pub prefixes: Vec<KeyPrefixSpec>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CommandSpec {
	pub common: MetaCommonSpec,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CommandsSpec {
	pub commands: Vec<CommandSpec>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct MotionSpec {
	pub common: MetaCommonSpec,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct MotionsSpec {
	pub motions: Vec<MotionSpec>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct TextObjectSpec {
	pub common: MetaCommonSpec,
	pub trigger: String,
	pub alt_triggers: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct TextObjectsSpec {
	pub text_objects: Vec<TextObjectSpec>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct OptionSpec {
	/// Common metadata.
	pub common: MetaCommonSpec,
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
pub struct OptionsSpec {
	pub options: Vec<OptionSpec>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct GutterSpec {
	pub common: MetaCommonSpec,
	pub width: String,
	pub enabled: bool,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct GuttersSpec {
	pub gutters: Vec<GutterSpec>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct StatuslineSegmentSpec {
	pub common: MetaCommonSpec,
	pub position: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct StatuslineSpec {
	pub segments: Vec<StatuslineSegmentSpec>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct HookSpec {
	pub common: MetaCommonSpec,
	pub event: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct HooksSpec {
	pub hooks: Vec<HookSpec>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct NotificationSpec {
	pub common: MetaCommonSpec,
	pub level: String,
	pub auto_dismiss: String,
	pub dismiss_ms: Option<u64>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct NotificationsSpec {
	pub notifications: Vec<NotificationSpec>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ThemeSpec {
	/// Common metadata.
	pub common: MetaCommonSpec,
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
pub struct ThemesSpec {
	pub themes: Vec<ThemeSpec>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct LanguageSpec {
	pub common: MetaCommonSpec,
	pub scope: Option<String>,
	pub grammar_name: Option<String>,
	pub injection_regex: Option<String>,
	pub auto_format: bool,
	pub extensions: Vec<String>,
	pub filenames: Vec<String>,
	pub globs: Vec<String>,
	pub shebangs: Vec<String>,
	pub comment_tokens: Vec<String>,
	pub block_comment: Option<(String, String)>,
	pub lsp_servers: Vec<String>,
	pub roots: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct LanguagesSpec {
	pub langs: Vec<LanguageSpec>,
}
