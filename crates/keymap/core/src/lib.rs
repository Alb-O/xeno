//! Core key mapping types and pattern matching.
//!
//! Provides the foundational types for key mapping:
//! - Key patterns: single keys (`a`), combinations (`ctrl-b`), sequences (`g g`)
//! - Key groups: `@upper`, `@lower`, `@alpha`, `@alnum`, `@digit`, `@any`
//! - Configuration file support (TOML, KDL)

pub use config::{Config, DerivedConfig, Item, KeyMapConfig};
pub use keymap::{Error, FromKeyMap, IntoKeyMap, KeyMap, ToKeyMap};
pub use matcher::{ContinuationEntry, ContinuationKind, MatchResult, Matcher};
pub use xeno_keymap_parser as parser;

pub mod backend;
pub mod config;
#[cfg(feature = "kdl")]
pub mod kdl;
mod keymap;
mod matcher;
