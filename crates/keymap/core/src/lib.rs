//! Core key mapping types and pattern matching.
//!
//! Provides the foundational types for key mapping:
//! * Key patterns: single keys (`a`), combinations (`ctrl-b`), sequences (`g g`)
//! * Key groups: `@upper`, `@lower`, `@alpha`, `@alnum`, `@digit`, `@any`

pub use matcher::{ContinuationEntry, ContinuationKind, MatchResult, Matcher};
pub use xeno_keymap_parser as parser;

mod matcher;
