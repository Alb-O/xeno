use xeno_primitives::Color;

use super::super::syntax::SyntaxStyles;
use super::types::{
	ColorPair, ModeColors, NotificationColors, OwnedTheme, PopupColors, SemanticColors,
	ThemeColors, ThemeDef, ThemeVariant, UiColors,
};

/// Register runtime themes into the [`THEMES`] registry.
///
/// Leaks each [`OwnedTheme`] to obtain `&'static ThemeDef` references, then
/// batch-inserts them with override semantics so later calls can shadow
/// earlier ones by ID. May be called multiple times.
#[cfg(feature = "db")]
pub fn register_runtime_themes(themes: Vec<OwnedTheme>) {
	let leaked: Vec<&'static ThemeDef> = themes.into_iter().map(OwnedTheme::leak).collect();
	if let Err(e) = THEMES.try_register_many_override(leaked) {
		tracing::warn!(error = %e, "failed to register runtime themes");
	}
}

/// Default fallback theme (minimal terminal colors).
pub static DEFAULT_THEME: ThemeDef = ThemeDef {
	meta: crate::core::RegistryMeta {
		id: "default",
		name: "default",
		aliases: &[],
		description: "",
		priority: 0,
		source: crate::core::RegistrySource::Builtin,
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

use crate::RegistryEntry;
#[cfg(feature = "db")]
pub use crate::db::THEMES;

/// Find a theme by name or alias.
pub fn get_theme(name: &str) -> Option<crate::core::RegistryRef<ThemeDef>> {
	let normalize = |s: &str| -> String {
		s.chars()
			.filter(|c| *c != '-' && *c != '_')
			.collect::<String>()
			.to_lowercase()
	};

	let search = normalize(name);

	#[cfg(feature = "db")]
	{
		if let Some(theme) = THEMES.get(name) {
			return Some(theme);
		}

		if let Some(theme) = THEMES.iter().into_iter().find(|t| {
			normalize(t.name()) == search || t.aliases().iter().any(|a| normalize(a) == search)
		}) {
			return Some(theme);
		}
	}

	None
}
