//! Syntax highlighting styles using Helix-compatible scope names.
//!
//! This module defines strongly-typed Rust structs for syntax highlighting
//! that map directly to tree-sitter capture names used in `.scm` query files.
//! The scope naming follows Helix conventions for compatibility with their
//! extensive query library.
//!
//! # Scope Hierarchy
//!
//! Scopes follow a dot-separated hierarchy (e.g., `keyword.control.import`).
//! When resolving styles, more specific scopes take precedence. If a specific
//! scope isn't defined, it falls back to its parent (e.g., `keyword.control`
//! then `keyword`).
//!
//! # Example
//!
//! ```ignore
//! let syntax = SyntaxStyles::gruvbox();
//! let style = syntax.resolve("keyword.control.import");
//! // Returns keyword.control.import style, or falls back to keyword.control, then keyword
//! ```

// Re-export from ratatui for convenience when defining themes
use ratatui::style::Style;
pub use ratatui::style::{Color, Modifier};

/// A syntax style with optional foreground, background, and modifiers.
#[derive(Clone, Copy, Debug, Default)]
pub struct SyntaxStyle {
	pub fg: Option<Color>,
	pub bg: Option<Color>,
	pub modifiers: Modifier,
}

impl SyntaxStyle {
	pub const NONE: Self = Self {
		fg: None,
		bg: None,
		modifiers: Modifier::empty(),
	};

	pub const fn fg(color: Color) -> Self {
		Self {
			fg: Some(color),
			bg: None,
			modifiers: Modifier::empty(),
		}
	}

	pub const fn fg_mod(color: Color, modifiers: Modifier) -> Self {
		Self {
			fg: Some(color),
			bg: None,
			modifiers,
		}
	}

	pub const fn with_bg(mut self, color: Color) -> Self {
		self.bg = Some(color);
		self
	}

	/// Convert to ratatui Style.
	pub fn to_style(self) -> Style {
		let mut style = Style::default();
		if let Some(fg) = self.fg {
			style = style.fg(fg);
		}
		if let Some(bg) = self.bg {
			style = style.bg(bg);
		}
		if !self.modifiers.is_empty() {
			style = style.add_modifier(self.modifiers);
		}
		style
	}
}

/// Core syntax highlighting styles.
///
/// These map directly to tree-sitter capture names in `.scm` files.
/// Field names use underscores where Helix uses dots (e.g., `keyword_control`
/// maps to `@keyword.control` in queries).
#[derive(Clone, Copy, Debug)]
pub struct SyntaxStyles {
	pub attribute: SyntaxStyle,
	pub tag: SyntaxStyle,
	pub namespace: SyntaxStyle,

	pub comment: SyntaxStyle,
	pub comment_line: SyntaxStyle,
	pub comment_block: SyntaxStyle,
	pub comment_block_documentation: SyntaxStyle,

	pub constant: SyntaxStyle,
	pub constant_builtin: SyntaxStyle,
	pub constant_builtin_boolean: SyntaxStyle,
	pub constant_character: SyntaxStyle,
	pub constant_character_escape: SyntaxStyle,
	pub constant_numeric: SyntaxStyle,
	pub constant_numeric_integer: SyntaxStyle,
	pub constant_numeric_float: SyntaxStyle,

	pub constructor: SyntaxStyle,

	pub function: SyntaxStyle,
	pub function_builtin: SyntaxStyle,
	pub function_method: SyntaxStyle,
	pub function_macro: SyntaxStyle,
	pub function_special: SyntaxStyle,

	pub keyword: SyntaxStyle,
	pub keyword_control: SyntaxStyle,
	pub keyword_control_conditional: SyntaxStyle,
	pub keyword_control_repeat: SyntaxStyle,
	pub keyword_control_import: SyntaxStyle,
	pub keyword_control_return: SyntaxStyle,
	pub keyword_control_exception: SyntaxStyle,
	pub keyword_operator: SyntaxStyle,
	pub keyword_directive: SyntaxStyle,
	pub keyword_function: SyntaxStyle,
	pub keyword_storage: SyntaxStyle,
	pub keyword_storage_type: SyntaxStyle,
	pub keyword_storage_modifier: SyntaxStyle,

	pub label: SyntaxStyle,

	pub operator: SyntaxStyle,
	pub punctuation: SyntaxStyle,
	pub punctuation_bracket: SyntaxStyle,
	pub punctuation_delimiter: SyntaxStyle,
	pub punctuation_special: SyntaxStyle,

