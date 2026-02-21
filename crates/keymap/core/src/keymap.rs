//! Unified abstraction over key input representations.
//!
//! Provides a backend-agnostic `KeyMap` type (aliased from [`Node`]) and the
//! [`ToKeyMap`] trait for converting backend-specific key events into it.

use xeno_keymap_parser::Node;
use xeno_keymap_parser::parser::ParseError;

/// A type alias for a parsed keymap node tree.
///
/// This represents a keymap in an abstract format, using the [`Node`] type
/// from the `keymap_parser` crate.
pub type KeyMap = Node;

/// A trait for converting a backend-specific key type into a [`KeyMap`].
///
/// This is typically implemented by types like `xeno_primitives::Key`,
/// allowing the transformation of native input representations into the
/// abstract `KeyMap` format used for keybinding configuration and matching.
///
/// # Errors
///
/// Returns an [`Error`] if the conversion fails due to unsupported or unrepresentable keys.
pub trait ToKeyMap {
	/// Converts the type into a [`KeyMap`].
	///
	/// # Errors
	///
	/// Returns an [`Error`] if conversion fails due to unsupported or invalid keys.
	fn to_keymap(&self) -> Result<KeyMap, Error>;
}

/// Represents errors that can occur during keymap parsing or conversion.
#[derive(Debug)]
pub enum Error {
	/// A parsing error occurred while processing a `KeyMap`.
	Parse(ParseError),
}

impl std::fmt::Display for Error {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		match self {
			Error::Parse(e) => write!(f, "{e}"),
		}
	}
}

impl std::error::Error for Error {
	fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
		match self {
			Error::Parse(e) => Some(e),
		}
	}
}
