//! Theme schema types and registry.
//!
//! This module defines the type schema for editor themes. Theme definitions
//! come from KDL files in `runtime/themes/`. A fallback default theme exists
//! only for cases where no themes are loaded.

use std::sync::OnceLock;

pub use evildoer_base::color::{Color, Modifier};
use linkme::distributed_slice;

pub use crate::syntax::{SyntaxStyle, SyntaxStyles};

/// Runtime theme registry for dynamically loaded themes.
/// Themes are leaked to obtain 'static lifetime for consistency with the rest of the codebase.
static RUNTIME_THEMES: OnceLock<Vec<&'static Theme>> = OnceLock::new();

/// Whether a theme uses a light or dark background.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum ThemeVariant {
	#[default]
	Dark,
	Light,
}

/// UI color definitions for the editor chrome.
#[derive(Clone, Copy, Debug)]
pub struct UiColors {
	pub bg: Color,
	pub fg: Color,
	pub gutter_fg: Color,
	pub cursor_bg: Color,
	pub cursor_fg: Color,
	pub cursorline_bg: Color,
	pub selection_bg: Color,
	pub selection_fg: Color,
	pub message_fg: Color,
	pub command_input_fg: Color,
	/// Color for indentation guide overlays (spaces shown as dots, tabs as chevrons).
	/// If None, inherits from gutter_fg with reduced opacity.
	pub indent_guide_fg: Option<Color>,
}

/// Characters used to render indentation guides.
#[derive(Clone, Copy, Debug)]
pub struct IndentGuideChars {
	/// Character for space indentation (default: middle dot '·' U+00B7).
	pub space: char,
	/// Character for tab indentation (default: right chevron '›' U+203A).
	pub tab: char,
}

impl Default for IndentGuideChars {
	fn default() -> Self {
		Self {
			// Middle dot - subtle, vertically centered
			space: '\u{00B7}',
			// Single right-pointing angle quotation mark - clean chevron look
			tab: '\u{203A}',
		}
	}
}

/// Status line color definitions per mode.
#[derive(Clone, Copy, Debug)]
pub struct StatusColors {
	pub normal_bg: Color,
	pub normal_fg: Color,
	pub insert_bg: Color,
	pub insert_fg: Color,
	pub goto_bg: Color,
	pub goto_fg: Color,
	pub view_bg: Color,
	pub view_fg: Color,
	pub command_bg: Color,
	pub command_fg: Color,

	pub dim_fg: Color,
	pub warning_fg: Color,
	pub error_fg: Color,
	pub success_fg: Color,
}

/// Popup/menu color definitions.
#[derive(Clone, Copy, Debug)]
pub struct PopupColors {
	pub bg: Color,
	pub fg: Color,
	pub border: Color,
	pub title: Color,
}

/// Per-semantic-style color pair for notifications.
/// If None, inherits from the base theme colors.
#[derive(Clone, Copy, Debug, Default)]
pub struct SemanticColorPair {
	pub bg: Option<Color>,
	pub fg: Option<Color>,
}

impl SemanticColorPair {
	/// Const default with no overrides (inherit all).
	pub const NONE: Self = Self { bg: None, fg: None };
}

/// Notification-specific color overrides.
/// Uses a flat list of semantic identifiers mapped to color pairs.
#[derive(Clone, Copy, Debug)]
pub struct NotificationColors {
	/// Border color override (inherits from popup.border if None)
	pub border: Option<Color>,
	/// Map of semantic identifiers to color pairs.
	/// In static contexts, we use a fixed-size array for simplicity.
	pub overrides: &'static [(&'static str, SemanticColorPair)],
}

impl NotificationColors {
	/// Const default with no overrides (inherit all colors from popup/status).
	pub const INHERITED: Self = Self {
		border: None,
		overrides: &[],
	};
}

/// Complete theme color palette.
#[derive(Clone, Copy, Debug)]
pub struct ThemeColors {
	pub ui: UiColors,
	pub status: StatusColors,
	pub popup: PopupColors,
	/// Notification-specific color overrides (optional, inherits from popup/status)
	pub notification: NotificationColors,
	/// Syntax highlighting styles for tree-sitter captures
	pub syntax: SyntaxStyles,
}

impl ThemeColors {
	/// Resolve notification style for a given semantic identifier.
	/// Uses notification-specific overrides if set, otherwise inherits from popup/status colors.
	pub fn notification_style(&self, semantic: &str) -> evildoer_base::Style {
		let override_pair = self
			.notification
			.overrides
			.iter()
			.find(|(id, _)| *id == semantic)
			.map(|(_, pair)| pair);

		let bg = override_pair.and_then(|p| p.bg).unwrap_or(self.popup.bg);

		let fg = override_pair.and_then(|p| p.fg).unwrap_or_else(|| {
			use crate::*;
			match semantic {
				SEMANTIC_WARNING => self.status.warning_fg,
				SEMANTIC_ERROR => self.status.error_fg,
				SEMANTIC_SUCCESS => self.status.success_fg,
				SEMANTIC_DIM => self.status.dim_fg,
				_ => self.popup.fg,
			}
		});

		evildoer_base::Style::new().bg(bg).fg(fg)
	}