	pub string: SyntaxStyle,
	pub string_regexp: SyntaxStyle,
	pub string_special: SyntaxStyle,
	pub string_special_path: SyntaxStyle,
	pub string_special_url: SyntaxStyle,
	pub string_special_symbol: SyntaxStyle,

	pub r#type: SyntaxStyle,
	pub type_builtin: SyntaxStyle,
	pub type_parameter: SyntaxStyle,
	pub type_enum_variant: SyntaxStyle,

	pub variable: SyntaxStyle,
	pub variable_builtin: SyntaxStyle,
	pub variable_parameter: SyntaxStyle,
	pub variable_other: SyntaxStyle,
	pub variable_other_member: SyntaxStyle,

	pub markup_heading: SyntaxStyle,
	pub markup_heading_1: SyntaxStyle,
	pub markup_heading_2: SyntaxStyle,
	pub markup_heading_3: SyntaxStyle,
	pub markup_bold: SyntaxStyle,
	pub markup_italic: SyntaxStyle,
	pub markup_strikethrough: SyntaxStyle,
	pub markup_link: SyntaxStyle,
	pub markup_link_url: SyntaxStyle,
	pub markup_link_text: SyntaxStyle,
	pub markup_quote: SyntaxStyle,
	pub markup_raw: SyntaxStyle,
	pub markup_raw_inline: SyntaxStyle,
	pub markup_raw_block: SyntaxStyle,
	pub markup_list: SyntaxStyle,

	pub diff_plus: SyntaxStyle,
	pub diff_minus: SyntaxStyle,
	pub diff_delta: SyntaxStyle,

	pub special: SyntaxStyle,
}

impl Default for SyntaxStyles {
	fn default() -> Self {
		Self::minimal()
	}
}

impl SyntaxStyles {
	/// Minimal theme with no colors (inherits terminal defaults).
	pub const fn minimal() -> Self {
		Self {
			attribute: SyntaxStyle::NONE,
			tag: SyntaxStyle::NONE,
			namespace: SyntaxStyle::NONE,
			comment: SyntaxStyle::NONE,
			comment_line: SyntaxStyle::NONE,
			comment_block: SyntaxStyle::NONE,
			comment_block_documentation: SyntaxStyle::NONE,
			constant: SyntaxStyle::NONE,
			constant_builtin: SyntaxStyle::NONE,
			constant_builtin_boolean: SyntaxStyle::NONE,
			constant_character: SyntaxStyle::NONE,
			constant_character_escape: SyntaxStyle::NONE,
			constant_numeric: SyntaxStyle::NONE,
			constant_numeric_integer: SyntaxStyle::NONE,
			constant_numeric_float: SyntaxStyle::NONE,
			constructor: SyntaxStyle::NONE,
			function: SyntaxStyle::NONE,
			function_builtin: SyntaxStyle::NONE,
			function_method: SyntaxStyle::NONE,
			function_macro: SyntaxStyle::NONE,
			function_special: SyntaxStyle::NONE,
			keyword: SyntaxStyle::NONE,
			keyword_control: SyntaxStyle::NONE,
			keyword_control_conditional: SyntaxStyle::NONE,
			keyword_control_repeat: SyntaxStyle::NONE,
			keyword_control_import: SyntaxStyle::NONE,
			keyword_control_return: SyntaxStyle::NONE,
			keyword_control_exception: SyntaxStyle::NONE,
			keyword_operator: SyntaxStyle::NONE,
			keyword_directive: SyntaxStyle::NONE,
			keyword_function: SyntaxStyle::NONE,
			keyword_storage: SyntaxStyle::NONE,
			keyword_storage_type: SyntaxStyle::NONE,
			keyword_storage_modifier: SyntaxStyle::NONE,
			label: SyntaxStyle::NONE,
			operator: SyntaxStyle::NONE,
			punctuation: SyntaxStyle::NONE,
			punctuation_bracket: SyntaxStyle::NONE,
			punctuation_delimiter: SyntaxStyle::NONE,
			punctuation_special: SyntaxStyle::NONE,
			string: SyntaxStyle::NONE,
			string_regexp: SyntaxStyle::NONE,
			string_special: SyntaxStyle::NONE,
			string_special_path: SyntaxStyle::NONE,
			string_special_url: SyntaxStyle::NONE,
			string_special_symbol: SyntaxStyle::NONE,
			r#type: SyntaxStyle::NONE,
			type_builtin: SyntaxStyle::NONE,
			type_parameter: SyntaxStyle::NONE,
			type_enum_variant: SyntaxStyle::NONE,
			variable: SyntaxStyle::NONE,
			variable_builtin: SyntaxStyle::NONE,
			variable_parameter: SyntaxStyle::NONE,
			variable_other: SyntaxStyle::NONE,
			variable_other_member: SyntaxStyle::NONE,
			markup_heading: SyntaxStyle::NONE,
			markup_heading_1: SyntaxStyle::NONE,
			markup_heading_2: SyntaxStyle::NONE,
			markup_heading_3: SyntaxStyle::NONE,
			markup_bold: SyntaxStyle::NONE,
			markup_italic: SyntaxStyle::NONE,
			markup_strikethrough: SyntaxStyle::NONE,
			markup_link: SyntaxStyle::NONE,
			markup_link_url: SyntaxStyle::NONE,
			markup_link_text: SyntaxStyle::NONE,
			markup_quote: SyntaxStyle::NONE,
			markup_raw: SyntaxStyle::NONE,
			markup_raw_inline: SyntaxStyle::NONE,
			markup_raw_block: SyntaxStyle::NONE,
			markup_list: SyntaxStyle::NONE,
			diff_plus: SyntaxStyle::NONE,
			diff_minus: SyntaxStyle::NONE,
			diff_delta: SyntaxStyle::NONE,
			special: SyntaxStyle::NONE,
		}
	}

