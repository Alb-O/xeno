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

pub use xeno_primitives::{Color, Modifier, Style};

/// A syntax style with optional foreground, background, and modifiers.
#[derive(Clone, Copy, Debug, Default)]
pub struct SyntaxStyle {
	/// Foreground (text) color.
	pub fg: Option<Color>,
	/// Background color.
	pub bg: Option<Color>,
	/// Text modifiers (bold, italic, underline, etc.).
	pub modifiers: Modifier,
}

impl SyntaxStyle {
	/// Empty style with no colors or modifiers.
	pub const NONE: Self = Self {
		fg: None,
		bg: None,
		modifiers: Modifier::empty(),
	};

	/// Creates a style with only a foreground color.
	pub const fn fg(color: Color) -> Self {
		Self {
			fg: Some(color),
			bg: None,
			modifiers: Modifier::empty(),
		}
	}

	/// Creates a style with foreground color and modifiers.
	pub const fn fg_mod(color: Color, modifiers: Modifier) -> Self {
		Self {
			fg: Some(color),
			bg: None,
			modifiers,
		}
	}

	/// Returns a new style with the given background color added.
	pub const fn with_bg(mut self, color: Color) -> Self {
		self.bg = Some(color);
		self
	}

	/// Convert to abstract Style.
	pub fn to_style(self) -> Style {
		let mut style = Style::new().add_modifier(self.modifiers);
		if let Some(fg) = self.fg {
			style = style.fg(fg);
		}
		if let Some(bg) = self.bg {
			style = style.bg(bg);
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
	/// Style for attributes (e.g., `#[derive(...)]` in Rust).
	pub attribute: SyntaxStyle,
	/// Style for HTML/XML tags.
	pub tag: SyntaxStyle,
	/// Style for namespace identifiers.
	pub namespace: SyntaxStyle,

	/// Base style for all comments.
	pub comment: SyntaxStyle,
	/// Style for line comments (`//`).
	pub comment_line: SyntaxStyle,
	/// Style for block comments (`/* */`).
	pub comment_block: SyntaxStyle,
	/// Style for documentation comments (`///`, `/** */`).
	pub comment_block_documentation: SyntaxStyle,

	/// Base style for constants.
	pub constant: SyntaxStyle,
	/// Style for built-in constants (e.g., `None`, `null`).
	pub constant_builtin: SyntaxStyle,
	/// Style for boolean literals (`true`, `false`).
	pub constant_builtin_boolean: SyntaxStyle,
	/// Style for character literals.
	pub constant_character: SyntaxStyle,
	/// Style for escape sequences in strings/chars.
	pub constant_character_escape: SyntaxStyle,
	/// Base style for numeric literals.
	pub constant_numeric: SyntaxStyle,
	/// Style for integer literals.
	pub constant_numeric_integer: SyntaxStyle,
	/// Style for floating-point literals.
	pub constant_numeric_float: SyntaxStyle,

	/// Style for constructors (e.g., `Some(...)`, `new`).
	pub constructor: SyntaxStyle,

	/// Base style for function names.
	pub function: SyntaxStyle,
	/// Style for built-in functions.
	pub function_builtin: SyntaxStyle,
	/// Style for method calls.
	pub function_method: SyntaxStyle,
	/// Style for macro invocations.
	pub function_macro: SyntaxStyle,
	/// Style for special functions (e.g., `main`).
	pub function_special: SyntaxStyle,

	/// Base style for keywords.
	pub keyword: SyntaxStyle,
	/// Style for control flow keywords.
	pub keyword_control: SyntaxStyle,
	/// Style for conditional keywords (`if`, `else`, `match`).
	pub keyword_control_conditional: SyntaxStyle,
	/// Style for loop keywords (`for`, `while`, `loop`).
	pub keyword_control_repeat: SyntaxStyle,
	/// Style for import keywords (`use`, `import`, `require`).
	pub keyword_control_import: SyntaxStyle,
	/// Style for return keywords (`return`, `yield`).
	pub keyword_control_return: SyntaxStyle,
	/// Style for exception keywords (`try`, `catch`, `throw`).
	pub keyword_control_exception: SyntaxStyle,
	/// Style for operator keywords (`and`, `or`, `not`).
	pub keyword_operator: SyntaxStyle,
	/// Style for preprocessor directives.
	pub keyword_directive: SyntaxStyle,
	/// Style for function definition keywords (`fn`, `def`, `func`).
	pub keyword_function: SyntaxStyle,
	/// Style for storage keywords.
	pub keyword_storage: SyntaxStyle,
	/// Style for type keywords (`struct`, `enum`, `class`).
	pub keyword_storage_type: SyntaxStyle,
	/// Style for modifier keywords (`pub`, `mut`, `const`).
	pub keyword_storage_modifier: SyntaxStyle,

	/// Style for labels (e.g., `'lifetime`, `label:`).
	pub label: SyntaxStyle,

	/// Style for operators (`+`, `-`, `*`, `/`).
	pub operator: SyntaxStyle,
	/// Base style for punctuation.
	pub punctuation: SyntaxStyle,
	/// Style for brackets (`()`, `[]`, `{}`).
	pub punctuation_bracket: SyntaxStyle,
	/// Style for delimiters (`,`, `;`, `:`).
	pub punctuation_delimiter: SyntaxStyle,
	/// Style for special punctuation.
	pub punctuation_special: SyntaxStyle,

	/// Base style for string literals.
	pub string: SyntaxStyle,
	/// Style for regular expressions.
	pub string_regexp: SyntaxStyle,
	/// Style for special strings.
	pub string_special: SyntaxStyle,
	/// Style for file paths in strings.
	pub string_special_path: SyntaxStyle,
	/// Style for URLs in strings.
	pub string_special_url: SyntaxStyle,
	/// Style for symbols (e.g., Ruby `:symbol`).
	pub string_special_symbol: SyntaxStyle,

	/// Base style for type names.
	pub r#type: SyntaxStyle,
	/// Style for built-in types (`i32`, `String`, `bool`).
	pub type_builtin: SyntaxStyle,
	/// Style for type parameters (generics).
	pub type_parameter: SyntaxStyle,
	/// Style for enum variants.
	pub type_enum_variant: SyntaxStyle,

	/// Base style for variables.
	pub variable: SyntaxStyle,
	/// Style for built-in variables (`self`, `this`).
	pub variable_builtin: SyntaxStyle,
	/// Style for function parameters.
	pub variable_parameter: SyntaxStyle,
	/// Style for other variables.
	pub variable_other: SyntaxStyle,
	/// Style for struct/class members.
	pub variable_other_member: SyntaxStyle,

	/// Base style for markup headings.
	pub markup_heading: SyntaxStyle,
	/// Style for level-1 headings (`# H1`).
	pub markup_heading_1: SyntaxStyle,
	/// Style for level-2 headings (`## H2`).
	pub markup_heading_2: SyntaxStyle,
	/// Style for level-3 headings (`### H3`).
	pub markup_heading_3: SyntaxStyle,
	/// Style for bold text (`**bold**`).
	pub markup_bold: SyntaxStyle,
	/// Style for italic text (`*italic*`).
	pub markup_italic: SyntaxStyle,
	/// Style for strikethrough text (`~~strike~~`).
	pub markup_strikethrough: SyntaxStyle,
	/// Base style for links.
	pub markup_link: SyntaxStyle,
	/// Style for link URLs.
	pub markup_link_url: SyntaxStyle,
	/// Style for link text.
	pub markup_link_text: SyntaxStyle,
	/// Style for blockquotes.
	pub markup_quote: SyntaxStyle,
	/// Base style for raw/code content.
	pub markup_raw: SyntaxStyle,
	/// Style for inline code (`` `code` ``).
	pub markup_raw_inline: SyntaxStyle,
	/// Style for code blocks.
	pub markup_raw_block: SyntaxStyle,
	/// Style for list markers.
	pub markup_list: SyntaxStyle,

	/// Style for diff additions (`+`).
	pub diff_plus: SyntaxStyle,
	/// Style for diff deletions (`-`).
	pub diff_minus: SyntaxStyle,
	/// Style for diff modifications.
	pub diff_delta: SyntaxStyle,

	/// Style for special/miscellaneous tokens.
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
		let mut current = scope;
		loop {
			if let Some(style) = self.get_by_scope(current)
				&& (style.fg.is_some() || style.bg.is_some() || !style.modifiers.is_empty())
			{
				return style.to_style();
			}

			match current.rfind('.') {
				Some(idx) => current = &current[..idx],
				None => break,
			}
		}

		Style::new()
	}

	/// Get style by exact scope name (with dots converted to underscores).
	fn get_by_scope(&self, scope: &str) -> Option<SyntaxStyle> {
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
			"comment.line.documentation",
			"comment.block",
			"comment.block.documentation",
			"comment.unused",
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
			"keyword.special",
			"keyword.storage",
			"keyword.storage.type",
			"keyword.storage.modifier",
			"keyword.storage.modifier.mut",
			"keyword.storage.modifier.ref",
			"label",
			"operator",
			"operator.special",
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
			"type.enum.variant.builtin",
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

		let resolved = styles.resolve("keyword.control.import");
		assert_eq!(resolved.fg, Some(Color::Red));
	}

	#[test]
	fn test_resolve_partial_hierarchy() {
		let mut styles = SyntaxStyles::minimal();
		styles.keyword = SyntaxStyle::fg(Color::Red);
		styles.keyword_control = SyntaxStyle::fg(Color::Blue);

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
