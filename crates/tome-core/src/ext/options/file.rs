//! File handling options.

use linkme::distributed_slice;

use crate::ext::options::{OPTIONS, OptionDef, OptionScope, OptionType, OptionValue};

#[distributed_slice(OPTIONS)]
static OPT_BACKUP: OptionDef = OptionDef {
	name: "backup",
	description: "Create backup files before saving",
	value_type: OptionType::Bool,
	default: || OptionValue::Bool(false),
	scope: OptionScope::Global,
};

#[distributed_slice(OPTIONS)]
static OPT_UNDO_FILE: OptionDef = OptionDef {
	name: "undo_file",
	description: "Persist undo history to disk",
	value_type: OptionType::Bool,
	default: || OptionValue::Bool(false),
	scope: OptionScope::Global,
};

#[distributed_slice(OPTIONS)]
static OPT_AUTO_SAVE: OptionDef = OptionDef {
	name: "auto_save",
	description: "Automatically save files on focus loss",
	value_type: OptionType::Bool,
	default: || OptionValue::Bool(false),
	scope: OptionScope::Global,
};

#[distributed_slice(OPTIONS)]
static OPT_FINAL_NEWLINE: OptionDef = OptionDef {
	name: "final_newline",
	description: "Ensure files end with a newline when saving",
	value_type: OptionType::Bool,
	default: || OptionValue::Bool(true),
	scope: OptionScope::Buffer,
};

#[distributed_slice(OPTIONS)]
static OPT_TRIM_TRAILING_WHITESPACE: OptionDef = OptionDef {
	name: "trim_trailing_whitespace",
	description: "Remove trailing whitespace when saving",
	value_type: OptionType::Bool,
	default: || OptionValue::Bool(false),
	scope: OptionScope::Buffer,
};
