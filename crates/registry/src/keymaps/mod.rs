//! Keymap preset loading, resolution, and types.
//!
//! Provides compile-time-embedded keymap presets (e.g., `vim`, `emacs`) and
//! runtime loading of user-defined preset files. Presets define the base
//! binding set, initial mode, and behavioral tuning for the editor.
//!
//! Resolution order for a preset spec string:
//! * Builtin name (e.g., `"vim"`, `"emacs"`)
//! * Explicit file path (contains `/` or `\` or ends with `.nuon`)
//! * Convention path: `<config_dir>/keymaps/<name>.nuon`

use std::path::Path;
use std::sync::Arc;

use xeno_primitives::Mode;
use crate::schema::keymaps::KeymapPresetSpec;

/// Default preset name used when no preset is specified in config.
pub const DEFAULT_PRESET: &str = "vim";

/// Arc-wrapped preset for shared ownership.
pub type KeymapPresetRef = Arc<KeymapPreset>;

/// A loaded keymap preset with parsed binding and prefix data.
#[derive(Debug, Clone)]
pub struct KeymapPreset {
	/// Preset name (e.g., `"vim"`).
	pub name: Arc<str>,
	/// Initial editor mode for this preset.
	pub initial_mode: Mode,
	/// Behavioral flags controlling input handling semantics.
	pub behavior: KeymapBehavior,
	/// Bindings from key sequences to invocation spec strings.
	pub bindings: Vec<PresetBinding>,
	/// Named prefix groups for which-key HUD.
	pub prefixes: Vec<PresetPrefix>,
}

/// Behavioral flags that control input handling per preset.
#[derive(Debug, Clone, Copy)]
pub struct KeymapBehavior {
	/// Shift+letter casefolds to uppercase for keymap lookup (vim semantics).
	pub vim_shift_letter_casefold: bool,
	/// Bare digits in Normal mode accumulate a count prefix.
	pub normal_digit_prefix_count: bool,
}

impl Default for KeymapBehavior {
	fn default() -> Self {
		Self {
			vim_shift_letter_casefold: true,
			normal_digit_prefix_count: true,
		}
	}
}

/// A single binding in a preset.
#[derive(Debug, Clone)]
pub struct PresetBinding {
	/// Binding mode name (e.g., `"normal"`, `"insert"`).
	pub mode: String,
	/// Key sequence string (e.g., `"g g"`, `"ctrl-home"`).
	pub keys: Arc<str>,
	/// Invocation spec string (e.g., `"action:move_left"`).
	pub target: String,
}

/// A named prefix group for which-key display.
#[derive(Debug, Clone)]
pub struct PresetPrefix {
	/// Binding mode name.
	pub mode: String,
	/// Prefix key sequence (e.g., `"g"`, `"ctrl-w"`).
	pub keys: Arc<str>,
	/// Human-readable description (e.g., `"Goto"`).
	pub description: Arc<str>,
}

/// Errors encountered when loading or resolving a keymap preset.
#[derive(Debug)]
pub enum KeymapPresetError {
	/// File I/O error.
	Io(std::io::Error),
	/// NUON parse error.
	Parse(String),
	/// Invalid field value in preset.
	Invalid(String),
}

impl std::fmt::Display for KeymapPresetError {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		match self {
			Self::Io(e) => write!(f, "preset I/O error: {e}"),
			Self::Parse(msg) => write!(f, "preset parse error: {msg}"),
			Self::Invalid(msg) => write!(f, "preset validation error: {msg}"),
		}
	}
}

impl From<KeymapPresetSpec> for KeymapPreset {
	fn from(spec: KeymapPresetSpec) -> Self {
		Self {
			name: Arc::from(spec.name.as_str()),
			initial_mode: parse_initial_mode_safe(&spec.initial_mode).unwrap_or(Mode::Normal),
			behavior: KeymapBehavior {
				vim_shift_letter_casefold: spec.behavior.vim_shift_letter_casefold,
				normal_digit_prefix_count: spec.behavior.normal_digit_prefix_count,
			},
			bindings: spec
				.bindings
				.into_iter()
				.map(|b| PresetBinding {
					mode: b.mode,
					keys: Arc::from(b.keys.as_str()),
					target: b.target,
				})
				.collect(),
			prefixes: spec
				.prefixes
				.into_iter()
				.map(|p| PresetPrefix {
					mode: p.mode,
					keys: Arc::from(p.keys.as_str()),
					description: Arc::from(p.description.as_str()),
				})
				.collect(),
		}
	}
}

