//! Key mapping library for xeno.
//!
//! Provides compile-time validated key mappings with support for:
//! - Key patterns: single keys (`a`), combinations (`ctrl-b`), sequences (`g g`)
//! - Key groups: `@upper`, `@lower`, `@alpha`, `@alnum`, `@digit`, `@any`
//! - Derive macro for enum-based action definitions
//! - Configuration file support (TOML, KDL)

pub use config::{Config, DerivedConfig, Item, KeyMapConfig};
pub use keymap::{Error, FromKeyMap, IntoKeyMap, KeyMap, ToKeyMap};
pub use matcher::{ContinuationEntry, ContinuationKind, MatchResult, Matcher};
#[cfg(feature = "derive")]
#[doc(hidden)]
pub use xeno_keymap_derive::KeyMap;
pub use xeno_keymap_parser as parser;

pub mod backend;
pub mod config;
#[cfg(feature = "kdl")]
pub mod kdl;
mod keymap;
mod matcher;
