//! Key mapping library for evildoer.
//!
//! Provides compile-time validated key mappings with support for:
//! - Key patterns: single keys (`a`), combinations (`ctrl-b`), sequences (`g g`)
//! - Key groups: `@upper`, `@lower`, `@alpha`, `@alnum`, `@digit`, `@any`
//! - Derive macro for enum-based action definitions
//! - Configuration file support (TOML, KDL)

pub use config::{Config, DerivedConfig, Item, KeyMapConfig};
#[cfg(feature = "derive")]
#[doc(hidden)]
pub use evildoer_keymap_derive::KeyMap;
pub use evildoer_keymap_parser as parser;
pub use keymap::{Error, FromKeyMap, IntoKeyMap, KeyMap, ToKeyMap};
pub use matcher::Matcher;

pub mod backend;
pub mod config;
mod keymap;
mod matcher;
