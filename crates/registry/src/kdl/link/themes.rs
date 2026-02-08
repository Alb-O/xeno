use std::str::FromStr;

use super::*;
use crate::kdl::types::{RawStyle, ThemesBlob};
use crate::themes::theme::LinkedThemeDef;
use crate::themes::{
	Color, ColorPair, ModeColors, Modifier, NotificationColors, PopupColors, SemanticColors,
	SyntaxStyle, SyntaxStyles, ThemeColors, ThemeVariant, UiColors,
};

pub fn link_themes(blob: &ThemesBlob) -> Vec<LinkedThemeDef> {
	blob.themes
		.iter()
		.map(|meta| {
			let id = format!("xeno-registry::{}", meta.name);

			let variant = match meta.variant.as_str() {
				"dark" => ThemeVariant::Dark,
				"light" => ThemeVariant::Light,
				other => panic!("Theme '{}' unknown variant: '{}'", meta.name, other),
			};

			let mut palette = HashMap::new();
			let mut pending = meta.palette.clone();
			let mut progress = true;
			while progress && !pending.is_empty() {
				progress = false;
				let mut resolved_in_pass = Vec::new();
				for (name, val) in &pending {
					if let Ok(color) = parse_color(val, &palette) {
						palette.insert(name.clone(), color);
						resolved_in_pass.push(name.clone());
						progress = true;
					}
				}
				for name in resolved_in_pass {
					pending.remove(&name);
				}
			}

			if !pending.is_empty() {
				panic!(
					"Theme '{}' has unresolved or cyclic palette references: {:?}",
					meta.name,
					pending.keys().collect::<Vec<_>>()
				);
			}

			let colors = ThemeColors {
				ui: build_ui_colors(&meta.ui, &palette, &meta.name),
				mode: build_mode_colors(&meta.mode, &palette, &meta.name),
				semantic: build_semantic_colors(&meta.semantic, &palette, &meta.name),
				popup: build_popup_colors(&meta.popup, &palette, &meta.name),
				notification: NotificationColors::INHERITED,
				syntax: build_syntax_styles(&meta.syntax, &palette, &meta.name),
			};

			LinkedThemeDef {
				id,
				name: meta.name.clone(),
				keys: meta.keys.clone(),
				description: meta.description.clone(),
				priority: meta.priority,
				variant,
				colors,
				source: RegistrySource::Builtin,
			}
		})
		.collect()
}

fn parse_color(s: &str, palette: &HashMap<String, Color>) -> Result<Color, String> {
	if let Some(name) = s.strip_prefix('$') {
		return palette
			.get(name)
			.copied()
			.ok_or_else(|| format!("unknown palette color: {name}"));
	}
	Color::from_str(s).map_err(|_| format!("invalid color: {s}"))
}

fn build_ui_colors(
	map: &HashMap<String, String>,
	palette: &HashMap<String, Color>,
	theme_name: &str,
) -> UiColors {
	let get = |key: &str, default: Color| {
		map.get(key)
			.map(|s| {
				parse_color(s, palette)
					.unwrap_or_else(|e| panic!("Theme '{}' UI error: {}: {}", theme_name, key, e))
			})
			.unwrap_or(default)
	};
	let bg = get("bg", Color::Reset);
	UiColors {
		bg,
		fg: get("fg", Color::Reset),
		nontext_bg: get("nontext-bg", bg),
		gutter_fg: get("gutter-fg", Color::DarkGray),
		cursor_bg: get("cursor-bg", Color::White),
		cursor_fg: get("cursor-fg", Color::Black),
		cursorline_bg: get("cursorline-bg", Color::DarkGray),
		selection_bg: get("selection-bg", Color::Blue),
		selection_fg: get("selection-fg", Color::White),
		message_fg: get("message-fg", Color::Yellow),
		command_input_fg: get("command-input-fg", Color::White),
	}
}

