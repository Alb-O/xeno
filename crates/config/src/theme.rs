//! Theme configuration parsing.
//!
//! Parses theme definitions from KDL format into runtime-usable structures.

use std::path::PathBuf;

use evildoer_manifest::syntax::{SyntaxStyle, SyntaxStyles};
pub use evildoer_manifest::theme::{
	NotificationColors, PopupColors, StatusColors, ThemeColors, ThemeVariant, UiColors,
};
use kdl::{KdlDocument, KdlNode};

use crate::error::{ConfigError, Result};
use crate::kdl_util::{ParseContext, get_color_field, parse_modifier, parse_palette};

/// A parsed theme with owned data suitable for runtime use.
#[derive(Debug, Clone)]
pub struct ParsedTheme {
	pub name: String,
	pub variant: ThemeVariant,
	pub aliases: Vec<String>,
	pub colors: ThemeColors,
	pub source_path: Option<PathBuf>,
}

impl ParsedTheme {
	/// Convert to an OwnedTheme for registration in the runtime theme registry.
	pub fn into_owned_theme(self) -> evildoer_manifest::OwnedTheme {
		evildoer_manifest::OwnedTheme {
			id: self.name.clone(),
			name: self.name,
			aliases: self.aliases,
			variant: self.variant,
			colors: self.colors,
			priority: 0,
			source: evildoer_manifest::RegistrySource::Runtime,
		}
	}
}

/// Parse a standalone theme file (top-level structure).
pub fn parse_standalone_theme(input: &str) -> Result<ParsedTheme> {
	let doc: KdlDocument = input.parse()?;
	let mut ctx = ParseContext::default();
	if let Some(node) = doc.get("palette") {
		parse_palette(node, &mut ctx)?;
	}

	let name = doc
		.get_arg("name")
		.and_then(|v| v.as_string())
		.ok_or_else(|| ConfigError::MissingField("name".into()))?
		.to_string();

	let variant = doc
		.get_arg("variant")
		.and_then(|v| v.as_string())
		.map(parse_variant)
		.transpose()?
		.unwrap_or_default();

	let aliases = doc
		.get("aliases")
		.map(|node| {
			node.entries()
				.iter()
				.filter_map(|e| e.value().as_string().map(String::from))
				.collect()
		})
		.unwrap_or_default();

	let ui = parse_ui_colors(doc.get("ui"), &ctx)?;
	let status = parse_status_colors(doc.get("status"), &ctx)?;
	let popup = parse_popup_colors(doc.get("popup"), &ctx)?;
	let syntax = parse_syntax_styles(doc.get("syntax"), &ctx)?;

	Ok(ParsedTheme {
		name,
		variant,
		aliases,
		colors: ThemeColors {
			ui,
			status,
			popup,
			notification: NotificationColors::INHERITED,
			syntax,
		},
		source_path: None,
	})
}

/// Parse a theme from a `theme { }` node in a config file.
pub fn parse_theme_node(node: &KdlNode) -> Result<ParsedTheme> {
	let children = node
		.children()
		.ok_or_else(|| ConfigError::MissingField("theme children".into()))?;

	let mut ctx = ParseContext::default();

	if let Some(palette_node) = children.get("palette") {
		parse_palette(palette_node, &mut ctx)?;
	}

	let name = children
		.get_arg("name")
		.and_then(|v| v.as_string())
		.ok_or_else(|| ConfigError::MissingField("name".into()))?
		.to_string();

	let variant = children
		.get_arg("variant")
		.and_then(|v| v.as_string())
		.map(parse_variant)
		.transpose()?
		.unwrap_or_default();

	let aliases = children
		.get("aliases")
		.map(|node| {
			node.entries()
				.iter()
				.filter_map(|e| e.value().as_string().map(String::from))
				.collect()
		})
		.unwrap_or_default();

	let ui = parse_ui_colors(children.get("ui"), &ctx)?;
	let status = parse_status_colors(children.get("status"), &ctx)?;
	let popup = parse_popup_colors(children.get("popup"), &ctx)?;
	let syntax = parse_syntax_styles(children.get("syntax"), &ctx)?;

	Ok(ParsedTheme {
		name,
		variant,
		aliases,
		colors: ThemeColors {
			ui,
			status,
			popup,
			notification: NotificationColors::INHERITED,
			syntax,
		},
		source_path: None,
	})
}

fn parse_variant(s: &str) -> Result<ThemeVariant> {
	match s.to_lowercase().as_str() {
		"dark" => Ok(ThemeVariant::Dark),
		"light" => Ok(ThemeVariant::Light),
		other => Err(ConfigError::InvalidVariant(other.to_string())),
	}
}