fn parse_initial_mode_safe(s: &str) -> Result<Mode, KeymapPresetError> {
	match s {
		"normal" => Ok(Mode::Normal),
		"insert" => Ok(Mode::Insert),
		other => Err(KeymapPresetError::Invalid(format!("unknown initial_mode: {other:?}"))),
	}
}

// ── Builtin presets ──────────────────────────────────────────────────

fn load_vim_preset() -> KeymapPresetRef {
	const BYTES: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/keymap_vim.bin"));
	let spec: KeymapPresetSpec = crate::defs::loader::load_blob(BYTES, "keymap_vim");
	Arc::new(spec.into())
}

fn load_emacs_preset() -> KeymapPresetRef {
	const BYTES: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/keymap_emacs.bin"));
	let spec: KeymapPresetSpec = crate::defs::loader::load_blob(BYTES, "keymap_emacs");
	Arc::new(spec.into())
}

/// Returns a builtin preset by name, or `None` if unknown.
pub fn builtin(name: &str) -> Option<KeymapPresetRef> {
	static VIM: std::sync::OnceLock<KeymapPresetRef> = std::sync::OnceLock::new();
	static EMACS: std::sync::OnceLock<KeymapPresetRef> = std::sync::OnceLock::new();

	match name {
		"vim" => Some(Arc::clone(VIM.get_or_init(load_vim_preset))),
		"emacs" => Some(Arc::clone(EMACS.get_or_init(load_emacs_preset))),
		_ => None,
	}
}

/// Legacy alias for `builtin()`.
pub fn preset(name: &str) -> Option<KeymapPresetRef> {
	builtin(name)
}

// ── Runtime loading ──────────────────────────────────────────────────

/// Parses a keymap preset from a NUON string.
///
/// Uses the same NUON parser as config files. The `source` parameter is used
/// for error messages only.
#[cfg(feature = "config-nuon")]
pub fn load_from_str(input: &str, source: &str) -> Result<KeymapPreset, KeymapPresetError> {
	let value = xeno_nu_api::parse_nuon(input).map_err(|e| KeymapPresetError::Parse(format!("{source}: {e}")))?;

	let record = value
		.as_record()
		.map_err(|_| KeymapPresetError::Parse(format!("{source}: expected record at root")))?;

	let name = record.get("name").and_then(|v| v.as_str().ok()).unwrap_or(source).to_string();

	let initial_mode_str = record.get("initial_mode").and_then(|v| v.as_str().ok()).unwrap_or("normal");
	let initial_mode = parse_initial_mode_safe(initial_mode_str)?;

	let behavior = parse_behavior(record.get("behavior"))?;
	let bindings = parse_bindings_list(record.get("bindings"), source)?;
	let prefixes = parse_prefixes_list(record.get("prefixes"), source)?;

	Ok(KeymapPreset {
		name: Arc::from(name.as_str()),
		initial_mode,
		behavior,
		bindings,
		prefixes,
	})
}

/// Loads a keymap preset from a NUON file.
#[cfg(feature = "config-nuon")]
pub fn load_from_path(path: &Path) -> Result<KeymapPresetRef, KeymapPresetError> {
	let input = std::fs::read_to_string(path).map_err(KeymapPresetError::Io)?;
	let source = path.file_stem().and_then(|s| s.to_str()).unwrap_or("custom");
	let preset = load_from_str(&input, source)?;
	Ok(Arc::new(preset))
}

// ── Resolution ───────────────────────────────────────────────────────

