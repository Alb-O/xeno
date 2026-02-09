//! Configuration parsing utilities.

use std::collections::HashMap;

use kdl::KdlDocument;
use xeno_primitives::{Color, Modifier};

use super::{ConfigError, Result};

/// Context for parsing, including palette colors for variable resolution.
#[derive(Default)]
pub struct ParseContext {
	/// Named color definitions for `$variable` expansion.
	pub palette: HashMap<String, Color>,
}

impl ParseContext {
	/// Resolves a color value, expanding `$palette` variables.
	pub fn resolve_color(&self, value: &str) -> Result<Color> {
		if let Some(name) = value.strip_prefix('$') {
			self.palette
				.get(name)
				.copied()
				.ok_or_else(|| ConfigError::UndefinedPaletteColor(name.to_string()))
		} else {
			parse_color(value)
		}
	}
}

/// Parse a color value from a string.
///
/// Supports hex (`#RGB`, `#RRGGBB`), named colors, and `reset`/`default`.
pub fn parse_color(value: &str) -> Result<Color> {
	let value = value.trim();

	if value.eq_ignore_ascii_case("reset") || value.eq_ignore_ascii_case("default") {
		return Ok(Color::Reset);
	}

	if let Some(hex) = value.strip_prefix('#') {
		return parse_hex_color(hex);
	}

	parse_named_color(value)
}

/// Parses a hex color string (`#RGB` or `#RRGGBB`) into a Color.
fn parse_hex_color(hex: &str) -> Result<Color> {
	let hex = hex.trim_start_matches('#');
	let err = || ConfigError::InvalidColor(format!("#{hex}"));

	match hex.len() {
		3 => {
			let r = u8::from_str_radix(&hex[0..1].repeat(2), 16).map_err(|_| err())?;
			let g = u8::from_str_radix(&hex[1..2].repeat(2), 16).map_err(|_| err())?;
			let b = u8::from_str_radix(&hex[2..3].repeat(2), 16).map_err(|_| err())?;
			Ok(Color::Rgb(r, g, b))
		}
		6 => {
			let r = u8::from_str_radix(&hex[0..2], 16).map_err(|_| err())?;
			let g = u8::from_str_radix(&hex[2..4], 16).map_err(|_| err())?;
			let b = u8::from_str_radix(&hex[4..6], 16).map_err(|_| err())?;
			Ok(Color::Rgb(r, g, b))
		}
		_ => Err(err()),
	}
}

/// Parses a named color (e.g., "red", "bright-blue") into a Color.
fn parse_named_color(name: &str) -> Result<Color> {
	let normalized = name.to_lowercase().replace(['-', '_'], "");

	match normalized.as_str() {
		"black" => Ok(Color::Black),
		"red" => Ok(Color::Red),
		"green" => Ok(Color::Green),
		"yellow" => Ok(Color::Yellow),
		"blue" => Ok(Color::Blue),
		"magenta" => Ok(Color::Magenta),
		"cyan" => Ok(Color::Cyan),
		"gray" | "grey" => Ok(Color::Gray),
		"darkgray" | "darkgrey" => Ok(Color::DarkGray),
		"lightred" => Ok(Color::LightRed),
		"lightgreen" => Ok(Color::LightGreen),
		"lightyellow" => Ok(Color::LightYellow),
		"lightblue" => Ok(Color::LightBlue),
		"lightmagenta" => Ok(Color::LightMagenta),
		"lightcyan" => Ok(Color::LightCyan),
		"white" => Ok(Color::White),
		"reset" | "default" => Ok(Color::Reset),
		_ => Err(ConfigError::InvalidColor(name.to_string())),
	}
}

