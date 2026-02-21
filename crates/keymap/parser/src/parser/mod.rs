//! # Parser
//!
//! This module defines functionality for parsing plain-text keymap definitions into structured
//! representations. It supports sequences such as `"ctrl-alt-f1"` or `"a b"` and maps them to
//! key/modifier combinations.
//!
//! ## Supported Syntax
//!
//! ```text
//! node      = modifiers* key
//! modifiers = modifier "-"
//! modifier  = "ctrl" | "cmd" | "alt" | "shift"
//! key       = fn-key | named-key | group | char
//! fn-key    = "f" digit digit?
//! named-key = "del" | "insert" | "end" | ...
//! group     = "@" ("digit" | "lower" | "upper" | "alnum" | "alpha" | "any")
//! char      = ascii-char
//! ```
//!
//! Each `Node` consists of optional modifier keys followed by a key identifier.

use std::str::FromStr;

use crate::node::{CharGroup, KEY_SEP, Key, Modifier, Node};

#[cfg(test)]
mod tests;

/// Function pointer type for parser combinators.
type ParserFn<T> = fn(&mut Parser) -> Result<Option<T>, ParseError>;

/// Represents an error that occurred during parsing.
#[derive(Debug, PartialEq, Clone)]
pub struct ParseError {
	/// Human-readable description of the parse error.
	pub message: String,
	/// Byte offset in the input where the error occurred.
	pub position: usize,
}

impl std::fmt::Display for ParseError {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		write!(f, "Parse error at position {}: {}", self.position, self.message)
	}
}

impl std::error::Error for ParseError {}

/// Maintains the parser's state for recursive descent parsing.
struct Parser<'a> {
	/// The input string being parsed.
	input: &'a str,
	/// Current byte position in the input.
	position: usize,
}

impl<'a> Parser<'a> {
	/// Creates a new `Parser` from the given input string.
	fn new(input: &'a str) -> Self {
		Self { input, position: 0 }
	}

	/// Peeks at the next character without consuming it.
	fn peek(&self) -> Option<char> {
		self.input.chars().next()
	}

	/// Peeks at the character `n` positions ahead without consuming it.
	fn peek_at(&self, n: usize) -> Option<char> {
		self.input.chars().nth(n)
	}

	/// Consumes and returns the next character, advancing the parser.
	fn next(&mut self) -> Option<char> {
		if let Some(ch) = self.peek() {
			self.position += ch.len_utf8();
			self.input = &self.input[ch.len_utf8()..];

			Some(ch)
		} else {
			None
		}
	}

	/// Returns `true` if the parser has consumed all input.
	fn is_end(&self) -> bool {
		self.input.is_empty()
	}

	/// Consumes the next character if it matches the expected one.
	///
	/// # Errors
	///
	/// Returns a [`ParseError`] if the character doesn't match or if input is exhausted.
	fn take(&mut self, expected: char) -> Result<(), ParseError> {
		match self.next() {
			Some(ch) if ch == expected => Ok(()),
			Some(ch) => Err(ParseError {
				message: format!("expected '{expected}', found '{ch}'"),
				position: self.position - ch.len_utf8(),
			}),
			None => Err(ParseError {
				message: format!("expected '{expected}', found end of input"),
				position: self.position,
			}),
		}
	}

