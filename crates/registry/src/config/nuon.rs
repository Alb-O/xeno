//! NUON configuration parsing for Xeno.

use std::collections::HashMap;

use nu_protocol::{Record, Value};

use super::{Config, ConfigError, ConfigWarning, LanguageConfig, Result, UnresolvedKeys};
use crate::options::{OptionScope, OptionStore};

/// Parse a NUON string into a [`Config`].
pub fn parse_config_str(input: &str) -> Result<Config> {
	let value = parse_root_value(input)?;
	parse_config_value(&value)
}

/// Parse a NUON value into a [`Config`].
pub fn parse_config_value(value: &Value) -> Result<Config> {
	let root = expect_record(value, "config")?;
	validate_allowed_fields(&root, &["theme", "keys", "options", "languages"], "config")?;

	let mut warnings = Vec::new();

	#[cfg(feature = "themes")]
	let theme = parse_inline_theme(root.get("theme"))?;

	let keys = root.get("keys").map(parse_keys_value).transpose()?;

	let options = if let Some(value) = root.get("options") {
		let parsed = parse_options_with_context(value, ParseContext::Global, "options")?;
		warnings.extend(parsed.warnings);
		parsed.store
	} else {
		OptionStore::default()
	};

	let mut languages = Vec::new();
	if let Some(value) = root.get("languages") {
		for (idx, entry) in expect_list(value, "languages")?.iter().enumerate() {
			let field = format!("languages[{idx}]");
			let lang = expect_record(entry, &field)?;
			validate_allowed_fields(lang, &["name", "options"], &field)?;

			let name_field = format!("{field}.name");
			let name = lang
				.get("name")
				.ok_or_else(|| ConfigError::MissingField(name_field.clone()))
				.and_then(|v| expect_string(v, &name_field))?
				.to_string();

			let options = if let Some(v) = lang.get("options") {
				let parsed = parse_options_with_context(v, ParseContext::Language, &format!("{field}.options"))?;
				warnings.extend(parsed.warnings);
				parsed.store
			} else {
				OptionStore::default()
			};

			languages.push(LanguageConfig { name, options });
		}
	}

	Ok(Config {
		#[cfg(feature = "themes")]
		theme,
		keys,
		options,
		languages,
		warnings,
	})
}

/// Parse a standalone NUON theme file.
pub fn parse_theme_standalone_str(input: &str) -> Result<crate::themes::LinkedThemeDef> {
	let value = parse_root_value(input)?;
	parse_theme_value(&value)
}

/// Parse a NUON value into a standalone theme definition.
pub fn parse_theme_value(value: &Value) -> Result<crate::themes::LinkedThemeDef> {
	use crate::config::utils::{ParseContext as ColorContext, parse_modifier};
	use crate::themes::theme::LinkedThemeDef;

	let root = expect_record(value, "theme")?;
	validate_allowed_fields(
		&root,
		&["name", "variant", "keys", "palette", "ui", "mode", "semantic", "popup", "syntax"],
		"theme",
	)?;

	let mut ctx = ColorContext::default();
	if let Some(value) = root.get("palette") {
		let palette = expect_record(value, "palette")?;
		for (name, color) in palette.iter() {
			let color = expect_string(color, &format!("palette.{name}"))?;
			ctx.palette.insert(name.clone(), crate::config::utils::parse_color(color)?);
		}
	}

	let name = root
		.get("name")
		.ok_or_else(|| ConfigError::MissingField("name".into()))
		.and_then(|v| expect_string(v, "name"))?
		.to_string();

	let variant = root
		.get("variant")
		.map(|v| expect_string(v, "variant").and_then(parse_variant))
		.transpose()?
		.unwrap_or_default();

	let keys = if let Some(v) = root.get("keys") {
		expect_list(v, "keys")?
			.iter()
			.enumerate()
			.map(|(idx, entry)| expect_string(entry, &format!("keys[{idx}]")))
			.collect::<Result<Vec<_>>>()?
			.into_iter()
			.map(str::to_string)
			.collect()
	} else {
		Vec::new()
	};

	let ui = parse_ui_colors(root.get("ui"), &ctx)?;
	let mode = parse_mode_colors(root.get("mode"), &ctx)?;
	let semantic = parse_semantic_colors(root.get("semantic"), &ctx)?;
	let popup = parse_popup_colors(root.get("popup"), &ctx)?;
	let syntax = parse_syntax_styles(root.get("syntax"), &ctx, parse_modifier)?;

	let id = format!("xeno-registry::{name}");

	Ok(LinkedThemeDef {
		meta: crate::core::LinkedMetaOwned {
			id,
			name,
			keys,
			description: String::new(),
			priority: 0,
			flags: 0,
			source: crate::core::RegistrySource::Runtime,
			required_caps: Vec::new(),
			short_desc: None,
		},
		payload: crate::themes::theme::ThemePayload {
			variant,
			colors: crate::themes::ThemeColors {
				ui,
				mode,
				semantic,
				popup,
				notification: crate::themes::NotificationColors::INHERITED,
				syntax,
			},
		},
	})
}

