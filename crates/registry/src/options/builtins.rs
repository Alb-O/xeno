//! Built-in option implementations.

#[crate::derive_option]
#[option(kdl = "cursorline", scope = buffer)]
/// Whether to highlight the current line.
pub static CURSORLINE: bool = true;

#[crate::derive_option]
#[option(kdl = "tab-width", scope = buffer)]
/// Number of spaces a tab character occupies.
pub static TAB_WIDTH: i64 = 4;

#[crate::derive_option]
#[option(kdl = "scroll-lines", scope = global)]
/// Number of lines to scroll.
pub static SCROLL_LINES: i64 = 1;

#[crate::derive_option]
#[option(kdl = "scroll-margin", scope = buffer)]
/// Minimum number of lines to keep above/below the cursor.
pub static SCROLL_MARGIN: i64 = 3;

#[crate::derive_option]
#[option(kdl = "theme", scope = global)]
/// Active color theme name.
pub static THEME: String = "monokai".to_string();

#[crate::derive_option]
#[option(kdl = "default-theme-id", scope = global)]
/// Fallback theme ID if preferred theme is unavailable.
pub static DEFAULT_THEME_ID: String = "monokai".to_string();

pub fn register_builtins(builder: &mut crate::db::builder::RegistryDbBuilder) {
	builder.register_option(&__OPT_CURSORLINE);
	builder.register_option(&__OPT_TAB_WIDTH);
	builder.register_option(&__OPT_SCROLL_LINES);
	builder.register_option(&__OPT_SCROLL_MARGIN);
	builder.register_option(&__OPT_THEME);
	builder.register_option(&__OPT_DEFAULT_THEME_ID);
}
