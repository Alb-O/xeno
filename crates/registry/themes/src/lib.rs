//! Theme and syntax highlighting registry
//!
//! This crate provides:
//! - [`Theme`] and [`ThemeColors`] for complete theme definitions
//! - [`SyntaxStyles`] for tree-sitter syntax highlighting
//! - [`THEMES`] registry for compile-time registration
//! - Runtime theme loading via [`register_runtime_themes`]

use std::sync::{LazyLock, OnceLock};

pub use xeno_primitives::{Color, Mode, Modifier, Style};
use xeno_registry_core::{
	RegistryBuilder, RegistryIndex, RegistryMeta, RegistryReg, RegistrySource, impl_registry_entry,
};

mod syntax;

pub use syntax::{SyntaxStyle, SyntaxStyles};

/// Registry wrapper for theme definitions.
pub struct ThemeReg(pub &'static Theme);
inventory::collect!(ThemeReg);

impl RegistryReg<Theme> for ThemeReg {
	fn def(&self) -> &'static Theme {
		self.0
	}
}

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

/// A background/foreground color pair for UI elements like mode badges.
#[derive(Clone, Copy, Debug)]
pub struct ColorPair {
	/// Background color.
	pub bg: Color,
	/// Foreground (text) color.
	pub fg: Color,
}

impl ColorPair {
	/// Creates a new color pair.
	pub const fn new(bg: Color, fg: Color) -> Self {
		Self { bg, fg }
	}

	/// Converts to a [`Style`] with both bg and fg set.
	pub fn to_style(self) -> Style {
		Style::new().bg(self.bg).fg(self.fg)
	}
}

/// Mode indicator colors for status bar badges.
#[derive(Clone, Copy, Debug)]
pub struct ModeColors {
	/// Normal mode colors.
	pub normal: ColorPair,
	/// Insert mode colors.
	pub insert: ColorPair,
	/// Prefix/pending mode colors (window mode, multi-key sequences).
	pub prefix: ColorPair,
	/// Command mode colors.
	pub command: ColorPair,
}

impl ModeColors {
	/// Returns the color pair for a given editor mode.
	pub fn for_mode(&self, mode: &Mode) -> ColorPair {
		match mode {
			Mode::Normal => self.normal,
			Mode::Insert => self.insert,
			Mode::PendingAction(_) => self.command,
		}
	}
}

/// Semantic colors used throughout the UI.
///
/// These are single colors (not pairs) intended for use as foreground colors
/// to convey meaning: errors, warnings, matches, links, etc.
#[derive(Clone, Copy, Debug)]
pub struct SemanticColors {
	/// Error messages and indicators.
	pub error: Color,
	/// Warning messages and indicators.
	pub warning: Color,
	/// Success messages and indicators.
	pub success: Color,
	/// Informational messages.
	pub info: Color,
	/// Hints and subtle information.
	pub hint: Color,
	/// Dimmed/muted text.
	pub dim: Color,
	/// Links and interactive elements.
	pub link: Color,
	/// Search/filter match highlighting.
	pub match_hl: Color,
	/// General accent color for emphasis.
	pub accent: Color,
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
	/// Mode indicator colors for status bar.
	pub mode: ModeColors,
	/// Semantic colors for messages, highlights, etc.
	pub semantic: SemanticColors,
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
		self.mode.for_mode(mode).to_style()
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
				SEMANTIC_WARNING => self.semantic.warning,
				SEMANTIC_ERROR => self.semantic.error,
				SEMANTIC_SUCCESS => self.semantic.success,
				SEMANTIC_DIM => self.semantic.dim,
				SEMANTIC_INFO => self.semantic.info,
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
	/// Common registry metadata.
	pub meta: RegistryMeta,
	/// Whether this is a light or dark theme.
	pub variant: ThemeVariant,
	/// Complete color definitions.
	pub colors: ThemeColors,
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
			meta: RegistryMeta {
				id,
				name,
				aliases,
				description: "",
				priority: self.priority,
				source: self.source,
				required_caps: &[],
				flags: 0,
			},
			variant: self.variant,
			colors: self.colors,
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

/// Indexed collection of all compile-time themes.
pub static THEMES: LazyLock<RegistryIndex<Theme>> = LazyLock::new(|| {
	RegistryBuilder::new("themes")
		.extend_inventory::<ThemeReg>()
		.sort_by(|a, b| a.meta.priority.cmp(&b.meta.priority))
		.build()
});

inventory::submit! { ThemeReg(&DEFAULT_THEME) }

/// Default fallback theme (minimal terminal colors).
pub static DEFAULT_THEME: Theme = Theme {
	meta: RegistryMeta {
		id: "default",
		name: "default",
		aliases: &[],
		description: "",
		priority: 0,
		source: RegistrySource::Builtin,
		required_caps: &[],
		flags: 0,
	},
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
			bg: Color::Rgb(10, 10, 10),
			fg: Color::White,
			border: Color::White,
			title: Color::Yellow,
		},
		notification: NotificationColors::INHERITED,
		syntax: SyntaxStyles::minimal(),
	},
};

/// Default theme ID to use when no theme is specified.
pub const DEFAULT_THEME_ID: &str = "gruvbox";

/// Find a theme by name or alias.
///
/// Resolution order: runtime themes (KDL) → exact compile-time match → normalized search.
/// Normalization strips hyphens/underscores and lowercases for fuzzy matching.
pub fn get_theme(name: &str) -> Option<&'static Theme> {
	let normalize = |s: &str| -> String {
		s.chars()
			.filter(|c| *c != '-' && *c != '_')
			.collect::<String>()
			.to_lowercase()
	};

	let search = normalize(name);

	if let Some(theme) = runtime_themes().iter().find(|t| {
		normalize(t.meta.name) == search || t.meta.aliases.iter().any(|a| normalize(a) == search)
	}) {
		return Some(theme);
	}

	if let Some(theme) = THEMES.get(name) {
		return Some(theme);
	}

	THEMES.iter().find(|t| {
		normalize(t.meta.name) == search || t.meta.aliases.iter().any(|a| normalize(a) == search)
	})
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
		let score = strsim::jaro_winkler(&name, theme.meta.name);
		if score > best_score {
			best_score = score;
			best_match = Some(theme.meta.name);
		}

		for alias in theme.meta.aliases {
			let score = strsim::jaro_winkler(&name, alias);
			if score > best_score {
				best_score = score;
				best_match = Some(theme.meta.name);
			}
		}
	}

	for theme in THEMES.iter() {
		let score = strsim::jaro_winkler(&name, theme.meta.name);
		if score > best_score {
			best_score = score;
			best_match = Some(theme.meta.name);
		}

		for alias in theme.meta.aliases {
			let score = strsim::jaro_winkler(&name, alias);
			if score > best_score {
				best_score = score;
				best_match = Some(theme.meta.name);
			}
		}
	}

	if best_score > 0.8 { best_match } else { None }
}

impl_registry_entry!(Theme);