fn parse_root_value(input: &str) -> Result<Value> {
	nuon::from_nuon(input, None).map_err(|e| ConfigError::Nuon(e.to_string()))
}

#[cfg(feature = "themes")]
fn parse_inline_theme(value: Option<&Value>) -> Result<Option<crate::themes::LinkedThemeDef>> {
	let Some(value) = value else {
		return Ok(None);
	};

	if value.is_nothing() {
		Ok(None)
	} else {
		Err(invalid_type("theme", "nothing", value))
	}
}

fn validate_allowed_fields(record: &Record, allowed: &[&str], parent: &str) -> Result<()> {
	for (field, _) in record.iter() {
		if !allowed.iter().any(|k| k == field) {
			return Err(ConfigError::UnknownField(format!("{parent}.{field}")));
		}
	}
	Ok(())
}

fn expect_record<'a>(value: &'a Value, field: &str) -> Result<&'a Record> {
	value.as_record().map_err(|_| invalid_type(field, "record", value))
}

fn expect_list<'a>(value: &'a Value, field: &str) -> Result<&'a [Value]> {
	value.as_list().map_err(|_| invalid_type(field, "list", value))
}

fn expect_string<'a>(value: &'a Value, field: &str) -> Result<&'a str> {
	value.as_str().map_err(|_| invalid_type(field, "string", value))
}

fn invalid_type(field: &str, expected: &'static str, value: &Value) -> ConfigError {
	ConfigError::InvalidType {
		field: field.to_string(),
		expected,
		got: value.get_type().to_string(),
	}
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum ParseContext {
	Global,
	Language,
}

#[derive(Debug)]
struct ParsedOptions {
	store: OptionStore,
	warnings: Vec<ConfigWarning>,
}

fn parse_options_with_context(value: &Value, context: ParseContext, field: &str) -> Result<ParsedOptions> {
	use crate::options::find;
	use crate::options::parse::suggest_option;

	let mut store = OptionStore::new();
	let mut warnings = Vec::new();
	let record = expect_record(value, field)?;

	for (kdl_key, raw_value) in record.iter() {
		let def = find(kdl_key).ok_or_else(|| ConfigError::UnknownOption {
			key: kdl_key.to_string(),
			suggestion: suggest_option(kdl_key),
		})?;

		if context == ParseContext::Language && def.scope == OptionScope::Global {
			warnings.push(ConfigWarning::ScopeMismatch {
				option: kdl_key.to_string(),
				found_in: "language block",
				expected: "global options block",
			});
			continue;
		}

		let opt_value = value_to_option_value(raw_value).ok_or_else(|| ConfigError::OptionTypeMismatch {
			option: kdl_key.to_string(),
			expected: option_type_name(def.value_type),
			got: option_value_type(raw_value),
		})?;

		if !opt_value.matches_type(def.value_type) {
			return Err(ConfigError::OptionTypeMismatch {
				option: kdl_key.to_string(),
				expected: option_type_name(def.value_type),
				got: opt_value.type_name(),
			});
		}

		if let Err(e) = crate::options::validate(kdl_key, &opt_value) {
			eprintln!("Warning: {e}");
			continue;
		}

		let _ = store.set_by_kdl(&crate::options::OPTIONS, kdl_key, opt_value);
	}

	Ok(ParsedOptions { store, warnings })
}

fn value_to_option_value(value: &Value) -> Option<crate::options::OptionValue> {
	if let Ok(v) = value.as_bool() {
		return Some(crate::options::OptionValue::Bool(v));
	}
	if let Ok(v) = value.as_int() {
		return Some(crate::options::OptionValue::Int(v));
	}
	if let Ok(v) = value.as_str() {
		return Some(crate::options::OptionValue::String(v.to_string()));
	}
	None
}

fn option_value_type(value: &Value) -> &'static str {
	if value.as_bool().is_ok() {
		"bool"
	} else if value.as_int().is_ok() {
		"int"
	} else if value.as_str().is_ok() {
		"string"
	} else {
		"value"
	}
}

fn option_type_name(ty: crate::options::OptionType) -> &'static str {
	match ty {
		crate::options::OptionType::Bool => "bool",
		crate::options::OptionType::Int => "int",
		crate::options::OptionType::String => "string",
	}
}

