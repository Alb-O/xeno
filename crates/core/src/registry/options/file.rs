//! File handling options.

use crate::option;

option!(
	backup,
	Bool,
	false,
	Global,
	"Create backup files before saving"
);
option!(
	undo_file,
	Bool,
	false,
	Global,
	"Persist undo history to disk"
);
option!(
	auto_save,
	Bool,
	false,
	Global,
	"Automatically save files on focus loss"
);
option!(
	final_newline,
	Bool,
	true,
	Buffer,
	"Ensure files end with a newline when saving"
);
option!(
	trim_trailing_whitespace,
	Bool,
	false,
	Buffer,
	"Remove trailing whitespace when saving"
);
