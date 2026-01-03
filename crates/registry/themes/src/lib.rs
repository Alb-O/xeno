//! Theme and syntax highlighting registry
//!
//! This crate provides:
//! - [`Theme`] and [`ThemeColors`] for complete theme definitions
//! - [`SyntaxStyles`] for tree-sitter syntax highlighting
//! - [`THEMES`] distributed slice for compile-time registration
//! - Runtime theme loading via [`register_runtime_themes`]

use std::sync::OnceLock;

use linkme::distributed_slice;
pub use xeno_base::{Color, Mode, Modifier, Style};
use xeno_registry_core::{RegistrySource, impl_registry_metadata};

mod syntax;

pub use syntax::{SyntaxStyle, SyntaxStyles};

/// Runtime theme registry for dynamically loaded themes.
static RUNTIME_THEMES: OnceLock<Vec<&'static Theme>> = OnceLock::new();

/// Whether a theme uses a light or dark background.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum ThemeVariant {
	/// Dark theme with light text on dark background.
	#[default]
	Dark,
	/// Light theme with dark text on light background.
	Light,
}

/// UI color definitions for the editor chrome.
#[derive(Clone, Copy, Debug)]
pub struct UiColors {
	/// Main editor background color.
	pub bg: Color,
	/// Main editor foreground (text) color.
	pub fg: Color,
	/// Line number gutter foreground color.
	pub gutter_fg: Color,
	/// Cursor background color.
	pub cursor_bg: Color,
	/// Cursor foreground (text under cursor) color.
	pub cursor_fg: Color,
	/// Current line highlight background.
	pub cursorline_bg: Color,
	/// Selection background color.
	pub selection_bg: Color,
	/// Selection foreground color.
	pub selection_fg: Color,
	/// Status message foreground color.
	pub message_fg: Color,
	/// Command input line foreground color.
	pub command_input_fg: Color,
}

/// Status line color definitions per mode.
#[derive(Clone, Copy, Debug)]
pub struct StatusColors {
	/// Normal mode background color.
	pub normal_bg: Color,
	/// Normal mode foreground color.
	pub normal_fg: Color,
	/// Insert mode background color.
	pub insert_bg: Color,
	/// Insert mode foreground color.
	pub insert_fg: Color,
	/// Prefix mode colors (Window mode, multi-key sequences).
	pub prefix_mode_bg: Color,
	/// Prefix mode foreground color.
	pub prefix_mode_fg: Color,
	/// Accent colors for UI elements (completion icons, etc.).
	pub accent_bg: Color,
	/// Accent foreground color.
	pub accent_fg: Color,
	/// Command mode background color.
	pub command_bg: Color,
	/// Command mode foreground color.
	pub command_fg: Color,

	/// Dimmed/muted text color.
	pub dim_fg: Color,
	/// Warning message foreground color.
	pub warning_fg: Color,
	/// Error message foreground color.
	pub error_fg: Color,
	/// Success message foreground color.
	pub success_fg: Color,
}

/// Popup/menu color definitions.
#[derive(Clone, Copy, Debug)]
pub struct PopupColors {
	/// Popup background color.
	pub bg: Color,
	/// Popup foreground (text) color.
	pub fg: Color,
	/// Popup border color.
	pub border: Color,
	/// Popup title color.
	pub title: Color,
}

/// Per-semantic-style color pair for notifications.
#[derive(Clone, Copy, Debug, Default)]
pub struct SemanticColorPair {
	/// Background color override (None = inherit from popup).
	pub bg: Option<Color>,
	/// Foreground color override (None = inherit from semantic default).
	pub fg: Option<Color>,
}

impl SemanticColorPair {
	/// No color overrides (fully inherit).
	pub const NONE: Self = Self { bg: None, fg: None };
}

/// Notification-specific color overrides.
#[derive(Clone, Copy, Debug)]
pub struct NotificationColors {
	/// Custom border color (None = use popup border).
	pub border: Option<Color>,
	/// Per-semantic color overrides (e.g., "error" -> custom colors).
	pub overrides: &'static [(&'static str, SemanticColorPair)],
}

impl NotificationColors {
	/// No overrides (inherit all colors from popup/semantic defaults).
	pub const INHERITED: Self = Self {
		border: None,
		overrides: &[],
	};
}

