//! Editor notification keys (buffer, file, search operations).

use std::path::Path;
use std::time::Duration;

use crate::{AutoDismiss, Level, Notification, NotificationDef, NotificationKey, RegistrySource};

static NOTIF_BUFFER_READONLY: NotificationDef = NotificationDef::new(
	"buffer_readonly",
	Level::Warn,
	AutoDismiss::DEFAULT,
	RegistrySource::Builtin,
);

static NOTIF_NOTHING_TO_UNDO: NotificationDef = NotificationDef::new(
	"nothing_to_undo",
	Level::Warn,
	AutoDismiss::DEFAULT,
	RegistrySource::Builtin,
);

static NOTIF_NOTHING_TO_REDO: NotificationDef = NotificationDef::new(
	"nothing_to_redo",
	Level::Warn,
	AutoDismiss::DEFAULT,
	RegistrySource::Builtin,
);

static NOTIF_UNDO: NotificationDef = NotificationDef::new(
	"undo",
	Level::Info,
	AutoDismiss::DEFAULT,
	RegistrySource::Builtin,
);

static NOTIF_REDO: NotificationDef = NotificationDef::new(
	"redo",
	Level::Info,
	AutoDismiss::DEFAULT,
	RegistrySource::Builtin,
);

static NOTIF_SEARCH_WRAPPED: NotificationDef = NotificationDef::new(
	"search_wrapped",
	Level::Info,
	AutoDismiss::DEFAULT,
	RegistrySource::Builtin,
);

static NOTIF_NO_SEARCH_PATTERN: NotificationDef = NotificationDef::new(
	"no_search_pattern",
	Level::Warn,
	AutoDismiss::DEFAULT,
	RegistrySource::Builtin,
);

static NOTIF_NO_SELECTION: NotificationDef = NotificationDef::new(
	"no_selection",
	Level::Warn,
	AutoDismiss::DEFAULT,
	RegistrySource::Builtin,
);

static NOTIF_NO_MORE_MATCHES: NotificationDef = NotificationDef::new(
	"no_more_matches",
	Level::Warn,
	AutoDismiss::DEFAULT,
	RegistrySource::Builtin,
);

static NOTIF_NO_MATCHES_FOUND: NotificationDef = NotificationDef::new(
	"no_matches_found",
	Level::Warn,
	AutoDismiss::DEFAULT,
	RegistrySource::Builtin,
);

static NOTIF_NO_BUFFERS: NotificationDef = NotificationDef::new(
	"no_buffers",
	Level::Warn,
	AutoDismiss::DEFAULT,
	RegistrySource::Builtin,
);

static NOTIF_BUFFER_MODIFIED: NotificationDef = NotificationDef::new(
	"buffer_modified",
	Level::Warn,
	AutoDismiss::DEFAULT,
	RegistrySource::Builtin,
);

static NOTIF_NO_SELECTIONS_REMAIN: NotificationDef = NotificationDef::new(
	"no_selections_remain",
	Level::Warn,
	AutoDismiss::DEFAULT,
	RegistrySource::Builtin,
);

static NOTIF_YANKED_CHARS: NotificationDef = NotificationDef::new(
	"yanked_chars",
	Level::Info,
	AutoDismiss::DEFAULT,
	RegistrySource::Builtin,
);

static NOTIF_YANKED_LINES: NotificationDef = NotificationDef::new(
	"yanked_lines",
	Level::Info,
	AutoDismiss::DEFAULT,
	RegistrySource::Builtin,
);

static NOTIF_DELETED_CHARS: NotificationDef = NotificationDef::new(
	"deleted_chars",
	Level::Info,
	AutoDismiss::DEFAULT,
	RegistrySource::Builtin,
);

static NOTIF_PATTERN_NOT_FOUND: NotificationDef = NotificationDef::new(
	"pattern_not_found",
	Level::Warn,
	AutoDismiss::DEFAULT,
	RegistrySource::Builtin,
);

static NOTIF_REGEX_ERROR: NotificationDef = NotificationDef::new(
	"regex_error",
	Level::Error,
	AutoDismiss::After(Duration::from_secs(8)),
	RegistrySource::Builtin,
);

