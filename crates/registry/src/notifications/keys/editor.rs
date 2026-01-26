//! Editor notification keys (buffer, file, search operations).

use std::path::Path;
use std::time::Duration;

use crate::notifications::AutoDismiss;

pub mod keys {
	use super::*;

	// Buffer state
	notif!(buffer_readonly, Warn, "Buffer is read-only");
	notif!(buffer_modified, Warn, "Buffer has unsaved changes");
	notif!(no_buffers, Warn, "No buffers open");
	notif!(readonly_enabled, Info, "Read-only enabled");
	notif!(readonly_disabled, Info, "Read-only disabled");

	// Undo/redo
	notif!(nothing_to_undo, Warn, "Nothing to undo");
	notif!(nothing_to_redo, Warn, "Nothing to redo");
	notif!(undo, Info, "Undo");
	notif!(redo, Info, "Redo");

	// Selection
	notif!(no_selection, Warn, "No selection");
	notif_alias!(
		no_selection_to_search,
		no_selection,
		"No selection to search in"
	);
	notif_alias!(no_selection_to_split, no_selection, "No selection to split");
	notif!(no_selections_remain, Warn, "No selections remain");

	// Search
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

	// Replace
	notif!(replaced(count: usize), Info, format!("Replaced {} occurrences", count));

	// Match/split operations
	notif!(matches_count(count: usize), Info, format!("{} matches", count));
	notif!(splits_count(count: usize), Info, format!("{} splits", count));
	notif!(selections_kept(count: usize), Info, format!("{} selections kept", count));
	notif!(split_no_ranges, Warn, "Split produced no ranges");
	notif!(no_matches_to_split, Warn, "No matches found to split on");

	// Yank/delete
	notif!(yanked_chars(count: usize), Info, format!("Yanked {} chars", count));
	notif!(yanked_lines(count: usize), Info, format!("Yanked {} lines", count));
	notif!(deleted_chars(count: usize), Info, format!("Deleted {} chars", count));

	// File operations
	notif!(file_saved(path: &Path), Success, format!("Saved {}", path.display()));
	notif!(file_not_found(path: &Path), Error, format!("File not found: {}", path.display()));
	notif!(file_load_error(err: &str), Error, format!("Failed to load file: {}", err));
	notif!(file_save_error(err: &str), Error, format!("Failed to save: {}", err));
	notif!(buffer_closed(name: &str), Info, format!("Closed {}", name));

	// Options
	notif!(option_set(key: &str, value: &str), Info, format!("{}={}", key, value));
}
