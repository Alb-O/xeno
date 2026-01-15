//! KDL parsing utilities shared across config modules.

use std::collections::HashMap;

use kdl::{KdlDocument, KdlNode};
use xeno_base::{Color, Modifier};

use crate::error::{ConfigError, Result};

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

/// Parse a palette block into the context.
pub fn parse_palette(node: &KdlNode, ctx: &mut ParseContext) -> Result<()> {
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

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn test_parse_hex_color() {
		assert_eq!(parse_hex_color("FF0000").unwrap(), Color::Rgb(255, 0, 0));
		assert_eq!(parse_hex_color("00ff00").unwrap(), Color::Rgb(0, 255, 0));
		assert_eq!(parse_hex_color("F00").unwrap(), Color::Rgb(255, 0, 0));
	}

	#[test]
	fn test_parse_named_color() {
		assert_eq!(parse_named_color("red").unwrap(), Color::Red);
		assert_eq!(parse_named_color("dark-gray").unwrap(), Color::DarkGray);
	}

	#[test]
	fn test_parse_modifier() {
		assert_eq!(parse_modifier("bold").unwrap(), Modifier::BOLD);
		assert_eq!(
			parse_modifier("bold italic").unwrap(),
			Modifier::BOLD | Modifier::ITALIC
		);
	}
}