fn parse_keys_value(value: &Value) -> Result<UnresolvedKeys> {
	let mut config = UnresolvedKeys::default();
	let modes = expect_record(value, "keys")?;

	for (mode_name, mode_value) in modes.iter() {
		let mode_field = format!("keys.{mode_name}");
		let binding_record = expect_record(mode_value, &mode_field)?;
		let mut bindings = HashMap::new();
		for (key, action_value) in binding_record.iter() {
			let action = expect_string(action_value, &format!("{mode_field}.{key}"))?;
			bindings.insert(key.clone(), action.to_string());
		}
		config.modes.insert(mode_name.clone(), bindings);
	}

	Ok(config)
}

fn parse_variant(s: &str) -> Result<crate::themes::ThemeVariant> {
	match s.to_ascii_lowercase().as_str() {
		"dark" => Ok(crate::themes::ThemeVariant::Dark),
		"light" => Ok(crate::themes::ThemeVariant::Light),
		other => Err(ConfigError::InvalidVariant(other.to_string())),
	}
}

fn color_field(record: &Record, field: &str, ctx: &crate::config::utils::ParseContext) -> Result<xeno_primitives::Color> {
	let value = record
		.get(field)
		.ok_or_else(|| ConfigError::MissingField(field.to_string()))
		.and_then(|v| expect_string(v, field))?;
	ctx.resolve_color(value)
}

fn color_field_opt(record: &Record, field: &str, ctx: &crate::config::utils::ParseContext) -> Result<Option<xeno_primitives::Color>> {
	match record.get(field) {
		Some(v) => expect_string(v, field).and_then(|s| ctx.resolve_color(s)).map(Some),
		None => Ok(None),
	}
}

fn parse_ui_colors(node: Option<&Value>, ctx: &crate::config::utils::ParseContext) -> Result<crate::themes::UiColors> {
	let node = node.ok_or_else(|| ConfigError::MissingField("ui".into()))?;
	let record = expect_record(node, "ui")?;

	let bg = color_field(record, "bg", ctx)?;
	let nontext_bg = color_field_opt(record, "nontext-bg", ctx)?.unwrap_or_else(|| bg.blend(xeno_primitives::Color::Black, 0.85));

	Ok(crate::themes::UiColors {
		bg,
		fg: color_field(record, "fg", ctx)?,
		nontext_bg,
		gutter_fg: color_field(record, "gutter-fg", ctx)?,
		cursor_bg: color_field(record, "cursor-bg", ctx)?,
		cursor_fg: color_field(record, "cursor-fg", ctx)?,
		cursorline_bg: color_field(record, "cursorline-bg", ctx)?,
		selection_bg: color_field(record, "selection-bg", ctx)?,
		selection_fg: color_field(record, "selection-fg", ctx)?,
		message_fg: color_field(record, "message-fg", ctx)?,
		command_input_fg: color_field(record, "command-input-fg", ctx)?,
	})
}

fn parse_mode_colors(node: Option<&Value>, ctx: &crate::config::utils::ParseContext) -> Result<crate::themes::ModeColors> {
	let node = node.ok_or_else(|| ConfigError::MissingField("mode".into()))?;
	let record = expect_record(node, "mode")?;

	let parse_pair = |prefix: &str| -> Result<crate::themes::ColorPair> {
		Ok(crate::themes::ColorPair {
			bg: color_field(record, &format!("{prefix}-bg"), ctx)?,
			fg: color_field(record, &format!("{prefix}-fg"), ctx)?,
		})
	};

	Ok(crate::themes::ModeColors {
		normal: parse_pair("normal")?,
		insert: parse_pair("insert")?,
		prefix: parse_pair("prefix")?,
		command: parse_pair("command")?,
	})
}

fn parse_semantic_colors(node: Option<&Value>, ctx: &crate::config::utils::ParseContext) -> Result<crate::themes::SemanticColors> {
	let node = node.ok_or_else(|| ConfigError::MissingField("semantic".into()))?;
	let record = expect_record(node, "semantic")?;

	Ok(crate::themes::SemanticColors {
		error: color_field(record, "error", ctx)?,
		warning: color_field(record, "warning", ctx)?,
		success: color_field(record, "success", ctx)?,
		info: color_field(record, "info", ctx)?,
		hint: color_field(record, "hint", ctx)?,
		dim: color_field(record, "dim", ctx)?,
		link: color_field(record, "link", ctx)?,
		match_hl: color_field(record, "match", ctx)?,
		accent: color_field(record, "accent", ctx)?,
	})
}

fn parse_popup_colors(node: Option<&Value>, ctx: &crate::config::utils::ParseContext) -> Result<crate::themes::PopupColors> {
	let node = node.ok_or_else(|| ConfigError::MissingField("popup".into()))?;
	let record = expect_record(node, "popup")?;

	Ok(crate::themes::PopupColors {
		bg: color_field(record, "bg", ctx)?,
		fg: color_field(record, "fg", ctx)?,
		border: color_field(record, "border", ctx)?,
		title: color_field(record, "title", ctx)?,
	})
}