/// Parse text modifiers from a space-separated string.
pub fn parse_modifier(value: &str) -> Result<Modifier> {
	let mut modifiers = Modifier::empty();

	for part in value.split_whitespace() {
		let normalized = part.to_lowercase().replace(['-', '_'], "");
		modifiers |= match normalized.as_str() {
			"bold" => Modifier::BOLD,
			"dim" => Modifier::DIM,
			"italic" => Modifier::ITALIC,
			"underlined" | "underline" => Modifier::UNDERLINED,
			"slowblink" => Modifier::SLOW_BLINK,
			"rapidblink" => Modifier::RAPID_BLINK,
			"reversed" | "reverse" => Modifier::REVERSED,
			"hidden" => Modifier::HIDDEN,
			"crossedout" | "strikethrough" => Modifier::CROSSED_OUT,
			_ => return Err(ConfigError::InvalidModifier(part.to_string())),
		};
	}

	Ok(modifiers)
}

/// Get a required color field from a KDL document.
pub fn get_color_field(doc: &KdlDocument, name: &str, ctx: &ParseContext) -> Result<Color> {
	let value = doc
		.get_arg(name)
		.and_then(|v| v.as_string())
		.ok_or_else(|| ConfigError::MissingField(name.to_string()))?;
	ctx.resolve_color(value)
}

/// Gets an optional color field from a KDL document.
///
/// Returns `Ok(None)` if the field is absent, `Ok(Some(color))` if present and valid.
pub fn get_color_field_opt(
	doc: &KdlDocument,
	name: &str,
	ctx: &ParseContext,
) -> Result<Option<Color>> {
	doc.get_arg(name)
		.and_then(|v| v.as_string())
		.map(|value| ctx.resolve_color(value))
		.transpose()
}

/// Parse a palette block into the context.
pub fn parse_palette(node: &kdl::KdlNode, ctx: &mut ParseContext) -> Result<()> {
	let Some(children) = node.children() else {
		return Ok(());
	};
	for child in children.nodes() {
		let name = child.name().value();
		if let Some(value) = child.get(0).and_then(|v| v.as_string()) {
			ctx.palette.insert(name.to_string(), parse_color(value)?);
		}
	}
	Ok(())
}

/// Parses UI colors from a KDL node.
pub fn parse_ui_colors(
	node: Option<&kdl::KdlNode>,
	ctx: &ParseContext,
) -> Result<crate::themes::UiColors> {
	use crate::themes::UiColors;

	let node = node.ok_or_else(|| ConfigError::MissingField("ui".into()))?;
	let children = node
		.children()
		.ok_or_else(|| ConfigError::MissingField("ui".into()))?;

	let bg = get_color_field(children, "bg", ctx)?;
	let nontext_bg = get_color_field_opt(children, "nontext-bg", ctx)?
		.unwrap_or_else(|| bg.blend(xeno_primitives::Color::Black, 0.85));

	Ok(UiColors {
		bg,
		fg: get_color_field(children, "fg", ctx)?,
		nontext_bg,
		gutter_fg: get_color_field(children, "gutter-fg", ctx)?,
		cursor_bg: get_color_field(children, "cursor-bg", ctx)?,
		cursor_fg: get_color_field(children, "cursor-fg", ctx)?,
		cursorline_bg: get_color_field(children, "cursorline-bg", ctx)?,
		selection_bg: get_color_field(children, "selection-bg", ctx)?,
		selection_fg: get_color_field(children, "selection-fg", ctx)?,
		message_fg: get_color_field(children, "message-fg", ctx)?,
		command_input_fg: get_color_field(children, "command-input-fg", ctx)?,
	})
}

/// Parses mode indicator colors from a KDL node.
pub fn parse_mode_colors(
	node: Option<&kdl::KdlNode>,
	ctx: &ParseContext,
) -> Result<crate::themes::ModeColors> {
	use crate::themes::{ColorPair, ModeColors};

	let node = node.ok_or_else(|| ConfigError::MissingField("mode".into()))?;
	let children = node
		.children()
		.ok_or_else(|| ConfigError::MissingField("mode".into()))?;

	let parse_pair = |prefix: &str| -> Result<ColorPair> {
		Ok(ColorPair {
			bg: get_color_field(children, &format!("{prefix}-bg"), ctx)?,
			fg: get_color_field(children, &format!("{prefix}-fg"), ctx)?,
		})
	};

	Ok(ModeColors {
		normal: parse_pair("normal")?,
		insert: parse_pair("insert")?,
		prefix: parse_pair("prefix")?,
		command: parse_pair("command")?,
	})
}

