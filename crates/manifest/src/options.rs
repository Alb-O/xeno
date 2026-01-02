//! Options/settings system type definitions.
//!
//! Re-exports from [`evildoer_registry::options`] for backward compatibility.

pub use evildoer_registry::options::{
	all, find, OptionDef, OptionScope, OptionType, OptionValue, OPTIONS,
};

/// Backward-compatible alias for `find`.
pub fn find_option(name: &str) -> Option<&'static OptionDef> {
	find(name)
}

/// Backward-compatible alias for `all`.
pub fn all_options() -> &'static [OptionDef] {
	all()
}
