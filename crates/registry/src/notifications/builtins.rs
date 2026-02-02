//! Built-in notification definitions.

use std::path::Path;
use std::time::Duration;

use crate::notifications::AutoDismiss;

// --- Actions ---
notif!(unknown_action(name: &str), Error, format!("Unknown action: {}", name));
notif!(action_error(err: impl core::fmt::Display), Error, err.to_string());

// --- Commands ---
notif!(
	unsaved_changes_force_quit,
	Error,
	"Buffer has unsaved changes (use :q! to force quit)"
);
notif!(no_collisions, Info, "All good! No collisions found.");
notif!(unknown_command(cmd: &str), Error, format!("Unknown command: {}", cmd));
notif!(command_error(err: &str), Error, format!("Command failed: {}", err));
notif!(
	not_implemented(feature: &str),
	Warn,
	format!("{} - not yet implemented", feature)
);
notif!(theme_set(name: &str), Info, format!("Theme set to '{}'", name));
notif!(
	help_text(text: impl Into<String>),
	Info,
	text,
	auto_dismiss: AutoDismiss::Never
);
notif!(
	diagnostic_output(text: impl Into<String>),
	Info,
	text,
	auto_dismiss: AutoDismiss::Never
);
notif!(
	diagnostic_warning(text: impl Into<String>),
	Warn,
	text,
	auto_dismiss: AutoDismiss::Never
);

// --- Runtime ---
notif!(
	viewport_unavailable,
	Error,
	"Viewport info unavailable for screen motion"
);
notif_alias!(
	viewport_height_unavailable,
	viewport_unavailable,
	"Viewport height unavailable for screen motion"
);
notif!(
	screen_motion_unavailable,
	Error,
	"Screen motion target is unavailable"
);
notif!(pending_prompt(prompt: &str), Info, prompt.to_string());
notif!(count_display(count: usize), Info, count.to_string());

// --- Editor ---
notif!(buffer_readonly, Warn, "Buffer is read-only");
notif!(buffer_modified, Warn, "Buffer has unsaved changes");
notif!(no_buffers, Warn, "No buffers open");
notif!(readonly_enabled, Info, "Read-only enabled");
notif!(readonly_disabled, Info, "Read-only disabled");
notif!(nothing_to_undo, Warn, "Nothing to undo");
notif!(nothing_to_redo, Warn, "Nothing to redo");
notif!(undo, Info, "Undo");
notif!(redo, Info, "Redo");
notif!(no_selection, Warn, "No selection");
notif_alias!(
	no_selection_to_search,
	no_selection,
	"No selection to search in"
);
notif_alias!(no_selection_to_split, no_selection, "No selection to split");
notif!(no_selections_remain, Warn, "No selections remain");
notif!(search_wrapped, Info, "Search wrapped to beginning");
notif!(no_search_pattern, Warn, "No search pattern");
notif!(no_more_matches, Warn, "No more matches");
notif!(no_matches_found, Warn, "No matches found");
notif!(pattern_not_found, Warn, "Pattern not found");
notif!(
	pattern_not_found_with(pattern: &str),
	Warn,
	format!("Pattern '{}' not found", pattern)
);
notif!(
	regex_error(err: &str),
	Error,
	format!("Regex error: {}", err),
	auto_dismiss: AutoDismiss::After(Duration::from_secs(8))
);
notif!(search_info(text: &str), Info, format!("Search: {}", text));
notif!(replaced(count: usize), Info, format!("Replaced {} occurrences", count));
notif!(matches_count(count: usize), Info, format!("{} matches", count));
notif!(splits_count(count: usize), Info, format!("{} splits", count));
notif!(selections_kept(count: usize), Info, format!("{} selections kept", count));
notif!(split_no_ranges, Warn, "Split produced no ranges");
notif!(no_matches_to_split, Warn, "No matches found to split on");
notif!(yanked_chars(count: usize), Info, format!("Yanked {} chars", count));
notif!(yanked_lines(count: usize), Info, format!("Yanked {} lines", count));
notif!(deleted_chars(count: usize), Info, format!("Deleted {} chars", count));
notif!(file_saved(path: &Path), Success, format!("Saved {}", path.display()));
notif!(file_not_found(path: &Path), Error, format!("File not found: {}", path.display()));
notif!(file_load_error(err: &str), Error, format!("Failed to load file: {}", err));
notif!(file_save_error(err: &str), Error, format!("Failed to save: {}", err));
notif!(buffer_closed(name: &str), Info, format!("Closed {}", name));
notif!(option_set(key: &str, value: &str), Info, format!("{}={}", key, value));
notif!(unhandled_result(variant: &str), Warn, format!("Unhandled action result: {}", variant));