	/// Resolve a scope name to a style with hierarchical fallback.
	///
	/// Given "keyword.control.import", tries in order:
	/// 1. keyword_control_import
	/// 2. keyword_control
	/// 3. keyword
	/// 4. Default style
	pub fn resolve(&self, scope: &str) -> Style {
		// Try exact match first, then progressively shorter prefixes
		let mut current = scope;
		loop {
			if let Some(style) = self.get_by_scope(current) {
				if style.fg.is_some() || style.bg.is_some() || !style.modifiers.is_empty() {
					return style.to_style();
				}
			}

			match current.rfind('.') {
				Some(idx) => current = &current[..idx],
				None => break,
			}
		}

		Style::default()
	}

	/// Get style by exact scope name (with dots converted to underscores).
	fn get_by_scope(&self, scope: &str) -> Option<SyntaxStyle> {
		// Convert dots to underscores for matching
		Some(match scope {
			"attribute" => self.attribute,
			"tag" => self.tag,
			"namespace" => self.namespace,

			"comment" => self.comment,
			"comment.line" => self.comment_line,
			"comment.block" => self.comment_block,
			"comment.block.documentation" => self.comment_block_documentation,

			"constant" => self.constant,
			"constant.builtin" => self.constant_builtin,
			"constant.builtin.boolean" => self.constant_builtin_boolean,
			"constant.character" => self.constant_character,
			"constant.character.escape" => self.constant_character_escape,
			"constant.numeric" => self.constant_numeric,
			"constant.numeric.integer" => self.constant_numeric_integer,
			"constant.numeric.float" => self.constant_numeric_float,

			"constructor" => self.constructor,

			"function" => self.function,
			"function.builtin" => self.function_builtin,
			"function.method" => self.function_method,
			"function.macro" => self.function_macro,
			"function.special" => self.function_special,

			"keyword" => self.keyword,
			"keyword.control" => self.keyword_control,
			"keyword.control.conditional" => self.keyword_control_conditional,
			"keyword.control.repeat" => self.keyword_control_repeat,
			"keyword.control.import" => self.keyword_control_import,
			"keyword.control.return" => self.keyword_control_return,
			"keyword.control.exception" => self.keyword_control_exception,
			"keyword.operator" => self.keyword_operator,
			"keyword.directive" => self.keyword_directive,
			"keyword.function" => self.keyword_function,
			"keyword.storage" => self.keyword_storage,
			"keyword.storage.type" => self.keyword_storage_type,
			"keyword.storage.modifier" => self.keyword_storage_modifier,

			"label" => self.label,

			"operator" => self.operator,
			"punctuation" => self.punctuation,
			"punctuation.bracket" => self.punctuation_bracket,
			"punctuation.delimiter" => self.punctuation_delimiter,
			"punctuation.special" => self.punctuation_special,

			"string" => self.string,
			"string.regexp" => self.string_regexp,
			"string.special" => self.string_special,
			"string.special.path" => self.string_special_path,
			"string.special.url" => self.string_special_url,
			"string.special.symbol" => self.string_special_symbol,

			"type" => self.r#type,
			"type.builtin" => self.type_builtin,
			"type.parameter" => self.type_parameter,
			"type.enum.variant" => self.type_enum_variant,

			"variable" => self.variable,
			"variable.builtin" => self.variable_builtin,
			"variable.parameter" => self.variable_parameter,
			"variable.other" => self.variable_other,
			"variable.other.member" => self.variable_other_member,

			"markup.heading" => self.markup_heading,
			"markup.heading.1" => self.markup_heading_1,
			"markup.heading.2" => self.markup_heading_2,
			"markup.heading.3" => self.markup_heading_3,
			"markup.bold" => self.markup_bold,
			"markup.italic" => self.markup_italic,
			"markup.strikethrough" => self.markup_strikethrough,
			"markup.link" => self.markup_link,
			"markup.link.url" => self.markup_link_url,
			"markup.link.text" => self.markup_link_text,
			"markup.quote" => self.markup_quote,
			"markup.raw" => self.markup_raw,
			"markup.raw.inline" => self.markup_raw_inline,
			"markup.raw.block" => self.markup_raw_block,
			"markup.list" => self.markup_list,

			"diff.plus" => self.diff_plus,
			"diff.minus" => self.diff_minus,
			"diff.delta" => self.diff_delta,

			"special" => self.special,

			_ => return None,
		})
	}