/// Semantic identifier for informational messages.
pub const SEMANTIC_INFO: &str = "info";
/// Semantic identifier for warning messages.
pub const SEMANTIC_WARNING: &str = "warning";
/// Semantic identifier for error messages.
pub const SEMANTIC_ERROR: &str = "error";
/// Semantic identifier for success messages.
pub const SEMANTIC_SUCCESS: &str = "success";
/// Semantic identifier for dimmed/muted content.
pub const SEMANTIC_DIM: &str = "dim";
/// Semantic identifier for normal/default content.
pub const SEMANTIC_NORMAL: &str = "normal";

/// Complete theme color palette.
#[derive(Clone, Copy, Debug)]
pub struct ThemeColors {
	/// Core editor UI colors.
	pub ui: UiColors,
	/// Status line colors per mode.
	pub status: StatusColors,
	/// Popup/menu colors.
	pub popup: PopupColors,
	/// Notification color overrides.
	pub notification: NotificationColors,
	/// Syntax highlighting styles.
	pub syntax: SyntaxStyles,
}

impl ThemeColors {
	/// Get the style for a given editor mode (for status line mode indicator).
	#[inline]
	pub fn mode_style(&self, mode: &Mode) -> Style {
		let s = &self.status;
		match mode {
			Mode::Normal => Style::new().bg(s.normal_bg).fg(s.normal_fg),
			Mode::Insert => Style::new().bg(s.insert_bg).fg(s.insert_fg),
			Mode::PendingAction(_) => Style::new().bg(s.command_bg).fg(s.command_fg),
		}
	}

	/// Resolve notification style for a given semantic identifier.
	pub fn notification_style(&self, semantic: &str) -> Style {
		let override_pair = self
			.notification
			.overrides
			.iter()
			.find(|(id, _)| *id == semantic)
			.map(|(_, pair)| pair);

		let bg = override_pair.and_then(|p| p.bg).unwrap_or(self.popup.bg);

		let fg = override_pair.and_then(|p| p.fg).unwrap_or({
			match semantic {
				SEMANTIC_WARNING => self.status.warning_fg,
				SEMANTIC_ERROR => self.status.error_fg,
				SEMANTIC_SUCCESS => self.status.success_fg,
				SEMANTIC_DIM => self.status.dim_fg,
				_ => self.popup.fg,
			}
		});

		Style::new().bg(bg).fg(fg)
	}

	/// Resolve notification border color.
	pub fn notification_border(&self) -> Color {
		self.notification.border.unwrap_or(self.popup.border)
	}
}

/// A complete theme definition.
#[derive(Clone, Copy, Debug)]
pub struct Theme {
	/// Unique identifier for the theme.
	pub id: &'static str,
	/// Human-readable display name.
	pub name: &'static str,
	/// Alternative names for theme lookup.
	pub aliases: &'static [&'static str],
	/// Whether this is a light or dark theme.
	pub variant: ThemeVariant,
	/// Complete color definitions.
	pub colors: ThemeColors,
	/// Sort priority (higher = listed first).
	pub priority: i16,
	/// Where this theme was registered from.
	pub source: RegistrySource,
}

/// Owned theme data for runtime-loaded themes.
#[derive(Clone, Debug)]
pub struct OwnedTheme {
	/// Unique identifier for the theme.
	pub id: String,
	/// Human-readable display name.
	pub name: String,
	/// Alternative names for theme lookup.
	pub aliases: Vec<String>,
	/// Whether this is a light or dark theme.
	pub variant: ThemeVariant,
	/// Complete color definitions.
	pub colors: ThemeColors,
	/// Sort priority (higher = listed first).
	pub priority: i16,
	/// Where this theme was registered from.
	pub source: RegistrySource,
}

impl OwnedTheme {
	/// Leaks this owned theme to produce a 'static Theme reference.
	pub fn leak(self) -> &'static Theme {
		let id: &'static str = Box::leak(self.id.into_boxed_str());
		let name: &'static str = Box::leak(self.name.into_boxed_str());
		let aliases: &'static [&'static str] = Box::leak(
			self.aliases
				.into_iter()
				.map(|s| -> &'static str { Box::leak(s.into_boxed_str()) })
				.collect::<Vec<_>>()
				.into_boxed_slice(),
		);

		Box::leak(Box::new(Theme {
			id,
			name,
			aliases,
			variant: self.variant,
			colors: self.colors,
			priority: self.priority,
			source: self.source,
		}))
	}
}

