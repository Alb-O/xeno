use linkme::distributed_slice;

pub mod themes;

// Re-export abstract color types from tome-base
pub use tome_base::color::{Color, Modifier};
pub use tome_manifest::syntax::{SyntaxStyle, SyntaxStyles};

/// Whether a theme uses a light or dark background.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum ThemeVariant {
	#[default]
	Dark,
	Light,
}

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
#[non_exhaustive]
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

#[non_exhaustive]
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

#[non_exhaustive]
#[derive(Clone, Copy, Debug)]
pub struct Theme {
	pub id: &'static str,
	pub name: &'static str,
	pub aliases: &'static [&'static str],
	pub variant: ThemeVariant,
	pub colors: ThemeColors,
	pub priority: i16,
	pub source: tome_manifest::RegistrySource,
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
		notification: NotificationColors::INHERITED,
		syntax: SyntaxStyles::minimal(),
	},
	priority: 0,
	source: tome_manifest::RegistrySource::Builtin,
};

impl ThemeColors {
	/// Resolve notification style for a given semantic identifier.
	/// Uses notification-specific overrides if set, otherwise inherits from popup/status colors.
	pub fn notification_style(&self, semantic: &str) -> tome_base::Style {
		let override_pair = self
			.notification
			.overrides
			.iter()
			.find(|(id, _)| *id == semantic)
			.map(|(_, pair)| pair);

		// Resolve background: notification override -> popup.bg
		let bg = override_pair.and_then(|p| p.bg).unwrap_or(self.popup.bg);

		// Resolve foreground: notification override -> semantic fallback from status/popup
		let fg = override_pair.and_then(|p| p.fg).unwrap_or_else(|| {
			use tome_manifest::*;
			match semantic {
				SEMANTIC_WARNING => self.status.warning_fg,
				SEMANTIC_ERROR => self.status.error_fg,
				SEMANTIC_SUCCESS => self.status.success_fg,
				SEMANTIC_DIM => self.status.dim_fg,
				_ => self.popup.fg, // Fallback for Info, Normal, and unknown semantics
			}
		});

		tome_base::Style::new().bg(bg).fg(fg)
	}

	/// Resolve notification border color.
	pub fn notification_border(&self) -> Color {
		self.notification.border.unwrap_or(self.popup.border)
	}
}

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

/// Blend two colors with the given alpha (0.0 = bg, 1.0 = fg).
/// Only works with RGB colors; returns fg unchanged for non-RGB.
pub fn blend_colors(fg: Color, bg: Color, alpha: f32) -> Color {
	let Color::Rgb(fg_r, fg_g, fg_b) = fg else {
		return fg;
	};
	let Color::Rgb(bg_r, bg_g, bg_b) = bg else {
		return fg;
	};

	let r = (fg_r as f32 * alpha + bg_r as f32 * (1.0 - alpha)) as u8;
	let g = (fg_g as f32 * alpha + bg_g as f32 * (1.0 - alpha)) as u8;
	let b = (fg_b as f32 * alpha + bg_b as f32 * (1.0 - alpha)) as u8;

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

use futures::future::LocalBoxFuture;
use tome_manifest::completion::{
	CompletionContext, CompletionItem, CompletionKind, CompletionResult, CompletionSource,
	PROMPT_COMMAND,
};
use tome_manifest::editor_ctx::MessageAccess;
use tome_manifest::{
	COMMANDS, CommandContext, CommandDef, CommandError, CommandOutcome, OPTIONS, OptionDef,
	OptionScope, OptionType, OptionValue,
};

pub const DEFAULT_THEME_ID: &str = "gruvbox";

#[distributed_slice(OPTIONS)]
pub static OPT_THEME: OptionDef = OptionDef {
	id: "theme",
	name: "theme",
	description: "Editor color theme",
	value_type: OptionType::String,
	default: || OptionValue::String(DEFAULT_THEME_ID.to_string()),
	scope: OptionScope::Global,
	source: tome_manifest::RegistrySource::Builtin,
};

fn cmd_theme<'a>(
	ctx: &'a mut CommandContext<'a>,
) -> LocalBoxFuture<'a, Result<CommandOutcome, CommandError>> {
	Box::pin(async move {
		let theme_name = ctx
			.args
			.first()
			.ok_or(CommandError::MissingArgument("theme name"))?;
		ctx.editor.set_theme(theme_name)?;
		ctx.notify("info", &format!("Theme set to '{}'", theme_name));
		Ok(CommandOutcome::Ok)
	})
}