/// Parses semantic colors from a KDL node.
pub fn parse_semantic_colors(
	node: Option<&kdl::KdlNode>,
	ctx: &ParseContext,
) -> Result<crate::themes::SemanticColors> {
	use crate::themes::SemanticColors;

	let node = node.ok_or_else(|| ConfigError::MissingField("semantic".into()))?;
	let children = node
		.children()
		.ok_or_else(|| ConfigError::MissingField("semantic".into()))?;

	Ok(SemanticColors {
		error: get_color_field(children, "error", ctx)?,
		warning: get_color_field(children, "warning", ctx)?,
		success: get_color_field(children, "success", ctx)?,
		info: get_color_field(children, "info", ctx)?,
		hint: get_color_field(children, "hint", ctx)?,
		dim: get_color_field(children, "dim", ctx)?,
		link: get_color_field(children, "link", ctx)?,
		match_hl: get_color_field(children, "match", ctx)?,
		accent: get_color_field(children, "accent", ctx)?,
	})
}

/// Parses popup/menu colors from a KDL node.
pub fn parse_popup_colors(
	node: Option<&kdl::KdlNode>,
	ctx: &ParseContext,
) -> Result<crate::themes::PopupColors> {
	use crate::themes::PopupColors;

	let node = node.ok_or_else(|| ConfigError::MissingField("popup".into()))?;
	let children = node
		.children()
		.ok_or_else(|| ConfigError::MissingField("popup".into()))?;

	Ok(PopupColors {
		bg: get_color_field(children, "bg", ctx)?,
		fg: get_color_field(children, "fg", ctx)?,
		border: get_color_field(children, "border", ctx)?,
		title: get_color_field(children, "title", ctx)?,
	})
}

/// Parses syntax highlighting styles from a KDL node.
pub fn parse_syntax_styles(
	node: Option<&kdl::KdlNode>,
	ctx: &ParseContext,
) -> Result<crate::themes::SyntaxStyles> {
	let Some(node) = node else {
		return Ok(crate::themes::SyntaxStyles::minimal());
	};
	let Some(children) = node.children() else {
		return Ok(crate::themes::SyntaxStyles::minimal());
	};

	let mut styles = crate::themes::SyntaxStyles::minimal();
	for child in children.nodes() {
		parse_syntax_node(child, "", &mut styles, ctx)?;
	}
	Ok(styles)
}

/// Parses a syntax node and its children recursively.
fn parse_syntax_node(
	node: &kdl::KdlNode,
	prefix: &str,
	styles: &mut crate::themes::SyntaxStyles,
	ctx: &ParseContext,
) -> Result<()> {
	let name = node.name().value();
	let scope = if prefix.is_empty() {
		name.to_string()
	} else {
		format!("{prefix}.{name}")
	};

	let style = parse_style_from_node(node, ctx)?;
	set_syntax_style(styles, &scope, style);

	if let Some(children) = node.children() {
		for child in children.nodes() {
			parse_syntax_node(child, &scope, styles, ctx)?;
		}
	}

	Ok(())
}

/// Parses a style definition from a KDL node's attributes.
fn parse_style_from_node(
	node: &kdl::KdlNode,
	ctx: &ParseContext,
) -> Result<crate::themes::SyntaxStyle> {
	let mut style = crate::themes::SyntaxStyle::NONE;

	if let Some(fg) = node.get("fg").and_then(|v| v.as_string()) {
		style.fg = Some(ctx.resolve_color(fg)?);
	}
	if let Some(bg) = node.get("bg").and_then(|v| v.as_string()) {
		style.bg = Some(ctx.resolve_color(bg)?);
	}
	if let Some(m) = node
		.get("mod")
		.or_else(|| node.get("modifiers"))
		.and_then(|v| v.as_string())
	{
		style.modifiers = parse_modifier(m)?;
	}

	Ok(style)
}

