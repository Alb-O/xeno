//! KDL configuration parsing for Xeno.
//!
//! This module provides KDL-specific parsing functions for configuration files.

use std::collections::HashMap;

use kdl::{KdlDocument, KdlNode};

use super::{Config, ConfigWarning, LanguageConfig, Result, UnresolvedKeys};
use crate::options::{OptionScope, OptionStore};

/// Parse a KDL string into a [`Config`].
///
/// Non-fatal warnings (e.g., scope mismatches) are collected in `Config::warnings`
/// rather than causing parse failure. Callers should check and display these.
pub fn parse_config_str(input: &str) -> Result<Config> {
	let doc: KdlDocument = input.parse()?;
	let mut warnings = Vec::new();

	let theme = doc.get("theme").map(parse_theme_node).transpose()?;
	let keys = doc.get("keys").map(parse_keys_node).transpose()?;

	let options = if let Some(opts_node) = doc.get("options") {
		let parsed = parse_options_with_context(opts_node, ParseContext::Global)?;
		warnings.extend(parsed.warnings);
		parsed.store
	} else {
		OptionStore::default()
	};

	let mut languages = Vec::new();
	for node in doc
		.nodes()
		.iter()
		.filter(|n| n.name().value() == "language")
	{
		if let Some(name) = node.get(0).and_then(|v| v.as_string()) {
			let parsed = parse_options_with_context(node, ParseContext::Language)?;
			warnings.extend(parsed.warnings);
			languages.push(LanguageConfig {
				name: name.to_string(),
				options: parsed.store,
			});
		}
	}

	Ok(Config {
		theme,
		keys,
		options,
		languages,
		warnings,
	})
}

