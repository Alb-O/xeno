//! Theme bootstrap cache for instant first-frame background color.
//!
//! Caches essential UI colors to disk so the editor can display the user's
//! theme background immediately on startup, avoiding the flash that occurs
//! when the terminal background differs from the editor theme.
//!
//! Call [`init`] early in startup (before creating the Editor) to load
//! cached theme colors. The bootstrap theme is then automatically used
//! by [`Config::new`] for the first frame.
//!
//! [`Config::new`]: crate::types::config::Config::new

use std::fs;
use std::path::PathBuf;
use std::sync::OnceLock;

use serde::{Deserialize, Serialize};
use xeno_primitives::Color;
use xeno_registry::themes::{Theme, ThemeVariant};

static BOOTSTRAP_THEME: OnceLock<&'static Theme> = OnceLock::new();

const MAGIC: &[u8; 8] = b"XENOBOOT";
const SCHEMA_VERSION: u32 = 1;
const CACHE_FILE: &str = "theme_bootstrap.bin";

/// Minimal color data for first-frame rendering.
///
/// Cache format: 8-byte magic (`XENOBOOT`) + 4-byte LE version + bincode payload.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BootstrapColors {
	pub theme_id: String,
	pub variant: BootstrapVariant,
	pub ui_bg: SerializableColor,
	pub ui_fg: SerializableColor,
	pub nontext_bg: SerializableColor,
	pub popup_bg: SerializableColor,
	pub popup_fg: SerializableColor,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum BootstrapVariant {
	Dark,
	Light,
}

impl From<ThemeVariant> for BootstrapVariant {
	fn from(v: ThemeVariant) -> Self {
		match v {
			ThemeVariant::Dark => Self::Dark,
			ThemeVariant::Light => Self::Light,
		}
	}
}

impl From<BootstrapVariant> for ThemeVariant {
	fn from(v: BootstrapVariant) -> Self {
		match v {
			BootstrapVariant::Dark => Self::Dark,
			BootstrapVariant::Light => Self::Light,
		}
	}
}

/// Serializable mirror of [`Color`] for bincode.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum SerializableColor {
	Reset,
	Black,
	Red,
	Green,
	Yellow,
	Blue,
	Magenta,
	Cyan,
	Gray,
	DarkGray,
	LightRed,
	LightGreen,
	LightYellow,
	LightBlue,
	LightMagenta,
	LightCyan,
	White,
	Rgb(u8, u8, u8),
	Indexed(u8),
}

impl From<Color> for SerializableColor {
	fn from(c: Color) -> Self {
		match c {
			Color::Reset => Self::Reset,
			Color::Black => Self::Black,
			Color::Red => Self::Red,
			Color::Green => Self::Green,
			Color::Yellow => Self::Yellow,
			Color::Blue => Self::Blue,
			Color::Magenta => Self::Magenta,
			Color::Cyan => Self::Cyan,
			Color::Gray => Self::Gray,
			Color::DarkGray => Self::DarkGray,
			Color::LightRed => Self::LightRed,
			Color::LightGreen => Self::LightGreen,
			Color::LightYellow => Self::LightYellow,
			Color::LightBlue => Self::LightBlue,
			Color::LightMagenta => Self::LightMagenta,
			Color::LightCyan => Self::LightCyan,
			Color::White => Self::White,
			Color::Rgb(r, g, b) => Self::Rgb(r, g, b),
			Color::Indexed(i) => Self::Indexed(i),
		}
	}
}

impl From<SerializableColor> for Color {
	fn from(c: SerializableColor) -> Self {
		match c {
			SerializableColor::Reset => Self::Reset,
			SerializableColor::Black => Self::Black,
			SerializableColor::Red => Self::Red,
			SerializableColor::Green => Self::Green,
			SerializableColor::Yellow => Self::Yellow,
			SerializableColor::Blue => Self::Blue,
			SerializableColor::Magenta => Self::Magenta,
			SerializableColor::Cyan => Self::Cyan,
			SerializableColor::Gray => Self::Gray,
			SerializableColor::DarkGray => Self::DarkGray,
			SerializableColor::LightRed => Self::LightRed,
			SerializableColor::LightGreen => Self::LightGreen,
			SerializableColor::LightYellow => Self::LightYellow,
			SerializableColor::LightBlue => Self::LightBlue,
			SerializableColor::LightMagenta => Self::LightMagenta,
			SerializableColor::LightCyan => Self::LightCyan,
			SerializableColor::White => Self::White,
			SerializableColor::Rgb(r, g, b) => Self::Rgb(r, g, b),
			SerializableColor::Indexed(i) => Self::Indexed(i),
		}
	}
}

