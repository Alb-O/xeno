use xeno_primitives::{Color, Mode, Style};

use super::super::syntax::SyntaxStyles;
use crate::core::index::{BuildEntry, RegistryMetaRef, StrListRef};
pub use crate::core::{
	CapabilitySet, FrozenInterner, RegistryMeta, RegistryMetaStatic, RegistryRef, RegistrySource,
	Symbol, SymbolList, ThemeId,
};

/// Whether a theme uses a light or dark background.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum ThemeVariant {
	#[default]
	Dark,
	Light,
}

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

#[derive(Clone, Copy, Debug)]
pub struct PopupColors {
	pub bg: Color,
	pub fg: Color,
	pub border: Color,
	pub title: Color,
}

#[derive(Clone, Copy, Debug, Default)]
pub struct SemanticColorPair {
	pub bg: Option<Color>,
	pub fg: Option<Color>,
}

impl SemanticColorPair {
	pub const NONE: Self = Self { bg: None, fg: None };
}

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

/// A complete theme definition (static input).
#[derive(Clone, Copy, Debug)]
pub struct ThemeDef {
	pub meta: RegistryMetaStatic,
	pub variant: ThemeVariant,
	pub colors: ThemeColors,
}

/// Symbolized theme entry.
pub struct ThemeEntry {
	pub meta: RegistryMeta,
	pub variant: ThemeVariant,
	pub colors: ThemeColors,
}

crate::impl_registry_entry!(ThemeEntry);

impl BuildEntry<ThemeEntry> for ThemeDef {
	fn meta_ref(&self) -> RegistryMetaRef<'_> {
		RegistryMetaRef {
			id: self.meta.id,
			name: self.meta.name,
			keys: StrListRef::Static(self.meta.keys),
			description: self.meta.description,
			priority: self.meta.priority,
			source: self.meta.source,
			required_caps: self.meta.required_caps,
			flags: self.meta.flags,
		}
	}

	fn short_desc_str(&self) -> &str {
		self.meta.name
	}

	fn collect_strings<'a>(&'a self, sink: &mut Vec<&'a str>) {
		crate::core::index::meta_build::collect_meta_strings(&self.meta_ref(), sink, []);
	}

	fn build(&self, interner: &FrozenInterner, key_pool: &mut Vec<Symbol>) -> ThemeEntry {
		let meta =
			crate::core::index::meta_build::build_meta(interner, key_pool, self.meta_ref(), []);

		ThemeEntry {
			meta,
			variant: self.variant,
			colors: self.colors,
		}
	}
}

/// Unified input for theme registration.
pub type ThemeInput = crate::core::def_input::DefInput<ThemeDef>;