// --- Generic ---
notif!(info(msg: impl Into<String>), Info, msg);
notif!(warn(msg: impl Into<String>), Warn, msg);
notif!(error(msg: impl Into<String>), Error, msg);
notif!(success(msg: impl Into<String>), Success, msg);
notif!(debug(msg: impl Into<String>), Debug, msg);

// --- Buffer Sync ---
notif!(sync_taking_ownership, Info, "Taking ownership...");
notif!(sync_ownership_denied, Info, "Ownership denied.");

pub fn register_builtins(builder: &mut crate::db::builder::RegistryDbBuilder) {
	builder.register_notification(&NOTIF_UNKNOWN_ACTION);
	builder.register_notification(&NOTIF_ACTION_ERROR);
	builder.register_notification(&NOTIF_UNSAVED_CHANGES_FORCE_QUIT);
	builder.register_notification(&NOTIF_NO_COLLISIONS);
	builder.register_notification(&NOTIF_UNKNOWN_COMMAND);
	builder.register_notification(&NOTIF_COMMAND_ERROR);
	builder.register_notification(&NOTIF_NOT_IMPLEMENTED);
	builder.register_notification(&NOTIF_THEME_SET);
	builder.register_notification(&NOTIF_HELP_TEXT);
	builder.register_notification(&NOTIF_DIAGNOSTIC_OUTPUT);
	builder.register_notification(&NOTIF_DIAGNOSTIC_WARNING);
	builder.register_notification(&NOTIF_VIEWPORT_UNAVAILABLE);
	builder.register_notification(&NOTIF_SCREEN_MOTION_UNAVAILABLE);
	builder.register_notification(&NOTIF_PENDING_PROMPT);
	builder.register_notification(&NOTIF_COUNT_DISPLAY);
	builder.register_notification(&NOTIF_BUFFER_READONLY);
	builder.register_notification(&NOTIF_BUFFER_MODIFIED);
	builder.register_notification(&NOTIF_NO_BUFFERS);
	builder.register_notification(&NOTIF_READONLY_ENABLED);
	builder.register_notification(&NOTIF_READONLY_DISABLED);
	builder.register_notification(&NOTIF_NOTHING_TO_UNDO);
	builder.register_notification(&NOTIF_NOTHING_TO_REDO);
	builder.register_notification(&NOTIF_UNDO);
	builder.register_notification(&NOTIF_REDO);
	builder.register_notification(&NOTIF_NO_SELECTION);
	builder.register_notification(&NOTIF_NO_SELECTIONS_REMAIN);
	builder.register_notification(&NOTIF_SEARCH_WRAPPED);
	builder.register_notification(&NOTIF_NO_SEARCH_PATTERN);
	builder.register_notification(&NOTIF_NO_MORE_MATCHES);
	builder.register_notification(&NOTIF_NO_MATCHES_FOUND);
	builder.register_notification(&NOTIF_PATTERN_NOT_FOUND);
	builder.register_notification(&NOTIF_PATTERN_NOT_FOUND_WITH);
	builder.register_notification(&NOTIF_REGEX_ERROR);
	builder.register_notification(&NOTIF_SEARCH_INFO);
	builder.register_notification(&NOTIF_REPLACED);
	builder.register_notification(&NOTIF_MATCHES_COUNT);
	builder.register_notification(&NOTIF_SPLITS_COUNT);
	builder.register_notification(&NOTIF_SELECTIONS_KEPT);
	builder.register_notification(&NOTIF_SPLIT_NO_RANGES);
	builder.register_notification(&NOTIF_NO_MATCHES_TO_SPLIT);
	builder.register_notification(&NOTIF_YANKED_CHARS);
	builder.register_notification(&NOTIF_YANKED_LINES);
	builder.register_notification(&NOTIF_DELETED_CHARS);
	builder.register_notification(&NOTIF_FILE_SAVED);
	builder.register_notification(&NOTIF_FILE_NOT_FOUND);
	builder.register_notification(&NOTIF_FILE_LOAD_ERROR);
	builder.register_notification(&NOTIF_FILE_SAVE_ERROR);
	builder.register_notification(&NOTIF_BUFFER_CLOSED);
	builder.register_notification(&NOTIF_OPTION_SET);
	builder.register_notification(&NOTIF_UNHANDLED_RESULT);
	builder.register_notification(&NOTIF_INFO);
	builder.register_notification(&NOTIF_WARN);
	builder.register_notification(&NOTIF_ERROR);
	builder.register_notification(&NOTIF_SUCCESS);
	builder.register_notification(&NOTIF_DEBUG);
	builder.register_notification(&NOTIF_SYNC_TAKING_OWNERSHIP);
	builder.register_notification(&NOTIF_SYNC_OWNERSHIP_DENIED);
}
