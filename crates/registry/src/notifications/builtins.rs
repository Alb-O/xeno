//! Built-in notification implementations.

use std::path::Path;

use crate::db::builder::RegistryDbBuilder;

notif!(unknown_action(name: &str), format!("Unknown action: {}", name));
notif!(action_error(err: impl core::fmt::Display), err.to_string());

notif!(
	unsaved_changes_force_quit,
	"Buffer has unsaved changes (use :q! to force quit)"
);
notif!(no_collisions, "All good! No collisions found.");
notif!(unknown_command(cmd: &str), format!("Unknown command: {}", cmd));
notif!(command_error(err: &str), format!("Command failed: {}", err));
notif!(
	not_implemented(feature: &str),
	format!("{} - not yet implemented", feature)
);
notif!(theme_set(name: &str), format!("Theme set to '{}'", name));
notif!(help_text(text: impl Into<String>), text);
notif!(diagnostic_output(text: impl Into<String>), text);
notif!(diagnostic_warning(text: impl Into<String>), text);

notif!(
	viewport_unavailable,
	"Viewport info unavailable for screen motion"
);
notif_alias!(
	viewport_height_unavailable,
	viewport_unavailable,
	"Viewport height unavailable for screen motion"
);
notif!(
	screen_motion_unavailable,
	"Screen motion target is unavailable"
);
notif!(pending_prompt(prompt: &str), prompt.to_string());
notif!(count_display(count: usize), count.to_string());

notif!(buffer_readonly, "Buffer is read-only");
notif!(buffer_modified, "Buffer has unsaved changes");
notif!(no_buffers, "No buffers open");
notif!(readonly_enabled, "Read-only enabled");
notif!(readonly_disabled, "Read-only disabled");
notif!(nothing_to_undo, "Nothing to undo");
notif!(nothing_to_redo, "Nothing to redo");
notif!(undo, "Undo");
notif!(redo, "Redo");
notif!(no_selection, "No selection");
notif_alias!(
	no_selection_to_search,
	no_selection,
	"No selection to search in"
);
notif_alias!(no_selection_to_split, no_selection, "No selection to split");
notif!(no_selections_remain, "No selections remain");
notif!(search_wrapped, "Search wrapped to beginning");
notif!(no_search_pattern, "No search pattern");
notif!(no_more_matches, "No more matches");
notif!(no_matches_found, "No matches found");
notif!(pattern_not_found, "Pattern not found");
notif!(
	pattern_not_found_with(pattern: &str),
	format!("Pattern '{}' not found", pattern)
);
notif!(
	regex_error(err: &str),
	format!("Regex error: {}", err)
);
notif!(search_info(text: &str), format!("Search: {}", text));
notif!(replaced(count: usize), format!("Replaced {} occurrences", count));
notif!(matches_count(count: usize), format!("{} matches", count));
notif!(splits_count(count: usize), format!("{} splits", count));
notif!(selections_kept(count: usize), format!("{} selections kept", count));
notif!(split_no_ranges, "Split produced no ranges");
notif!(no_matches_to_split, "No matches found to split on");
notif!(yanked_chars(count: usize), format!("Yanked {} chars", count));
notif!(yanked_lines(count: usize), format!("Yanked {} lines", count));
notif!(deleted_chars(count: usize), format!("Deleted {} chars", count));
notif!(file_saved(path: &Path), format!("Saved {}", path.display()));
notif!(file_not_found(path: &Path), format!("File not found: {}", path.display()));
notif!(file_load_error(err: &str), format!("Failed to load file: {}", err));
notif!(file_save_error(err: &str), format!("Failed to save: {}", err));
notif!(buffer_closed(name: &str), format!("Closed {}", name));
notif!(option_set(key: &str, value: &str), format!("{}={}", key, value));
notif!(unhandled_result(variant: &str), format!("Unhandled action result: {}", variant));

notif!(info(msg: impl Into<String>), msg);
notif!(warn(msg: impl Into<String>), msg);
notif!(error(msg: impl Into<String>), msg);
notif!(success(msg: impl Into<String>), msg);
notif!(debug(msg: impl Into<String>), msg);

notif!(sync_taking_ownership, "Taking ownership...");
notif!(sync_ownership_denied, "Ownership denied.");
notif!(
	sync_history_unavailable,
	"Undo unavailable: history store failed to initialize"
);

pub fn register_builtins(builder: &mut RegistryDbBuilder) {
	let blob = crate::kdl::loader::load_notification_metadata();
	let linked = crate::kdl::link::link_notifications(&blob);

	for def in linked {
		builder.register_linked_notification(def);
	}
}

fn register_builtins_reg(
	builder: &mut RegistryDbBuilder,
) -> Result<(), crate::db::builder::RegistryError> {
	register_builtins(builder);
	Ok(())
}

inventory::submit!(crate::db::builtins::BuiltinsReg {
	ordinal: 100,
	f: register_builtins_reg,
});