/// Register runtime themes. Call once at startup with themes from KDL files.
pub fn register_runtime_themes(themes: Vec<OwnedTheme>) {
	let leaked: Vec<&'static Theme> = themes.into_iter().map(OwnedTheme::leak).collect();
	let _ = RUNTIME_THEMES.set(leaked);
}

/// Get all registered runtime themes.
pub fn runtime_themes() -> &'static [&'static Theme] {
	RUNTIME_THEMES.get().map(|v| v.as_slice()).unwrap_or(&[])
}

/// Distributed slice for compile-time theme registration.
#[distributed_slice]
pub static THEMES: [Theme] = [..];

/// Default fallback theme (minimal terminal colors).
#[distributed_slice(THEMES)]
pub static DEFAULT_THEME: Theme = Theme {
	id: "default",
	name: "default",
	aliases: &[],
	variant: ThemeVariant::Dark,
	colors: ThemeColors {
		ui: UiColors {
			bg: Color::Reset,
			fg: Color::Reset,
			gutter_fg: Color::DarkGray,
			cursor_bg: Color::White,
			cursor_fg: Color::Black,
			cursorline_bg: Color::DarkGray,
			selection_bg: Color::Blue,
			selection_fg: Color::White,
			message_fg: Color::Yellow,
			command_input_fg: Color::White,
		},
		status: StatusColors {
			normal_bg: Color::Blue,
			normal_fg: Color::White,
			insert_bg: Color::Green,
			insert_fg: Color::Black,
			prefix_mode_bg: Color::Magenta,
			prefix_mode_fg: Color::White,
			accent_bg: Color::Cyan,
			accent_fg: Color::Black,
			command_bg: Color::Yellow,
			command_fg: Color::Black,
			dim_fg: Color::DarkGray,
			warning_fg: Color::Yellow,
			error_fg: Color::Red,
			success_fg: Color::Green,
		},
		popup: PopupColors {
			bg: Color::Rgb(10, 10, 10),
			fg: Color::White,
			border: Color::White,
			title: Color::Yellow,
		},
		notification: NotificationColors::INHERITED,
		syntax: SyntaxStyles::minimal(),
	},
	priority: 0,
	source: RegistrySource::Builtin,
};

/// Default theme ID to use when no theme is specified.
pub const DEFAULT_THEME_ID: &str = "gruvbox";

/// Find a theme by name or alias.
pub fn get_theme(name: &str) -> Option<&'static Theme> {
	let normalize = |s: &str| -> String {
		s.chars()
			.filter(|c| *c != '-' && *c != '_')
			.collect::<String>()
			.to_lowercase()
	};

	let search = normalize(name);

	// Check runtime themes first (from KDL files)
	if let Some(theme) = runtime_themes()
		.iter()
		.find(|t| normalize(t.name) == search || t.aliases.iter().any(|a| normalize(a) == search))
	{
		return Some(theme);
	}

	// Fall back to compile-time themes
	THEMES
		.iter()
		.find(|t| normalize(t.name) == search || t.aliases.iter().any(|a| normalize(a) == search))
}

/// Blend two colors with the given alpha (0.0 = bg, 1.0 = fg).
#[inline]
pub fn blend_colors(fg: Color, bg: Color, alpha: f32) -> Color {
	fg.blend(bg, alpha)
}

/// Suggest a similar theme name using fuzzy matching.
pub fn suggest_theme(name: &str) -> Option<&'static str> {
	let name = name.to_lowercase();
	let mut best_match = None;
	let mut best_score = 0.0;

	for theme in runtime_themes() {
		let score = strsim::jaro_winkler(&name, theme.name);
		if score > best_score {
			best_score = score;
			best_match = Some(theme.name);
		}

		for alias in theme.aliases {
			let score = strsim::jaro_winkler(&name, alias);
			if score > best_score {
				best_score = score;
				best_match = Some(theme.name);
			}
		}
	}

	for theme in THEMES {
		let score = strsim::jaro_winkler(&name, theme.name);
		if score > best_score {
			best_score = score;
			best_match = Some(theme.name);
		}

		for alias in theme.aliases {
			let score = strsim::jaro_winkler(&name, alias);
			if score > best_score {
				best_score = score;
				best_match = Some(theme.name);
			}
		}
	}

	if best_score > 0.8 { best_match } else { None }
}

impl_registry_metadata!(Theme);