#[distributed_slice(COMMANDS)]
pub static CMD_THEME: CommandDef = CommandDef {
	id: "theme",
	name: "theme",
	aliases: &["colorscheme"],
	description: "Set the editor theme",
	handler: cmd_theme,
	user_data: None,
	priority: 0,
	source: tome_manifest::RegistrySource::Builtin,
	required_caps: &[],
	flags: tome_manifest::flags::NONE,
};

/// Completion source for theme names.
/// Provides completions when typing `:theme <arg>` or `:colorscheme <arg>`.
pub struct ThemeSource;

impl CompletionSource for ThemeSource {
	fn complete(&self, ctx: &CompletionContext) -> CompletionResult {
		if ctx.prompt != PROMPT_COMMAND {
			return CompletionResult::empty();
		}

		// Parse the command line to check if we're completing theme arguments
		let parts: Vec<&str> = ctx.input.split_whitespace().collect();

		// Only complete if the command is "theme" or "colorscheme" and we have an argument position
		let is_theme_cmd = match parts.first() {
			Some(&"theme") | Some(&"colorscheme") => true,
			_ => false,
		};

		if !is_theme_cmd {
			return CompletionResult::empty();
		}

		// Get the partial theme name (if any)
		let prefix = parts.get(1).copied().unwrap_or("");

		// Check if we're still typing the command name vs. the argument
		// If the input doesn't end with a space and we only have 1 part, we're still typing the command
		if parts.len() == 1 && !ctx.input.ends_with(' ') {
			return CompletionResult::empty();
		}

		// Calculate where the argument starts (after "theme " or "colorscheme ")
		let cmd_name = parts.first().unwrap();
		let arg_start = cmd_name.len() + 1; // +1 for the space

		let items: Vec<_> = THEMES
			.iter()
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

		CompletionResult::new(arg_start, items)
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn test_theme_completion_replace_start() {
		let ctx = CompletionContext {
			input: "theme ".to_string(),
			cursor: 6,
			prompt: PROMPT_COMMAND,
		};

		let result = ThemeSource.complete(&ctx);
		assert!(!result.is_empty(), "Should have theme completions");
		assert_eq!(result.start, 6, "Theme completion should start at position 6");

		for item in &result.items {
			assert_eq!(item.kind, CompletionKind::Theme);
		}
	}

	#[test]
	fn test_theme_completion_with_partial_arg() {
		let ctx = CompletionContext {
			input: "theme gr".to_string(),
			cursor: 8,
			prompt: PROMPT_COMMAND,
		};

		let result = ThemeSource.complete(&ctx);
		assert_eq!(result.start, 6, "Should replace from position 6");

		// Should filter to themes starting with "gr"
		assert!(
			result.items.iter().any(|i| i.label == "gruvbox"),
			"Should include gruvbox"
		);
	}

	#[test]
	fn test_colorscheme_alias_completion() {
		let ctx = CompletionContext {
			input: "colorscheme ".to_string(),
			cursor: 12,
			prompt: PROMPT_COMMAND,
		};

		let result = ThemeSource.complete(&ctx);
		assert!(!result.is_empty(), "Should have theme completions for colorscheme alias");
		assert_eq!(
			result.start, 12,
			"Colorscheme completion should start at position 12"
		);
	}

	#[test]
	fn test_no_completion_for_other_commands() {
		let ctx = CompletionContext {
			input: "write ".to_string(),
			cursor: 6,
			prompt: PROMPT_COMMAND,
		};

		let result = ThemeSource.complete(&ctx);
		assert!(result.is_empty(), "Should not complete for non-theme commands");
	}

	#[test]
	fn test_no_completion_while_typing_command() {
		let ctx = CompletionContext {
			input: "them".to_string(),
			cursor: 4,
			prompt: PROMPT_COMMAND,
		};

		let result = ThemeSource.complete(&ctx);
		assert!(
			result.is_empty(),
			"Should not complete while still typing command name"
		);
	}
}
