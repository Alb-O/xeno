use xeno_primitives::Color;

use super::super::syntax::SyntaxStyles;
use super::types::{ColorPair, ModeColors, NotificationColors, PopupColors, SemanticColors, ThemeColors, ThemeDef, ThemeEntry, UiColors};
use crate::core::{RegistryMetaStatic, RegistrySource};

/// Default fallback theme (minimal terminal colors).
pub static DEFAULT_THEME: ThemeDef = ThemeDef {
	meta: RegistryMetaStatic {
		id: "default",
		name: "default",
		keys: &[],
		description: "",
		priority: 0,
		source: RegistrySource::Builtin,
		mutates_buffer: false,
		flags: 0,
	},
	variant: crate::themes::ThemeVariant::Dark,
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
pub const DEFAULT_THEME_ID: &str = "xeno-registry::monokai";

#[cfg(feature = "minimal")]
pub use crate::db::THEMES;

/// Find a theme by name or key.
pub fn get_theme(name: &str) -> Option<crate::core::RegistryRef<ThemeEntry, crate::core::ThemeId>> {
	#[cfg(feature = "minimal")]
	{
		if let Some(theme) = THEMES.get(name) {
			return Some(theme);
		}
	}

	None
}
