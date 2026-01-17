//! Command notification keys.

use crate::{
	AutoDismiss, Level, Notification, NotificationDef, NotificationKey, NotificationReg,
	RegistrySource,
};

static NOTIF_UNKNOWN_COMMAND: NotificationDef = NotificationDef::new(
	"unknown_command",
	Level::Error,
	AutoDismiss::DEFAULT,
	RegistrySource::Builtin,
);
inventory::submit! { NotificationReg(&NOTIF_UNKNOWN_COMMAND) }

static NOTIF_COMMAND_ERROR: NotificationDef = NotificationDef::new(
	"command_error",
	Level::Error,
	AutoDismiss::DEFAULT,
	RegistrySource::Builtin,
);
inventory::submit! { NotificationReg(&NOTIF_COMMAND_ERROR) }

static NOTIF_UNSAVED_CHANGES_FORCE_QUIT: NotificationDef = NotificationDef::new(
	"unsaved_changes_force_quit",
	Level::Error,
	AutoDismiss::DEFAULT,
	RegistrySource::Builtin,
);
inventory::submit! { NotificationReg(&NOTIF_UNSAVED_CHANGES_FORCE_QUIT) }

static NOTIF_NOT_IMPLEMENTED: NotificationDef = NotificationDef::new(
	"not_implemented",
	Level::Warn,
	AutoDismiss::DEFAULT,
	RegistrySource::Builtin,
);
inventory::submit! { NotificationReg(&NOTIF_NOT_IMPLEMENTED) }

static NOTIF_HELP_TEXT: NotificationDef = NotificationDef::new(
	"help_text",
	Level::Info,
	AutoDismiss::Never,
	RegistrySource::Builtin,
);
inventory::submit! { NotificationReg(&NOTIF_HELP_TEXT) }

static NOTIF_DIAGNOSTIC_OUTPUT: NotificationDef = NotificationDef::new(
	"diagnostic_output",
	Level::Info,
	AutoDismiss::Never,
	RegistrySource::Builtin,
);
inventory::submit! { NotificationReg(&NOTIF_DIAGNOSTIC_OUTPUT) }

static NOTIF_DIAGNOSTIC_WARNING: NotificationDef = NotificationDef::new(
	"diagnostic_warning",
	Level::Warn,
	AutoDismiss::Never,
	RegistrySource::Builtin,
);
inventory::submit! { NotificationReg(&NOTIF_DIAGNOSTIC_WARNING) }

static NOTIF_NO_COLLISIONS: NotificationDef = NotificationDef::new(
	"no_collisions",
	Level::Info,
	AutoDismiss::DEFAULT,
	RegistrySource::Builtin,
);
inventory::submit! { NotificationReg(&NOTIF_NO_COLLISIONS) }

static NOTIF_THEME_SET: NotificationDef = NotificationDef::new(
	"theme_set",
	Level::Info,
	AutoDismiss::DEFAULT,
	RegistrySource::Builtin,
);
inventory::submit! { NotificationReg(&NOTIF_THEME_SET) }

#[allow(non_upper_case_globals, non_camel_case_types)]
pub mod keys {
	use super::*;

	pub const unsaved_changes_force_quit: NotificationKey = NotificationKey::new(
		&NOTIF_UNSAVED_CHANGES_FORCE_QUIT,
		"Buffer has unsaved changes (use :q! to force quit)",
	);
	pub const no_collisions: NotificationKey =
		NotificationKey::new(&NOTIF_NO_COLLISIONS, "All good! No collisions found.");

	/// "Unknown command: X".
	pub struct unknown_command;
	impl unknown_command {
		pub fn call(cmd: &str) -> Notification {
			Notification::new(&NOTIF_UNKNOWN_COMMAND, format!("Unknown command: {}", cmd))
		}
	}

	/// Command execution error.
	pub struct command_error;
	impl command_error {
		pub fn call(err: &str) -> Notification {
			Notification::new(&NOTIF_COMMAND_ERROR, format!("Command failed: {}", err))
		}
	}

	/// "X - not yet implemented".
	pub struct not_implemented;
	impl not_implemented {
		pub fn call(feature: &str) -> Notification {
			Notification::new(
				&NOTIF_NOT_IMPLEMENTED,
				format!("{} - not yet implemented", feature),
			)
		}
	}

	/// Multi-line help text (no auto-dismiss).
	pub struct help_text;
	impl help_text {
		pub fn call(text: impl Into<String>) -> Notification {
			Notification::new(&NOTIF_HELP_TEXT, text)
		}
	}

	/// Multi-line diagnostic output (no auto-dismiss).
	pub struct diagnostic_output;
	impl diagnostic_output {
		pub fn call(text: impl Into<String>) -> Notification {
			Notification::new(&NOTIF_DIAGNOSTIC_OUTPUT, text)
		}
	}

	/// Multi-line diagnostic warning (no auto-dismiss).
	pub struct diagnostic_warning;
	impl diagnostic_warning {
		pub fn call(text: impl Into<String>) -> Notification {
			Notification::new(&NOTIF_DIAGNOSTIC_WARNING, text)
		}
	}

	/// "Theme set to 'X'".
	pub struct theme_set;
	impl theme_set {
		pub fn call(name: &str) -> Notification {
			Notification::new(&NOTIF_THEME_SET, format!("Theme set to '{}'", name))
		}
	}
}