	/// Resolve notification border color.
	pub fn notification_border(&self) -> Color {
		self.notification.border.unwrap_or(self.popup.border)
	}
}

/// A complete theme definition.
#[derive(Clone, Copy, Debug)]
pub struct Theme {
	pub id: &'static str,
	pub name: &'static str,
	pub aliases: &'static [&'static str],
	pub variant: ThemeVariant,
	pub colors: ThemeColors,
	pub priority: i16,
	pub source: crate::RegistrySource,
}

/// Owned theme data for runtime-loaded themes.
/// This is converted to a leaked `Theme` for 'static lifetime.
#[derive(Clone, Debug)]
pub struct OwnedTheme {
	pub id: String,
	pub name: String,
	pub aliases: Vec<String>,
	pub variant: ThemeVariant,
	pub colors: ThemeColors,
	pub priority: i16,
	pub source: crate::RegistrySource,
}

impl OwnedTheme {
	/// Leaks this owned theme to produce a 'static Theme reference.
	/// The memory is intentionally leaked to provide stable 'static references.
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

/// Register runtime themes. This should be called once at startup with all
/// themes loaded from KDL files. Subsequent calls will be ignored.
pub fn register_runtime_themes(themes: Vec<OwnedTheme>) {
	let leaked: Vec<&'static Theme> = themes.into_iter().map(OwnedTheme::leak).collect();
	let _ = RUNTIME_THEMES.set(leaked);
}

/// Get all registered runtime themes.
pub fn runtime_themes() -> &'static [&'static Theme] {
	RUNTIME_THEMES.get().map(|v| v.as_slice()).unwrap_or(&[])
}

#[distributed_slice]
pub static THEMES: [Theme] = [..];

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
			indent_guide_fg: None,
		},
		status: StatusColors {
			normal_bg: Color::Blue,
			normal_fg: Color::White,
			insert_bg: Color::Green,
			insert_fg: Color::Black,
			goto_bg: Color::Magenta,
			goto_fg: Color::White,
			view_bg: Color::Cyan,
			view_fg: Color::Black,
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
	source: crate::RegistrySource::Builtin,
};

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

	// Fall back to compile-time themes (just the default fallback)
	THEMES
		.iter()
		.find(|t| normalize(t.name) == search || t.aliases.iter().any(|a| normalize(a) == search))
}

/// Blend two colors with the given alpha (0.0 = bg, 1.0 = fg).
///
/// This is a convenience wrapper around `Color::blend`.
#[inline]
pub fn blend_colors(fg: Color, bg: Color, alpha: f32) -> Color {
	fg.blend(bg, alpha)
}

pub fn suggest_theme(name: &str) -> Option<&'static str> {
	let name = name.to_lowercase();
	let mut best_match = None;
	let mut best_score = 0.0;

	// Check runtime themes first
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

	// Then check compile-time themes
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

pub const DEFAULT_THEME_ID: &str = "gruvbox";

use crate::completion::{
	CompletionContext, CompletionItem, CompletionKind, CompletionResult, CompletionSource,
	PROMPT_COMMAND,
};

/// Completion source for theme names.
/// Provides completions when typing `:theme <arg>` or `:colorscheme <arg>`.
pub struct ThemeSource;

impl CompletionSource for ThemeSource {
	fn complete(&self, ctx: &CompletionContext) -> CompletionResult {
		if ctx.prompt != PROMPT_COMMAND {
			return CompletionResult::empty();
		}

		let parts: Vec<&str> = ctx.input.split_whitespace().collect();
		let is_theme_cmd = matches!(parts.first(), Some(&"theme") | Some(&"colorscheme"));

		if !is_theme_cmd {
			return CompletionResult::empty();
		}

		let prefix = parts.get(1).copied().unwrap_or("");

		if parts.len() == 1 && !ctx.input.ends_with(' ') {
			return CompletionResult::empty();
		}

		let cmd_name = parts.first().unwrap();
		let arg_start = cmd_name.len() + 1;

		// Collect from both runtime and compile-time themes
		let mut items: Vec<_> = runtime_themes()
			.iter()
			.copied()
			.chain(THEMES.iter())
			.filter(|theme| {
				theme.name.starts_with(prefix)
					|| theme.aliases.iter().any(|a| a.starts_with(prefix))
			})
			.map(|theme| {
				let variant_str = match theme.variant {
					ThemeVariant::Dark => "dark",
					ThemeVariant::Light => "light",
				};
				CompletionItem {
					label: theme.name.to_string(),
					insert_text: theme.name.to_string(),
					detail: Some(format!("{} theme", variant_str)),
					filter_text: None,
					kind: CompletionKind::Theme,
				}
			})
			.collect();

		// Deduplicate by label (runtime themes take precedence)
		items.dedup_by(|a, b| a.label == b.label);

		CompletionResult::new(arg_start, items)
	}
}
