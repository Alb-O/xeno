//! Built-in option implementations.

use super::TypedOptionKey;
use crate::db::builder::RegistryDbBuilder;

/// Whether to highlight the current line.
pub const CURSORLINE: TypedOptionKey<bool> = TypedOptionKey::new("xeno-registry::cursorline");

/// Number of spaces a tab character occupies.
pub const TAB_WIDTH: TypedOptionKey<i64> = TypedOptionKey::new("xeno-registry::tab_width");

/// Number of lines to scroll.
pub const SCROLL_LINES: TypedOptionKey<i64> = TypedOptionKey::new("xeno-registry::scroll_lines");

/// Minimum number of lines to keep above/below the cursor.
pub const SCROLL_MARGIN: TypedOptionKey<i64> = TypedOptionKey::new("xeno-registry::scroll_margin");

/// Active color theme name.
pub const THEME: TypedOptionKey<String> = TypedOptionKey::new("xeno-registry::theme");

/// Fallback theme ID if preferred theme is unavailable.
pub const DEFAULT_THEME_ID: TypedOptionKey<String> =
	TypedOptionKey::new("xeno-registry::default_theme_id");

// Register standard validators
crate::option_validator!(positive_int, super::validators::positive_int);

pub fn register_builtins(builder: &mut RegistryDbBuilder) {
	let blob = crate::kdl::loader::load_option_metadata();
	let validators = inventory::iter::<crate::options::OptionValidatorReg>
		.into_iter()
		.map(|r| r.0);
	let linked = crate::kdl::link::link_options(&blob, validators);

	for def in linked {
		builder.register_linked_option(def);
	}
}