	/// Returns the list of all recognized scope names.
	/// Used to configure the tree-sitter highlighter.
	pub fn scope_names() -> &'static [&'static str] {
		&[
			"attribute",
			"tag",
			"namespace",
			"comment",
			"comment.line",
			"comment.block",
			"comment.block.documentation",
			"constant",
			"constant.builtin",
			"constant.builtin.boolean",
			"constant.character",
			"constant.character.escape",
			"constant.numeric",
			"constant.numeric.integer",
			"constant.numeric.float",
			"constructor",
			"function",
			"function.builtin",
			"function.method",
			"function.macro",
			"function.special",
			"keyword",
			"keyword.control",
			"keyword.control.conditional",
			"keyword.control.repeat",
			"keyword.control.import",
			"keyword.control.return",
			"keyword.control.exception",
			"keyword.operator",
			"keyword.directive",
			"keyword.function",
			"keyword.storage",
			"keyword.storage.type",
			"keyword.storage.modifier",
			"label",
			"operator",
			"punctuation",
			"punctuation.bracket",
			"punctuation.delimiter",
			"punctuation.special",
			"string",
			"string.regexp",
			"string.special",
			"string.special.path",
			"string.special.url",
			"string.special.symbol",
			"type",
			"type.builtin",
			"type.parameter",
			"type.enum.variant",
			"variable",
			"variable.builtin",
			"variable.parameter",
			"variable.other",
			"variable.other.member",
			"markup.heading",
			"markup.heading.1",
			"markup.heading.2",
			"markup.heading.3",
			"markup.bold",
			"markup.italic",
			"markup.strikethrough",
			"markup.link",
			"markup.link.url",
			"markup.link.text",
			"markup.quote",
			"markup.raw",
			"markup.raw.inline",
			"markup.raw.block",
			"markup.list",
			"diff.plus",
			"diff.minus",
			"diff.delta",
			"special",
		]
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn test_resolve_exact_match() {
		let mut styles = SyntaxStyles::minimal();
		styles.keyword = SyntaxStyle::fg(Color::Red);

		let resolved = styles.resolve("keyword");
		assert_eq!(resolved.fg, Some(Color::Red));
	}

	#[test]
	fn test_resolve_hierarchical_fallback() {
		let mut styles = SyntaxStyles::minimal();
		styles.keyword = SyntaxStyle::fg(Color::Red);
		// keyword.control.import is not set, should fall back to keyword

		let resolved = styles.resolve("keyword.control.import");
		assert_eq!(resolved.fg, Some(Color::Red));
	}

	#[test]
	fn test_resolve_partial_hierarchy() {
		let mut styles = SyntaxStyles::minimal();
		styles.keyword = SyntaxStyle::fg(Color::Red);
		styles.keyword_control = SyntaxStyle::fg(Color::Blue);
		// keyword.control.import not set, should fall back to keyword.control

		let resolved = styles.resolve("keyword.control.import");
		assert_eq!(resolved.fg, Some(Color::Blue));
	}

	#[test]
	fn test_scope_names_complete() {
		let names = SyntaxStyles::scope_names();
		assert!(names.contains(&"keyword"));
		assert!(names.contains(&"function.macro"));
		assert!(names.contains(&"variable.other.member"));
	}
}