static NOTIF_SEARCH_INFO: NotificationDef = NotificationDef::new(
	"search_info",
	Level::Info,
	AutoDismiss::DEFAULT,
	RegistrySource::Builtin,
);

static NOTIF_REPLACED: NotificationDef = NotificationDef::new(
	"replaced",
	Level::Info,
	AutoDismiss::DEFAULT,
	RegistrySource::Builtin,
);

static NOTIF_MATCHES_COUNT: NotificationDef = NotificationDef::new(
	"matches_count",
	Level::Info,
	AutoDismiss::DEFAULT,
	RegistrySource::Builtin,
);

static NOTIF_SPLITS_COUNT: NotificationDef = NotificationDef::new(
	"splits_count",
	Level::Info,
	AutoDismiss::DEFAULT,
	RegistrySource::Builtin,
);

static NOTIF_SELECTIONS_KEPT: NotificationDef = NotificationDef::new(
	"selections_kept",
	Level::Info,
	AutoDismiss::DEFAULT,
	RegistrySource::Builtin,
);

static NOTIF_FILE_SAVED: NotificationDef = NotificationDef::new(
	"file_saved",
	Level::Success,
	AutoDismiss::DEFAULT,
	RegistrySource::Builtin,
);

static NOTIF_FILE_NOT_FOUND: NotificationDef = NotificationDef::new(
	"file_not_found",
	Level::Error,
	AutoDismiss::DEFAULT,
	RegistrySource::Builtin,
);

static NOTIF_FILE_LOAD_ERROR: NotificationDef = NotificationDef::new(
	"file_load_error",
	Level::Error,
	AutoDismiss::DEFAULT,
	RegistrySource::Builtin,
);

static NOTIF_FILE_SAVE_ERROR: NotificationDef = NotificationDef::new(
	"file_save_error",
	Level::Error,
	AutoDismiss::DEFAULT,
	RegistrySource::Builtin,
);

static NOTIF_BUFFER_CLOSED: NotificationDef = NotificationDef::new(
	"buffer_closed",
	Level::Info,
	AutoDismiss::DEFAULT,
	RegistrySource::Builtin,
);

static NOTIF_SPLIT_NO_RANGES: NotificationDef = NotificationDef::new(
	"split_no_ranges",
	Level::Warn,
	AutoDismiss::DEFAULT,
	RegistrySource::Builtin,
);

static NOTIF_NO_MATCHES_TO_SPLIT: NotificationDef = NotificationDef::new(
	"no_matches_to_split",
	Level::Warn,
	AutoDismiss::DEFAULT,
	RegistrySource::Builtin,
);

static NOTIF_READONLY_ENABLED: NotificationDef = NotificationDef::new(
	"readonly_enabled",
	Level::Info,
	AutoDismiss::DEFAULT,
	RegistrySource::Builtin,
);

static NOTIF_READONLY_DISABLED: NotificationDef = NotificationDef::new(
	"readonly_disabled",
	Level::Info,
	AutoDismiss::DEFAULT,
	RegistrySource::Builtin,
);

static NOTIF_OPTION_SET: NotificationDef = NotificationDef::new(
	"option_set",
	Level::Info,
	AutoDismiss::DEFAULT,
	RegistrySource::Builtin,
);

#[allow(non_upper_case_globals, non_camel_case_types)]
pub mod keys {
	use super::*;

