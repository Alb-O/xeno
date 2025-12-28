//! Option implementations.
//!
//! This module contains the built-in option definitions.
//! Type definitions are in evildoer-manifest.

mod behavior;
mod display;
mod file;
mod indent;
mod scroll;
mod search;

// Re-export types from evildoer-manifest for use in option definitions
pub use evildoer_manifest::options::{
	OPTIONS, OptionDef, OptionScope, OptionType, OptionValue, all_options, find_option,
};