impl BootstrapColors {
	pub fn from_theme(theme: &Theme) -> Self {
		Self {
			theme_id: theme.meta.id.to_string(),
			variant: theme.variant.into(),
			ui_bg: theme.colors.ui.bg.into(),
			ui_fg: theme.colors.ui.fg.into(),
			nontext_bg: theme.colors.ui.nontext_bg.into(),
			popup_bg: theme.colors.popup.bg.into(),
			popup_fg: theme.colors.popup.fg.into(),
		}
	}
}

fn cache_path() -> Option<PathBuf> {
	crate::paths::get_cache_dir().map(|d| d.join(CACHE_FILE))
}

fn load_bootstrap_cache() -> Option<BootstrapColors> {
	let data = fs::read(cache_path()?).ok()?;
	if data.len() < 12 || &data[0..8] != MAGIC {
		return None;
	}
	let version = u32::from_le_bytes(data[8..12].try_into().ok()?);
	if version != SCHEMA_VERSION {
		return None;
	}
	bincode::deserialize(&data[12..]).ok()
}

fn write_bootstrap_cache(colors: &BootstrapColors) {
	let Some(path) = cache_path() else { return };
	let Ok(payload) = bincode::serialize(colors) else {
		return;
	};

	if let Some(parent) = path.parent() {
		let _ = fs::create_dir_all(parent);
	}

	let mut data = Vec::with_capacity(12 + payload.len());
	data.extend_from_slice(MAGIC);
	data.extend_from_slice(&SCHEMA_VERSION.to_le_bytes());
	data.extend_from_slice(&payload);
	let _ = fs::write(path, data);
}

/// Loads bootstrap theme from cache. Call before creating [`Editor`].
///
/// [`Editor`]: crate::Editor
pub fn init() {
	if let Some(colors) = load_bootstrap_cache() {
		let _ = BOOTSTRAP_THEME.set(create_bootstrap_theme(&colors));
	}
}

/// Returns the bootstrap theme if loaded from cache.
pub fn get() -> Option<&'static Theme> {
	BOOTSTRAP_THEME.get().copied()
}

/// Caches theme colors for next startup. Call after theme resolution.
pub fn cache_theme(theme: &Theme) {
	write_bootstrap_cache(&BootstrapColors::from_theme(theme));
}

fn create_bootstrap_theme(colors: &BootstrapColors) -> &'static Theme {
	use xeno_registry::themes::{
		ColorPair, ModeColors, NotificationColors, PopupColors, SemanticColors, SyntaxStyles,
		ThemeColors, UiColors,
	};
	use xeno_registry_core::{RegistryMeta, RegistrySource};

	Box::leak(Box::new(Theme {
		meta: RegistryMeta {
			id: "bootstrap",
			name: "bootstrap",
			aliases: &[],
			description: "",
			priority: -1000,
			source: RegistrySource::Builtin,
			required_caps: &[],
			flags: 0,
		},
		variant: colors.variant.into(),
		colors: ThemeColors {
			ui: UiColors {
				bg: colors.ui_bg.into(),
				fg: colors.ui_fg.into(),
				nontext_bg: colors.nontext_bg.into(),
				gutter_fg: Color::DarkGray,
				cursor_bg: Color::White,
				cursor_fg: Color::Black,
				cursorline_bg: Color::DarkGray,
				selection_bg: Color::Blue,
				selection_fg: Color::White,
				message_fg: Color::Yellow,
				command_input_fg: Color::White,
			},
			mode: ModeColors {
				normal: ColorPair::new(Color::Blue, Color::White),
				insert: ColorPair::new(Color::Green, Color::Black),
				prefix: ColorPair::new(Color::Magenta, Color::White),
				command: ColorPair::new(Color::Yellow, Color::Black),
			},
			semantic: SemanticColors {
				error: Color::Red,
				warning: Color::Yellow,
				success: Color::Green,
				info: Color::Cyan,
				hint: Color::DarkGray,
				dim: Color::DarkGray,
				link: Color::Cyan,
				match_hl: Color::Green,
				accent: Color::Cyan,
			},
			popup: PopupColors {
				bg: colors.popup_bg.into(),
				fg: colors.popup_fg.into(),
				border: Color::White,
				title: Color::Yellow,
			},
			notification: NotificationColors::INHERITED,
			syntax: SyntaxStyles::minimal(),
		},
	}))
}
