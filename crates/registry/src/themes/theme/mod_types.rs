use std::sync::OnceLock;

use xeno_primitives::Color;

use super::super::syntax::SyntaxStyles;
use super::types::{
	ColorPair, ModeColors, NotificationColors, OwnedTheme, PopupColors, SemanticColors,
	ThemeColors, ThemeDef, ThemeVariant, UiColors,
};

/// Runtime theme registry for dynamically loaded themes.
static RUNTIME_THEMES: OnceLock<Vec<&'static ThemeDef>> = OnceLock::new();

/// Register runtime themes. Call once at startup with themes from KDL files.
pub fn register_runtime_themes(themes: Vec<OwnedTheme>) {
	let leaked: Vec<&'static ThemeDef> = themes.into_iter().map(OwnedTheme::leak).collect();
	let _ = RUNTIME_THEMES.set(leaked);
}

/// Get all registered runtime themes.
pub fn runtime_themes() -> &'static [&'static ThemeDef] {
	RUNTIME_THEMES.get().map(|v| v.as_slice()).unwrap_or(&[])
}

/// Default fallback theme (minimal terminal colors).
pub static DEFAULT_THEME: ThemeDef = ThemeDef {
	meta: xeno_registry_core::RegistryMeta {
		id: "default",
		name: "default",
		aliases: &[],
		description: "",
		priority: 0,
		source: xeno_registry_core::RegistrySource::Builtin,
		required_caps: &[],
		flags: 0,
	},
	variant: ThemeVariant::Dark,
	colors: ThemeColors {
		ui: UiColors {
			bg: Color::Reset,
			fg: Color::Reset,
			nontext_bg: Color::Rgb(5, 5, 5),
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

#[cfg(feature = "db")]
pub use crate::db::THEMES;

/// Find a theme by name or alias.
pub fn get_theme(name: &str) -> Option<&'static ThemeDef> {
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

	#[cfg(feature = "db")]
	{
		if let Some(theme) = THEMES.get(name) {
			return Some(theme);
		}

		THEMES.iter().find(|t| {
			normalize(t.meta.name) == search
				|| t.meta.aliases.iter().any(|a| normalize(a) == search)
		})
	}
	#[cfg(not(feature = "db"))]
	{
		None
	}
}