	pub const buffer_readonly: NotificationKey =
		NotificationKey::new(&NOTIF_BUFFER_READONLY, "Buffer is read-only");
	pub const nothing_to_undo: NotificationKey =
		NotificationKey::new(&NOTIF_NOTHING_TO_UNDO, "Nothing to undo");
	pub const nothing_to_redo: NotificationKey =
		NotificationKey::new(&NOTIF_NOTHING_TO_REDO, "Nothing to redo");
	pub const undo: NotificationKey = NotificationKey::new(&NOTIF_UNDO, "Undo");
	pub const redo: NotificationKey = NotificationKey::new(&NOTIF_REDO, "Redo");
	pub const search_wrapped: NotificationKey =
		NotificationKey::new(&NOTIF_SEARCH_WRAPPED, "Search wrapped to beginning");
	pub const no_search_pattern: NotificationKey =
		NotificationKey::new(&NOTIF_NO_SEARCH_PATTERN, "No search pattern");
	pub const no_selection: NotificationKey =
		NotificationKey::new(&NOTIF_NO_SELECTION, "No selection");
	pub const no_more_matches: NotificationKey =
		NotificationKey::new(&NOTIF_NO_MORE_MATCHES, "No more matches");
	pub const no_matches_found: NotificationKey =
		NotificationKey::new(&NOTIF_NO_MATCHES_FOUND, "No matches found");
	pub const no_buffers: NotificationKey =
		NotificationKey::new(&NOTIF_NO_BUFFERS, "No buffers open");
	pub const buffer_modified: NotificationKey =
		NotificationKey::new(&NOTIF_BUFFER_MODIFIED, "Buffer has unsaved changes");
	pub const no_selections_remain: NotificationKey =
		NotificationKey::new(&NOTIF_NO_SELECTIONS_REMAIN, "No selections remain");
	pub const pattern_not_found: NotificationKey =
		NotificationKey::new(&NOTIF_PATTERN_NOT_FOUND, "Pattern not found");
	pub const no_selection_to_search: NotificationKey =
		NotificationKey::new(&NOTIF_NO_SELECTION, "No selection to search in");
	pub const no_selection_to_split: NotificationKey =
		NotificationKey::new(&NOTIF_NO_SELECTION, "No selection to split");
	pub const split_no_ranges: NotificationKey =
		NotificationKey::new(&NOTIF_SPLIT_NO_RANGES, "Split produced no ranges");
	pub const no_matches_to_split: NotificationKey =
		NotificationKey::new(&NOTIF_NO_MATCHES_TO_SPLIT, "No matches found to split on");
	pub const readonly_enabled: NotificationKey =
		NotificationKey::new(&NOTIF_READONLY_ENABLED, "Read-only enabled");
	pub const readonly_disabled: NotificationKey =
		NotificationKey::new(&NOTIF_READONLY_DISABLED, "Read-only disabled");

	/// "Yanked N chars".
	pub struct yanked_chars;
	impl yanked_chars {
		pub fn call(count: usize) -> Notification {
			Notification::new(&NOTIF_YANKED_CHARS, format!("Yanked {} chars", count))
		}
	}

	/// "Yanked N lines".
	pub struct yanked_lines;
	impl yanked_lines {
		pub fn call(count: usize) -> Notification {
			Notification::new(&NOTIF_YANKED_LINES, format!("Yanked {} lines", count))
		}
	}

	/// "Deleted N chars".
	pub struct deleted_chars;
	impl deleted_chars {
		pub fn call(count: usize) -> Notification {
			Notification::new(&NOTIF_DELETED_CHARS, format!("Deleted {} chars", count))
		}
	}

	/// "Pattern 'X' not found".
	pub struct pattern_not_found_with;
	impl pattern_not_found_with {
		pub fn call(pattern: &str) -> Notification {
			Notification::new(
				&NOTIF_PATTERN_NOT_FOUND,
				format!("Pattern '{}' not found", pattern),
			)
		}
	}

	/// Regex compilation error.
	pub struct regex_error;
	impl regex_error {
		pub fn call(err: &str) -> Notification {
			Notification::new(&NOTIF_REGEX_ERROR, format!("Regex error: {}", err))
		}
	}

	/// "Search: X".
	pub struct search_info;
	impl search_info {
		pub fn call(text: &str) -> Notification {
			Notification::new(&NOTIF_SEARCH_INFO, format!("Search: {}", text))
		}
	}

	/// "Replaced N occurrences".
	pub struct replaced;
	impl replaced {
		pub fn call(count: usize) -> Notification {
			Notification::new(&NOTIF_REPLACED, format!("Replaced {} occurrences", count))
		}
	}