/// Resolves a preset spec string to a loaded preset.
///
/// Resolution order:
/// * Builtin name match (e.g., `"vim"`, `"emacs"`)
/// * Explicit file path (contains `/` or `\` or ends with `.nuon`)
/// * Convention: `<base_dir>/keymaps/<spec>.nuon`
#[cfg(feature = "config-nuon")]
pub fn resolve(spec: &str, base_dir: Option<&Path>) -> Result<KeymapPresetRef, KeymapPresetError> {
	// 1. Try builtin
	if let Some(p) = builtin(spec) {
		return Ok(p);
	}

	// 2. Explicit path
	let is_path = spec.contains('/') || spec.contains('\\') || spec.ends_with(".nuon");
	if is_path {
		let path = if Path::new(spec).is_absolute() {
			std::path::PathBuf::from(spec)
		} else if let Some(base) = base_dir {
			base.join(spec)
		} else {
			std::path::PathBuf::from(spec)
		};
		return load_from_path(&path);
	}

	// 3. Convention: <base_dir>/keymaps/<spec>.nuon
	if let Some(base) = base_dir {
		let convention_path = base.join("keymaps").join(format!("{spec}.nuon"));
		if convention_path.exists() {
			return load_from_path(&convention_path);
		}
	}

	Err(KeymapPresetError::Invalid(format!("unknown preset: {spec:?}")))
}

// ── NUON field parsers ───────────────────────────────────────────────

#[cfg(feature = "config-nuon")]
fn parse_behavior(value: Option<&xeno_nu_data::Value>) -> Result<KeymapBehavior, KeymapPresetError> {
	let Some(value) = value else {
		return Ok(KeymapBehavior::default());
	};
	let record = value
		.as_record()
		.map_err(|_| KeymapPresetError::Parse("behavior: expected record".to_string()))?;

	Ok(KeymapBehavior {
		vim_shift_letter_casefold: record.get("vim_shift_letter_casefold").and_then(|v| v.as_bool().ok()).unwrap_or(true),
		normal_digit_prefix_count: record.get("normal_digit_prefix_count").and_then(|v| v.as_bool().ok()).unwrap_or(true),
	})
}

#[cfg(feature = "config-nuon")]
fn parse_bindings_list(value: Option<&xeno_nu_data::Value>, source: &str) -> Result<Vec<PresetBinding>, KeymapPresetError> {
	let Some(value) = value else {
		return Ok(Vec::new());
	};
	let list = value
		.as_list()
		.map_err(|_| KeymapPresetError::Parse(format!("{source}: bindings: expected list")))?;

	list.iter()
		.enumerate()
		.map(|(i, item)| {
			let rec = item
				.as_record()
				.map_err(|_| KeymapPresetError::Parse(format!("{source}: bindings[{i}]: expected record")))?;
			let mode = rec
				.get("mode")
				.and_then(|v| v.as_str().ok())
				.ok_or_else(|| KeymapPresetError::Parse(format!("{source}: bindings[{i}]: missing mode")))?
				.to_string();
			let keys = rec
				.get("keys")
				.and_then(|v| v.as_str().ok())
				.ok_or_else(|| KeymapPresetError::Parse(format!("{source}: bindings[{i}]: missing keys")))?;
			let target = rec
				.get("target")
				.and_then(|v| v.as_str().ok())
				.ok_or_else(|| KeymapPresetError::Parse(format!("{source}: bindings[{i}]: missing target")))?
				.to_string();
			Ok(PresetBinding {
				mode,
				keys: Arc::from(keys),
				target,
			})
		})
		.collect()
}

#[cfg(feature = "config-nuon")]
fn parse_prefixes_list(value: Option<&xeno_nu_data::Value>, source: &str) -> Result<Vec<PresetPrefix>, KeymapPresetError> {
	let Some(value) = value else {
		return Ok(Vec::new());
	};
	let list = value
		.as_list()
		.map_err(|_| KeymapPresetError::Parse(format!("{source}: prefixes: expected list")))?;

	list.iter()
		.enumerate()
		.map(|(i, item)| {
			let rec = item
				.as_record()
				.map_err(|_| KeymapPresetError::Parse(format!("{source}: prefixes[{i}]: expected record")))?;
			let mode = rec
				.get("mode")
				.and_then(|v| v.as_str().ok())
				.ok_or_else(|| KeymapPresetError::Parse(format!("{source}: prefixes[{i}]: missing mode")))?
				.to_string();
			let keys = rec
				.get("keys")
				.and_then(|v| v.as_str().ok())
				.ok_or_else(|| KeymapPresetError::Parse(format!("{source}: prefixes[{i}]: missing keys")))?;
			let description = rec
				.get("description")
				.and_then(|v| v.as_str().ok())
				.ok_or_else(|| KeymapPresetError::Parse(format!("{source}: prefixes[{i}]: missing description")))?;
			Ok(PresetPrefix {
				mode,
				keys: Arc::from(keys),
				description: Arc::from(description),
			})
		})
		.collect()
}