fn build_mode_colors(
	map: &HashMap<String, String>,
	palette: &HashMap<String, Color>,
	theme_name: &str,
) -> ModeColors {
	let get = |key: &str, default: Color| {
		map.get(key)
			.map(|s| {
				parse_color(s, palette)
					.unwrap_or_else(|e| panic!("Theme '{}' mode error: {}: {}", theme_name, key, e))
			})
			.unwrap_or(default)
	};
	ModeColors {
		normal: ColorPair::new(
			get("normal-bg", Color::Blue),
			get("normal-fg", Color::White),
		),
		insert: ColorPair::new(
			get("insert-bg", Color::Green),
			get("insert-fg", Color::Black),
		),
		prefix: ColorPair::new(
			get("prefix-bg", Color::Magenta),
			get("prefix-fg", Color::White),
		),
		command: ColorPair::new(
			get("command-bg", Color::Yellow),
			get("command-fg", Color::Black),
		),
	}
}

fn build_semantic_colors(
	map: &HashMap<String, String>,
	palette: &HashMap<String, Color>,
	theme_name: &str,
) -> SemanticColors {
	let get = |key: &str, default: Color| {
		map.get(key)
			.map(|s| {
				parse_color(s, palette).unwrap_or_else(|e| {
					panic!("Theme '{}' semantic error: {}: {}", theme_name, key, e)
				})
			})
			.unwrap_or(default)
	};
	SemanticColors {
		error: get("error", Color::Red),
		warning: get("warning", Color::Yellow),
		success: get("success", Color::Green),
		info: get("info", Color::Cyan),
		hint: get("hint", Color::DarkGray),
		dim: get("dim", Color::DarkGray),
		link: get("link", Color::Cyan),
		match_hl: get("match", Color::Green),
		accent: get("accent", Color::Cyan),
	}
}

fn build_popup_colors(
	map: &HashMap<String, String>,
	palette: &HashMap<String, Color>,
	theme_name: &str,
) -> PopupColors {
	let get = |key: &str, default: Color| {
		map.get(key)
			.map(|s| {
				parse_color(s, palette).unwrap_or_else(|e| {
					panic!("Theme '{}' popup error: {}: {}", theme_name, key, e)
				})
			})
			.unwrap_or(default)
	};
	PopupColors {
		bg: get("bg", Color::Reset),
		fg: get("fg", Color::Reset),
		border: get("border", Color::DarkGray),
		title: get("title", Color::Yellow),
	}
}

fn build_syntax_styles(
	map: &HashMap<String, RawStyle>,
	palette: &HashMap<String, Color>,
	theme_name: &str,
) -> SyntaxStyles {
	let mut styles = SyntaxStyles::minimal();
	for (scope, raw) in map {
		let style = SyntaxStyle {
			fg: raw.fg.as_ref().map(|s| {
				parse_color(s, palette).unwrap_or_else(|e| {
					panic!("Theme '{}' syntax fg error: {}: {}", theme_name, scope, e)
				})
			}),
			bg: raw.bg.as_ref().map(|s| {
				parse_color(s, palette).unwrap_or_else(|e| {
					panic!("Theme '{}' syntax bg error: {}: {}", theme_name, scope, e)
				})
			}),
			modifiers: raw
				.modifiers
				.as_ref()
				.map(|s| parse_modifiers(s, theme_name, scope))
				.unwrap_or(Modifier::empty()),
		};
		set_syntax_style(&mut styles, scope, style);
	}
	styles
}

pub(crate) fn parse_modifiers(s: &str, theme_name: &str, scope: &str) -> Modifier {
	let mut modifiers = Modifier::empty();
	for part in s.split('|').map(|s| s.trim()) {
		if part.is_empty() {
			continue;
		}
		match part.to_lowercase().as_str() {
			"bold" => modifiers.insert(Modifier::BOLD),
			"italic" => modifiers.insert(Modifier::ITALIC),
			"underlined" => modifiers.insert(Modifier::UNDERLINED),
			"reversed" => modifiers.insert(Modifier::REVERSED),
			"dim" => modifiers.insert(Modifier::DIM),
			"crossed-out" => modifiers.insert(Modifier::CROSSED_OUT),
			other => panic!(
				"Theme '{}' scope '{}' unknown modifier: '{}'",
				theme_name, scope, other
			),
		}
	}
	modifiers
}

fn set_syntax_style(styles: &mut SyntaxStyles, scope: &str, style: SyntaxStyle) {
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