/// Parse a standalone theme file (top-level structure).
pub fn parse_theme_standalone_str(input: &str) -> Result<crate::themes::LinkedThemeDef> {
	use crate::config::utils::{ParseContext as ColorContext, parse_palette};
	use crate::themes::theme::LinkedThemeDef;

	let doc: KdlDocument = input.parse()?;
	let mut ctx = ColorContext::default();
	if let Some(node) = doc.get("palette") {
		parse_palette(node, &mut ctx)?;
	}

	let name = doc
		.get_arg("name")
		.and_then(|v| v.as_string())
		.ok_or_else(|| super::ConfigError::MissingField("name".into()))?
		.to_string();

	let variant = doc
		.get_arg("variant")
		.and_then(|v| v.as_string())
		.map(parse_variant)
		.transpose()?
		.unwrap_or_default();

	let keys = doc
		.get("keys")
		.map(|node| {
			node.entries()
				.iter()
				.filter_map(|e| e.value().as_string().map(String::from))
				.collect()
		})
		.unwrap_or_default();

	let ui = parse_ui_colors(doc.get("ui"), &ctx)?;
	let mode = parse_mode_colors(doc.get("mode"), &ctx)?;
	let semantic = parse_semantic_colors(doc.get("semantic"), &ctx)?;
	let popup = parse_popup_colors(doc.get("popup"), &ctx)?;
	let syntax = parse_syntax_styles(doc.get("syntax"), &ctx)?;

	let id = format!("xeno-registry::{}", name);

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

/// Parse a theme from a `theme { }` node in a config file.
fn parse_theme_node(node: &KdlNode) -> Result<crate::themes::LinkedThemeDef> {
	use crate::config::utils::{ParseContext as ColorContext, parse_palette};

	let children = node
		.children()
		.ok_or_else(|| super::ConfigError::MissingField("theme children".into()))?;

	let mut ctx = ColorContext::default();

	if let Some(palette_node) = children.get("palette") {
		parse_palette(palette_node, &mut ctx)?;
	}

	let name = children
		.get_arg("name")
		.and_then(|v| v.as_string())
		.ok_or_else(|| super::ConfigError::MissingField("name".into()))?
		.to_string();

	let variant = children
		.get_arg("variant")
		.and_then(|v| v.as_string())
		.map(parse_variant)
		.transpose()?
		.unwrap_or_default();

	let keys = children
		.get("keys")
		.map(|node| {
			node.entries()
				.iter()
				.filter_map(|e| e.value().as_string().map(String::from))
				.collect()
		})
		.unwrap_or_default();

	let ui = parse_ui_colors(children.get("ui"), &ctx)?;
	let mode = parse_mode_colors(children.get("mode"), &ctx)?;
	let semantic = parse_semantic_colors(children.get("semantic"), &ctx)?;
	let popup = parse_popup_colors(children.get("popup"), &ctx)?;
	let syntax = parse_syntax_styles(children.get("syntax"), &ctx)?;

	let id = format!("xeno-registry::{}", name);

	Ok(crate::themes::LinkedThemeDef {
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

/// Parse a `keys { }` node into [`UnresolvedKeys`].
fn parse_keys_node(node: &KdlNode) -> Result<UnresolvedKeys> {
	let mut config = UnresolvedKeys::default();

	let Some(children) = node.children() else {
		return Ok(config);
	};

	for mode_node in children.nodes() {
		let mode_name = mode_node.name().value().to_string();
		let mut bindings = HashMap::new();

		if let Some(mode_children) = mode_node.children() {
			for binding_node in mode_children.nodes() {
				let key = binding_node.name().value().to_string();
				if let Some(action) = binding_node.get(0).and_then(|v| v.as_string()) {
					bindings.insert(key, action.to_string());
				}
			}
		}

		config.modes.insert(mode_name, bindings);
	}

	Ok(config)
}

/// Context for option parsing - indicates where options are being parsed from.
#[derive(Clone, Copy, PartialEq, Eq)]
enum ParseContext {
	/// Inside a global `options { }` block.
	Global,
	/// Inside a `language "foo" { }` block.
	Language,
}

/// Result of parsing options, including any non-fatal warnings.
#[derive(Debug)]
struct ParsedOptions {
	/// The parsed option store.
	store: OptionStore,
	/// Non-fatal warnings encountered during parsing.
	warnings: Vec<ConfigWarning>,
}

/// Parses options from a KDL node with scope context validation.
fn parse_options_with_context(node: &KdlNode, context: ParseContext) -> Result<ParsedOptions> {
	use crate::options::parse::suggest_option;
	use crate::options::{OptionValue, find};

	let mut store = OptionStore::new();
	let mut warnings = Vec::new();

	let Some(children) = node.children() else {
		return Ok(ParsedOptions { store, warnings });
	};

	for opt_node in children.nodes() {
		let kdl_key = opt_node.name().value();

		let def = find(kdl_key).ok_or_else(|| super::ConfigError::UnknownOption {
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

		let Some(entry) = opt_node.entries().first() else {
			continue;
		};

		let value = entry.value();
		let opt_value = if let Some(b) = value.as_bool() {
			OptionValue::Bool(b)
		} else if let Some(i) = value.as_integer() {
			OptionValue::Int(i as i64)
		} else if let Some(s) = value.as_string() {
			OptionValue::String(s.to_string())
		} else {
			continue;
		};

		if !opt_value.matches_type(def.value_type) {
			return Err(super::ConfigError::OptionTypeMismatch {
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

/// Parses a theme variant string into a `ThemeVariant`.
fn parse_variant(s: &str) -> Result<crate::themes::ThemeVariant> {
	match s.to_lowercase().as_str() {
		"dark" => Ok(crate::themes::ThemeVariant::Dark),
		"light" => Ok(crate::themes::ThemeVariant::Light),
		other => Err(super::ConfigError::InvalidVariant(other.to_string())),
	}
}

/// Returns a human-readable name for an option type.
fn option_type_name(ty: crate::options::OptionType) -> &'static str {
	match ty {
		crate::options::OptionType::Bool => "bool",
		crate::options::OptionType::Int => "int",
		crate::options::OptionType::String => "string",
	}
}

// Re-export theme parsing helpers
use crate::config::utils::{
	parse_mode_colors, parse_popup_colors, parse_semantic_colors, parse_syntax_styles,
	parse_ui_colors,
};
