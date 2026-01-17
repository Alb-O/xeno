//! Command notification keys.

use crate::AutoDismiss;

pub mod keys {
	use super::*;

	// Static messages
	notif!(
		unsaved_changes_force_quit,
		Error,
		"Buffer has unsaved changes (use :q! to force quit)"
	);
	notif!(no_collisions, Info, "All good! No collisions found.");

	// Parameterized messages
	notif!(unknown_command(cmd: &str), Error, format!("Unknown command: {}", cmd));
	notif!(command_error(err: &str), Error, format!("Command failed: {}", err));
	notif!(
		not_implemented(feature: &str),
		Warn,
		format!("{} - not yet implemented", feature)
	);
	notif!(theme_set(name: &str), Info, format!("Theme set to '{}'", name));

	// Multi-line output (no auto-dismiss)
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
}