fn parse_syntax_styles(
	node: Option<&Value>,
	ctx: &crate::config::utils::ParseContext,
	parse_modifier: fn(&str) -> Result<xeno_primitives::Modifier>,
) -> Result<crate::themes::SyntaxStyles> {
	let Some(node) = node else {
		return Ok(crate::themes::SyntaxStyles::minimal());
	};
	let record = expect_record(node, "syntax")?;

	let mut styles = crate::themes::SyntaxStyles::minimal();
	for (name, value) in record.iter() {
		parse_syntax_node(name, value, "", &mut styles, ctx, parse_modifier)?;
	}
	Ok(styles)
}

fn parse_syntax_node(
	name: &str,
	value: &Value,
	prefix: &str,
	styles: &mut crate::themes::SyntaxStyles,
	ctx: &crate::config::utils::ParseContext,
	parse_modifier: fn(&str) -> Result<xeno_primitives::Modifier>,
) -> Result<()> {
	let scope = if prefix.is_empty() { name.to_string() } else { format!("{prefix}.{name}") };

	let node = expect_record(value, &scope)?;
	let mut style = crate::themes::SyntaxStyle::NONE;

	if let Some(v) = node.get("fg") {
		style.fg = Some(ctx.resolve_color(expect_string(v, &format!("{scope}.fg"))?)?);
	}
	if let Some(v) = node.get("bg") {
		style.bg = Some(ctx.resolve_color(expect_string(v, &format!("{scope}.bg"))?)?);
	}
	if let Some(v) = node.get("mod").or_else(|| node.get("modifiers")) {
		style.modifiers = parse_modifier(expect_string(v, &format!("{scope}.mod"))?)?;
	}

	crate::config::utils::set_syntax_style(styles, &scope, style);

	for (child, child_value) in node.iter() {
		if matches!(child.as_str(), "fg" | "bg" | "mod" | "modifiers") {
			continue;
		}
		parse_syntax_node(child, child_value, &scope, styles, ctx, parse_modifier)?;
	}

	Ok(())
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn parse_config_supports_options_languages_and_keys() {
		let input = r#"
{
	options: {
		tab-width: 4,
		theme: "gruvbox",
	},
	languages: [
		{ name: "rust", options: { tab-width: 2, theme: "monokai" } },
	],
	keys: {
		normal: { "ctrl+s": "command:write" }
	}
}
"#;

		let config = parse_config_str(input).expect("config should parse");

		assert_eq!(config.languages.len(), 1);
		assert_eq!(config.languages[0].name, "rust");
		assert_eq!(config.warnings.len(), 1);
		assert!(matches!(
			&config.warnings[0],
			ConfigWarning::ScopeMismatch {
				option,
				found_in: "language block",
				expected: "global options block"
			} if option == "theme"
		));

		let keys = config.keys.expect("keys should be parsed");
		assert_eq!(
			keys.modes.get("normal").and_then(|m| m.get("ctrl+s")).map(String::as_str),
			Some("command:write")
		);
	}

	#[test]
	fn parse_config_rejects_unknown_top_level_field() {
		let input = r#"{ foo: 1 }"#;
		let err = parse_config_str(input).expect_err("unknown field should fail");
		assert!(matches!(err, ConfigError::UnknownField(field) if field == "config.foo"));
	}

	#[test]
	fn parse_theme_standalone_supports_nuon() {
		let input = r##"
{
	name: "nuon-demo",
	variant: "dark",
	palette: {
		base: "#101010",
		fg: "#f0f0f0",
	},
	ui: {
		bg: "$base",
		fg: "$fg",
		nontext-bg: "#0a0a0a",
		gutter-fg: "gray",
		cursor-bg: "white",
		cursor-fg: "black",
		cursorline-bg: "#202020",
		selection-bg: "blue",
		selection-fg: "white",
		message-fg: "yellow",
		command-input-fg: "white",
	},
	mode: {
		normal-bg: "blue",
		normal-fg: "white",
		insert-bg: "green",
		insert-fg: "black",
		prefix-bg: "magenta",
		prefix-fg: "white",
		command-bg: "yellow",
		command-fg: "black",
	},
	semantic: {
		error: "red",
		warning: "yellow",
		success: "green",
		info: "cyan",
		hint: "dark-gray",
		dim: "dark-gray",
		link: "cyan",
		match: "green",
		accent: "cyan",
	},
	popup: {
		bg: "#111111",
		fg: "white",
		border: "white",
		title: "yellow",
	},
}
"##;

		let theme = parse_theme_standalone_str(input).expect("theme should parse");
		assert_eq!(theme.meta.name, "nuon-demo");
		assert_eq!(theme.meta.id, "xeno-registry::nuon-demo");
		assert!(matches!(theme.payload.variant, crate::themes::ThemeVariant::Dark));
	}
}
