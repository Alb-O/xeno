//! Option implementations.
//!
//! This module contains the built-in option definitions.
//! Type definitions are in tome-manifest.

mod behavior;
mod display;
mod file;
mod indent;
mod scroll;
mod search;

// Re-export types from tome-manifest for use in option definitions
pub use tome_manifest::options::{
	OPTIONS, OptionDef, OptionScope, OptionType, OptionValue, all_options, find_option,
};