	/// Attempts to parse with a fallback: restores state if parsing fails.
	///
	/// Returns `Ok(Some(value))` if successful, or `Ok(None)` on failure.
	fn try_parse<T, F>(&mut self, f: F) -> Result<Option<T>, ParseError>
	where
		F: FnOnce(&mut Parser<'a>) -> Result<Option<T>, ParseError>,
	{
		let snapshot = (self.input, self.position);
		match f(self) {
			Ok(Some(val)) => Ok(Some(val)),
			Ok(None) | Err(_) => {
				self.input = snapshot.0;
				self.position = snapshot.1;
				Ok(None)
			}
		}
	}

	/// Consumes and returns characters that satisfy a predicate.
	fn take_while<F>(&mut self, predicate: F) -> String
	where
		F: Fn(char) -> bool,
	{
		let mut result = String::new();

		while let Some(ch) = self.peek() {
			if predicate(ch) {
				result.push(ch);
				self.next();
			} else {
				break;
			}
		}

		result
	}

	/// Tries multiple parsers in sequence, returning the result of the first successful one.
	fn alt<T>(&mut self, parsers: &[ParserFn<T>]) -> Result<Option<T>, ParseError> {
		for p in parsers {
			match p(self)? {
				Some(value) => return Ok(Some(value)),
				None => continue,
			}
		}

		Ok(None)
	}

	/// Creates a [`ParseError`] with the current parser position.
	fn error(&self, message: String) -> ParseError {
		ParseError {
			message,
			position: self.position,
		}
	}
}

/// Parses a single key expression into a [`Node`].
///
/// Accepts strings like `"ctrl-b"`, `"@digit"`, or `"f1"`.
///
/// # Errors
///
/// Returns a [`ParseError`] if the input does not match the expected grammar.
///
/// # Examples
///
/// ```
/// use xeno_keymap_parser::{parse, Node, Key, Modifier};
///
/// let node = parse("ctrl-a").unwrap();
/// assert_eq!(node, Node::new(Modifier::Ctrl as u8, Key::Char('a')));
/// ```
pub fn parse(s: &str) -> Result<Node, ParseError> {
	let mut parser = Parser::new(s);
	let node = parse_node(&mut parser)?;

	if !parser.is_end() {
		return Err(parser.error(format!("expect end of input, found: {}", parser.peek().unwrap())));
	}

	Ok(node)
}

/// Parses a key combination with optional modifiers followed by a key.
///
/// Grammar: `node = modifiers* key`
fn parse_node(parser: &mut Parser) -> Result<Node, ParseError> {
	let mut modifiers = 0u8;

	for _ in 0..4 {
		if let Some(modifier) = try_parse_modifier(parser)? {
			modifiers |= modifier as u8;
		} else {
			break;
		}
	}

	let key = parse_key(parser)?;
	Ok(Node::new(modifiers, key))
}

/// Attempts to parse a single modifier, followed by a `-`.
///
/// Returns `None` if no valid modifier is found.
fn try_parse_modifier(parser: &mut Parser) -> Result<Option<Modifier>, ParseError> {
	parser.try_parse(|p| {
		let name = p.take_while(|ch| ch.is_ascii_alphabetic());
		let Ok(modifier) = name.parse::<Modifier>() else {
			return Ok(None);
		};

		p.take(KEY_SEP)?;

		Ok(Some(modifier))
	})
}

/// Parses a key value, which may be a function key, named key, character group, or ASCII char.
fn parse_key(parser: &mut Parser) -> Result<Key, ParseError> {
	match parser.alt(&[try_parse_fn_key, try_parse_named_key, try_parse_group, try_parse_char])? {
		Some(key) => Ok(key),
		None => Err(parser.error("expected a valid key".to_string())),
	}
}

/// Attempts to parse a function key (e.g., `"f1"` to `"f35"`).
///
/// Only activates when the input starts with `f` followed by a digit.
/// Once activated, the digits must form a valid function key number (1-35)
/// or an error is returned (no silent degradation to a char key).
fn try_parse_fn_key(parser: &mut Parser) -> Result<Option<Key>, ParseError> {
	if parser.peek() != Some('f') {
		return Ok(None);
	}

	if !matches!(parser.peek_at(1), Some(ch) if ch.is_ascii_digit()) {
		return Ok(None);
	}

	parser.take('f')?;
	let num = parser.take_while(|ch| ch.is_ascii_digit());

	match num.parse::<u8>() {
		Ok(n) if (1..=35).contains(&n) => Ok(Some(Key::F(n))),
		_ => Err(parser.error("invalid function key number (must be 1-35)".to_string())),
	}
}

/// Attempts to parse a named key such as `"del"`, `"insert"`, or `"end"`.
fn try_parse_named_key(parser: &mut Parser) -> Result<Option<Key>, ParseError> {
	parser.try_parse(|p| {
		let name = p.take_while(|ch| ch.is_ascii_alphabetic());
		if name.len() < 2 {
			return Ok(None);
		}

		match name.parse::<Key>() {
			Ok(key) => Ok(Some(key)),
			Err(_) => Ok(None),
		}
	})
}

/// Attempts to parse a character group like `"@digit"` or `"@lower"`.
fn try_parse_group(parser: &mut Parser) -> Result<Option<Key>, ParseError> {
	if parser.peek() != Some('@') || parser.peek_at(1).is_none() {
		return Ok(None);
	}

	parser.take('@')?;

	let group_name = parser.take_while(|ch| ch.is_ascii_alphabetic());
	let group = match group_name.parse::<CharGroup>() {
		Ok(group) => Key::Group(group),
		Err(_) => return Err(parser.error(format!("unknown char group: '@{group_name}'"))),
	};

	Ok(Some(group))
}

/// Attempts to parse a single ASCII character as a key.
fn try_parse_char(parser: &mut Parser) -> Result<Option<Key>, ParseError> {
	if let Some(ch) = parser.peek() {
		if ch.is_ascii() {
			parser.next();
			Ok(Some(Key::Char(ch)))
		} else {
			Ok(None)
		}
	} else {
		Ok(None)
	}
}

/// Parses a whitespace-separated sequence of key expressions.
///
/// Each part is parsed as a [`Node`].
///
/// # Errors
///
/// Returns a [`ParseError`] if any segment fails to parse.
///
/// # Examples
///
/// ```
/// use xeno_keymap_parser::{parse_seq, Key, Node};
///
/// let seq = parse_seq("a b").unwrap();
/// assert_eq!(seq, vec![Node::from(Key::Char('a')), Node::from(Key::Char('b'))]);
/// ```
pub fn parse_seq(s: &str) -> Result<Vec<Node>, ParseError> {
	str::split_whitespace(s).map(parse).collect()
}

impl FromStr for Node {
	type Err = ParseError;

	fn from_str(s: &str) -> Result<Self, Self::Err> {
		parse(s)
	}
}