/// Sets a syntax style for the given scope name.
fn set_syntax_style(
	styles: &mut crate::themes::SyntaxStyles,
	scope: &str,
	style: crate::themes::SyntaxStyle,
) {
	if style.fg.is_none() && style.bg.is_none() && style.modifiers.is_empty() {
		return;
	}

	match scope {
		"attribute" => styles.attribute = style,
		"tag" => styles.tag = style,
		"namespace" => styles.namespace = style,
		"comment" => styles.comment = style,
		"comment.line" => styles.comment_line = style,
		"comment.block" => styles.comment_block = style,
		"comment.block.documentation" => styles.comment_block_documentation = style,
		"constant" => styles.constant = style,
		"constant.builtin" => styles.constant_builtin = style,
		"constant.builtin.boolean" => styles.constant_builtin_boolean = style,
		"constant.character" => styles.constant_character = style,
		"constant.character.escape" => styles.constant_character_escape = style,
		"constant.numeric" => styles.constant_numeric = style,
		"constant.numeric.integer" => styles.constant_numeric_integer = style,
		"constant.numeric.float" => styles.constant_numeric_float = style,
		"constructor" => styles.constructor = style,
		"function" => styles.function = style,
		"function.builtin" => styles.function_builtin = style,
		"function.method" => styles.function_method = style,
		"function.macro" => styles.function_macro = style,
		"function.special" => styles.function_special = style,
		"keyword" => styles.keyword = style,
		"keyword.control" => styles.keyword_control = style,
		"keyword.control.conditional" => styles.keyword_control_conditional = style,
		"keyword.control.repeat" => styles.keyword_control_repeat = style,
		"keyword.control.import" => styles.keyword_control_import = style,
		"keyword.control.return" => styles.keyword_control_return = style,
		"keyword.control.exception" => styles.keyword_control_exception = style,
		"keyword.operator" => styles.keyword_operator = style,
		"keyword.directive" => styles.keyword_directive = style,
		"keyword.function" => styles.keyword_function = style,
		"keyword.storage" => styles.keyword_storage = style,
		"keyword.storage.type" => styles.keyword_storage_type = style,
		"keyword.storage.modifier" => styles.keyword_storage_modifier = style,
		"label" => styles.label = style,
		"operator" => styles.operator = style,
		"punctuation" => styles.punctuation = style,
		"punctuation.bracket" => styles.punctuation_bracket = style,
		"punctuation.delimiter" => styles.punctuation_delimiter = style,
		"punctuation.special" => styles.punctuation_special = style,
		"string" => styles.string = style,
		"string.regexp" => styles.string_regexp = style,
		"string.special" => styles.string_special = style,
		"string.special.path" => styles.string_special_path = style,
		"string.special.url" => styles.string_special_url = style,
		"string.special.symbol" => styles.string_special_symbol = style,
		"type" => styles.r#type = style,
		"type.builtin" => styles.type_builtin = style,
		"type.parameter" => styles.type_parameter = style,
		"type.enum.variant" => styles.type_enum_variant = style,
		"variable" => styles.variable = style,
		"variable.builtin" => styles.variable_builtin = style,
		"variable.parameter" => styles.variable_parameter = style,
		"variable.other" => styles.variable_other = style,
		"variable.other.member" => styles.variable_other_member = style,
		"markup.heading" => styles.markup_heading = style,
		"markup.heading.1" => styles.markup_heading_1 = style,
		"markup.heading.2" => styles.markup_heading_2 = style,
		"markup.heading.3" => styles.markup_heading_3 = style,
		"markup.bold" => styles.markup_bold = style,
		"markup.italic" => styles.markup_italic = style,
		"markup.strikethrough" => styles.markup_strikethrough = style,
		"markup.link" => styles.markup_link = style,
		"markup.link.url" => styles.markup_link_url = style,
		"markup.link.text" => styles.markup_link_text = style,
		"markup.quote" => styles.markup_quote = style,
		"markup.raw" => styles.markup_raw = style,
		"markup.raw.inline" => styles.markup_raw_inline = style,
		"markup.raw.block" => styles.markup_raw_block = style,
		"markup.list" => styles.markup_list = style,
		"diff.plus" => styles.diff_plus = style,
		"diff.minus" => styles.diff_minus = style,
		"diff.delta" => styles.diff_delta = style,
		"special" => styles.special = style,
		_ => {}
	}
}