	/// "N matches".
	pub struct matches_count;
	impl matches_count {
		pub fn call(count: usize) -> Notification {
			Notification::new(&NOTIF_MATCHES_COUNT, format!("{} matches", count))
		}
	}

	/// "N splits".
	pub struct splits_count;
	impl splits_count {
		pub fn call(count: usize) -> Notification {
			Notification::new(&NOTIF_SPLITS_COUNT, format!("{} splits", count))
		}
	}

	/// "N selections kept".
	pub struct selections_kept;
	impl selections_kept {
		pub fn call(count: usize) -> Notification {
			Notification::new(&NOTIF_SELECTIONS_KEPT, format!("{} selections kept", count))
		}
	}

	/// "Saved /path/to/file".
	pub struct file_saved;
	impl file_saved {
		pub fn call(path: &Path) -> Notification {
			Notification::new(&NOTIF_FILE_SAVED, format!("Saved {}", path.display()))
		}
	}

	/// "File not found: /path".
	pub struct file_not_found;
	impl file_not_found {
		pub fn call(path: &Path) -> Notification {
			Notification::new(
				&NOTIF_FILE_NOT_FOUND,
				format!("File not found: {}", path.display()),
			)
		}
	}

	/// File load error.
	pub struct file_load_error;
	impl file_load_error {
		pub fn call(err: &str) -> Notification {
			Notification::new(
				&NOTIF_FILE_LOAD_ERROR,
				format!("Failed to load file: {}", err),
			)
		}
	}

	/// File save error.
	pub struct file_save_error;
	impl file_save_error {
		pub fn call(err: &str) -> Notification {
			Notification::new(&NOTIF_FILE_SAVE_ERROR, format!("Failed to save: {}", err))
		}
	}

	/// "Closed name".
	pub struct buffer_closed;
	impl buffer_closed {
		pub fn call(name: &str) -> Notification {
			Notification::new(&NOTIF_BUFFER_CLOSED, format!("Closed {}", name))
		}
	}

	/// "Set option = value".
	pub struct option_set;
	impl option_set {
		pub fn call(key: &str, value: &str) -> Notification {
			Notification::new(&NOTIF_OPTION_SET, format!("{}={}", key, value))
		}
	}
}

pub(crate) static NOTIFICATIONS: &[&NotificationDef] = &[
	&NOTIF_BUFFER_READONLY,
	&NOTIF_NOTHING_TO_UNDO,
	&NOTIF_NOTHING_TO_REDO,
	&NOTIF_UNDO,
	&NOTIF_REDO,
	&NOTIF_SEARCH_WRAPPED,
	&NOTIF_NO_SEARCH_PATTERN,
	&NOTIF_NO_SELECTION,
	&NOTIF_NO_MORE_MATCHES,
	&NOTIF_NO_MATCHES_FOUND,
	&NOTIF_NO_BUFFERS,
	&NOTIF_BUFFER_MODIFIED,
	&NOTIF_NO_SELECTIONS_REMAIN,
	&NOTIF_YANKED_CHARS,
	&NOTIF_YANKED_LINES,
	&NOTIF_DELETED_CHARS,
	&NOTIF_PATTERN_NOT_FOUND,
	&NOTIF_REGEX_ERROR,
	&NOTIF_SEARCH_INFO,
	&NOTIF_REPLACED,
	&NOTIF_MATCHES_COUNT,
	&NOTIF_SPLITS_COUNT,
	&NOTIF_SELECTIONS_KEPT,
	&NOTIF_FILE_SAVED,
	&NOTIF_FILE_NOT_FOUND,
	&NOTIF_FILE_LOAD_ERROR,
	&NOTIF_FILE_SAVE_ERROR,
	&NOTIF_BUFFER_CLOSED,
	&NOTIF_SPLIT_NO_RANGES,
	&NOTIF_NO_MATCHES_TO_SPLIT,
	&NOTIF_READONLY_ENABLED,
	&NOTIF_READONLY_DISABLED,
	&NOTIF_OPTION_SET,
];
