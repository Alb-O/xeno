//! File handling options.

use crate::option;

option!(backup, {
	kdl: "backup",
	type: Bool,
	default: false,
	scope: Global,
	description: "Create backup files before saving",
});

option!(undo_file, {
	kdl: "undo-file",
	type: Bool,
	default: false,
	scope: Global,
	description: "Persist undo history to disk",
});

option!(auto_save, {
	kdl: "auto-save",
	type: Bool,
	default: false,
	scope: Global,
	description: "Automatically save files on focus loss",
});

option!(final_newline, {
	kdl: "final-newline",
	type: Bool,
	default: true,
	scope: Buffer,
	description: "Ensure files end with a newline when saving",
});

option!(trim_trailing_whitespace, {
	kdl: "trim-trailing-whitespace",
	type: Bool,
	default: false,
	scope: Buffer,
	description: "Remove trailing whitespace when saving",
});
