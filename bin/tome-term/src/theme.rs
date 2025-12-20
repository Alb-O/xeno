use linkme::distributed_slice;
use ratatui::style::Color;

#[non_exhaustive]
#[derive(Clone, Copy, Debug)]
pub struct UiColors {
	pub bg: Color,
	pub fg: Color,
	pub gutter_fg: Color,
	pub cursor_bg: Color,
	pub cursor_fg: Color,
	pub selection_bg: Color,
	pub selection_fg: Color,
	pub message_fg: Color,
	pub command_input_fg: Color,
}

#[non_exhaustive]
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

#[non_exhaustive]
#[derive(Clone, Copy, Debug)]
pub struct PopupColors {
	pub bg: Color,
	pub fg: Color,
	pub border: Color,
	pub title: Color,
}

#[non_exhaustive]
#[derive(Clone, Copy, Debug)]
pub struct ThemeColors {
	pub ui: UiColors,
	pub status: StatusColors,
	pub popup: PopupColors,
}

#[non_exhaustive]
#[derive(Clone, Copy, Debug)]
pub struct Theme {
	pub id: &'static str,
	pub name: &'static str,
	pub aliases: &'static [&'static str],
	pub colors: ThemeColors,
	pub priority: i16,
	pub source: tome_core::ext::ExtensionSource,
}

#[distributed_slice]
pub static THEMES: [Theme] = [..];

#[distributed_slice(THEMES)]
pub static DEFAULT_THEME: Theme = Theme {
	id: "default",
	name: "default",
	aliases: &[],
	colors: ThemeColors {
		ui: UiColors {
			bg: Color::Reset,
			fg: Color::Reset,
			gutter_fg: Color::DarkGray,
			cursor_bg: Color::White,
			cursor_fg: Color::Black,
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
	},
	priority: 0,
	source: tome_core::ext::ExtensionSource::Builtin,
};

pub fn get_theme(name: &str) -> Option<&'static Theme> {
	let normalize = |s: &str| -> String {
		s.chars()
			.filter(|c| *c != '-' && *c != '_')
			.collect::<String>()
			.to_lowercase()
	};

	let search = normalize(name);

	THEMES
		.iter()
		.find(|t| normalize(t.name) == search || t.aliases.iter().any(|a| normalize(a) == search))
}

pub fn blend_colors(fg: Color, bg: Color, alpha: f32) -> Color {
	let fg_rgb = match fg {
		Color::Rgb(r, g, b) => (r, g, b),
		_ => return fg, // Fallback for non-RGB colors
	};

	let bg_rgb = match bg {
		Color::Rgb(r, g, b) => (r, g, b),
		_ => return fg, // Fallback if background is unknown/non-RGB
	};

	let r = (fg_rgb.0 as f32 * alpha + bg_rgb.0 as f32 * (1.0 - alpha)) as u8;
	let g = (fg_rgb.1 as f32 * alpha + bg_rgb.1 as f32 * (1.0 - alpha)) as u8;
	let b = (fg_rgb.2 as f32 * alpha + bg_rgb.2 as f32 * (1.0 - alpha)) as u8;

	Color::Rgb(r, g, b)
}

pub fn suggest_theme(name: &str) -> Option<&'static str> {
	let name = name.to_lowercase();
	let mut best_match = None;
	let mut best_score = 0.0;

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

use tome_core::ext::{
	COMMANDS, CommandDef, CommandError, CommandOutcome, OPTIONS, OptionDef, OptionScope,
	OptionType, OptionValue,
};

#[distributed_slice(OPTIONS)]
pub static OPT_THEME: OptionDef = OptionDef {
	id: "theme",
	name: "theme",
	description: "Editor color theme",
	value_type: OptionType::String,
	default: || OptionValue::String("solarized_dark".to_string()),
	scope: OptionScope::Global,
	source: tome_core::ext::ExtensionSource::Builtin,
};

#[distributed_slice(COMMANDS)]
pub static CMD_THEME: CommandDef = CommandDef {
	id: "theme",
	name: "theme",
	aliases: &["colorscheme"],
	description: "Set the editor theme",
	handler: |ctx| {
		let theme_name = ctx
			.args
			.first()
			.ok_or(CommandError::MissingArgument("theme name"))?;
		ctx.editor
			.set_theme(theme_name)
			.map_err(|e| CommandError::Failed(e.to_string()))?;
		ctx.message(&format!("Theme set to {}", theme_name));
		Ok(CommandOutcome::Ok)
	},
	user_data: None,
	priority: 0,
	source: tome_core::ext::ExtensionSource::Builtin,
	required_caps: &[],
	flags: tome_core::ext::flags::NONE,
};