fn parse_ui_colors(node: Option<&KdlNode>, ctx: &ParseContext) -> Result<UiColors> {
	let node = node.ok_or_else(|| ConfigError::MissingField("ui".into()))?;
	let children = node
		.children()
		.ok_or_else(|| ConfigError::MissingField("ui".into()))?;

	Ok(UiColors {
		bg: get_color_field(children, "bg", ctx)?,
		fg: get_color_field(children, "fg", ctx)?,
		gutter_fg: get_color_field(children, "gutter-fg", ctx)?,
		cursor_bg: get_color_field(children, "cursor-bg", ctx)?,
		cursor_fg: get_color_field(children, "cursor-fg", ctx)?,
		cursorline_bg: get_color_field(children, "cursorline-bg", ctx)?,
		selection_bg: get_color_field(children, "selection-bg", ctx)?,
		selection_fg: get_color_field(children, "selection-fg", ctx)?,
		message_fg: get_color_field(children, "message-fg", ctx)?,
		command_input_fg: get_color_field(children, "command-input-fg", ctx)?,
		indent_guide_fg: get_optional_color_field(children, "indent-guide-fg", ctx)?,
	})
}

fn get_optional_color_field(
	children: &kdl::KdlDocument,
	name: &str,
	ctx: &ParseContext,
) -> Result<Option<evildoer_base::color::Color>> {
	if children.get(name).is_some() {
		Ok(Some(get_color_field(children, name, ctx)?))
	} else {
		Ok(None)
	}
}

fn parse_status_colors(node: Option<&KdlNode>, ctx: &ParseContext) -> Result<StatusColors> {
	let node = node.ok_or_else(|| ConfigError::MissingField("status".into()))?;
	let children = node
		.children()
		.ok_or_else(|| ConfigError::MissingField("status".into()))?;

	Ok(StatusColors {
		normal_bg: get_color_field(children, "normal-bg", ctx)?,
		normal_fg: get_color_field(children, "normal-fg", ctx)?,
		insert_bg: get_color_field(children, "insert-bg", ctx)?,
		insert_fg: get_color_field(children, "insert-fg", ctx)?,
		goto_bg: get_color_field(children, "goto-bg", ctx)?,
		goto_fg: get_color_field(children, "goto-fg", ctx)?,
		view_bg: get_color_field(children, "view-bg", ctx)?,
		view_fg: get_color_field(children, "view-fg", ctx)?,
		command_bg: get_color_field(children, "command-bg", ctx)?,
		command_fg: get_color_field(children, "command-fg", ctx)?,
		dim_fg: get_color_field(children, "dim-fg", ctx)?,
		warning_fg: get_color_field(children, "warning-fg", ctx)?,
		error_fg: get_color_field(children, "error-fg", ctx)?,
		success_fg: get_color_field(children, "success-fg", ctx)?,
	})
}

fn parse_popup_colors(node: Option<&KdlNode>, ctx: &ParseContext) -> Result<PopupColors> {
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

fn parse_syntax_styles(node: Option<&KdlNode>, ctx: &ParseContext) -> Result<SyntaxStyles> {
	let Some(node) = node else {
		return Ok(SyntaxStyles::minimal());
	};
	let Some(children) = node.children() else {
		return Ok(SyntaxStyles::minimal());
	};

	let mut styles = SyntaxStyles::minimal();
	for child in children.nodes() {
		parse_syntax_node(child, "", &mut styles, ctx)?;
	}
	Ok(styles)
}

fn parse_syntax_node(
	node: &KdlNode,
	prefix: &str,
	styles: &mut SyntaxStyles,
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

fn parse_style_from_node(node: &KdlNode, ctx: &ParseContext) -> Result<SyntaxStyle> {
	let mut style = SyntaxStyle::NONE;

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

fn set_syntax_style(styles: &mut SyntaxStyles, scope: &str, style: SyntaxStyle) {
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

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn test_parse_standalone_theme() {
		let kdl = include_str!("../../../runtime/themes/gruvbox.kdl");
		let theme = parse_standalone_theme(kdl).unwrap();

		assert_eq!(theme.name, "gruvbox");
		assert_eq!(theme.variant, ThemeVariant::Dark);
		assert_eq!(theme.aliases, vec!["gruvbox_dark", "gruvbox-dark"]);

		// Verify syntax styles are parsed
		let keyword_style = theme.colors.syntax.resolve("keyword");
		assert!(
			keyword_style.fg.is_some(),
			"keyword style should have fg color"
		);

		let comment_style = theme.colors.syntax.resolve("comment");
		assert!(
			comment_style.fg.is_some(),
			"comment style should have fg color"
		);
	}
}
