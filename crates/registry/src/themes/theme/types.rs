use xeno_primitives::{Color, Mode, Style};

use super::super::syntax::SyntaxStyles;
use crate::core::{RegistryMeta, RegistrySource};

/// Whether a theme uses a light or dark background.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum ThemeVariant {
	/// Dark theme with light text on dark background.
	#[default]
	Dark,
	/// Light theme with dark text on light background.
	/// Light theme with dark text on light background.
	Light,
}

/// UI color definitions for the editor chrome.
#[derive(Clone, Copy, Debug)]
pub struct UiColors {
	pub bg: Color,
	pub fg: Color,
	pub nontext_bg: Color,
	pub gutter_fg: Color,
	pub cursor_bg: Color,
	pub cursor_fg: Color,
	pub cursorline_bg: Color,
	pub selection_bg: Color,
	pub selection_fg: Color,
	pub message_fg: Color,
	pub command_input_fg: Color,
}

/// A background/foreground color pair for UI elements like mode badges.
#[derive(Clone, Copy, Debug)]
pub struct ColorPair {
	pub bg: Color,
	pub fg: Color,
}

impl ColorPair {
	pub const fn new(bg: Color, fg: Color) -> Self {
		Self { bg, fg }
	}

	pub fn to_style(self) -> Style {
		Style::new().bg(self.bg).fg(self.fg)
	}
}

/// Mode indicator colors for status bar badges.
#[derive(Clone, Copy, Debug)]
pub struct ModeColors {
	pub normal: ColorPair,
	pub insert: ColorPair,
	pub prefix: ColorPair,
	pub command: ColorPair,
}

impl ModeColors {
	pub fn for_mode(&self, mode: &Mode) -> ColorPair {
		match mode {
			Mode::Normal => self.normal,
			Mode::Insert => self.insert,
			Mode::PendingAction(_) => self.command,
		}
	}
}

/// Semantic colors used throughout the UI.
#[derive(Clone, Copy, Debug)]
pub struct SemanticColors {
	pub error: Color,
	pub warning: Color,
	pub success: Color,
	pub info: Color,
	pub hint: Color,
	pub dim: Color,
	pub link: Color,
	pub match_hl: Color,
	pub accent: Color,
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
#[derive(Clone, Copy, Debug, Default)]
pub struct SemanticColorPair {
	pub bg: Option<Color>,
	pub fg: Option<Color>,
}

impl SemanticColorPair {
	pub const NONE: Self = Self { bg: None, fg: None };
}

/// Notification-specific color overrides.
#[derive(Clone, Copy, Debug)]
pub struct NotificationColors {
	pub border: Option<Color>,
	pub overrides: &'static [(&'static str, SemanticColorPair)],
}

impl NotificationColors {
	pub const INHERITED: Self = Self {
		border: None,
		overrides: &[],
	};
}

pub const SEMANTIC_INFO: &str = "info";
pub const SEMANTIC_WARNING: &str = "warning";
pub const SEMANTIC_ERROR: &str = "error";
pub const SEMANTIC_SUCCESS: &str = "success";
pub const SEMANTIC_DIM: &str = "dim";
pub const SEMANTIC_NORMAL: &str = "normal";

/// Complete theme color palette.
#[derive(Clone, Copy, Debug)]
pub struct ThemeColors {
	pub ui: UiColors,
	pub mode: ModeColors,
	pub semantic: SemanticColors,
	pub popup: PopupColors,
	pub notification: NotificationColors,
	pub syntax: SyntaxStyles,
}

impl ThemeColors {
	#[inline]
	pub fn mode_style(&self, mode: &Mode) -> Style {
		self.mode.for_mode(mode).to_style()
	}

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

	pub fn notification_border(&self) -> Color {
		self.notification.border.unwrap_or(self.popup.border)
	}
}

/// A complete theme definition.
#[derive(Clone, Copy, Debug)]
pub struct ThemeDef {
	pub meta: RegistryMeta,
	pub variant: ThemeVariant,
	pub colors: ThemeColors,
}

/// Owned theme data for runtime-loaded themes.
#[derive(Clone, Debug)]
pub struct OwnedTheme {
	pub id: String,
	pub name: String,
	pub aliases: Vec<String>,
	pub variant: ThemeVariant,
	pub colors: ThemeColors,
	pub priority: i16,
	pub source: RegistrySource,
}

impl OwnedTheme {
	pub fn leak(self) -> &'static ThemeDef {
		let id: &'static str = Box::leak(self.id.into_boxed_str());
		let name: &'static str = Box::leak(self.name.into_boxed_str());
		let aliases: &'static [&'static str] = Box::leak(
			self.aliases
				.into_iter()
				.map(|s| -> &'static str { Box::leak(s.into_boxed_str()) })
				.collect::<Vec<_>>()
				.into_boxed_slice(),
		);

		Box::leak(Box::new(ThemeDef {
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
