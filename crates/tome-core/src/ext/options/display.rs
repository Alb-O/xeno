//! Display-related options.

use linkme::distributed_slice;

use crate::ext::options::{OPTIONS, OptionDef, OptionScope, OptionType, OptionValue};

#[distributed_slice(OPTIONS)]
static OPT_LINE_NUMBERS: OptionDef = OptionDef {
	name: "line_numbers",
	description: "Show line numbers in the gutter",
	value_type: OptionType::Bool,
	default: || OptionValue::Bool(true),
	scope: OptionScope::Global,
};

#[distributed_slice(OPTIONS)]
static OPT_WRAP_LINES: OptionDef = OptionDef {
	name: "wrap_lines",
	description: "Wrap long lines at window edge",
	value_type: OptionType::Bool,
	default: || OptionValue::Bool(true),
	scope: OptionScope::Buffer,
};

#[distributed_slice(OPTIONS)]
static OPT_CURSORLINE: OptionDef = OptionDef {
	name: "cursorline",
	description: "Highlight the current line",
	value_type: OptionType::Bool,
	default: || OptionValue::Bool(true),
	scope: OptionScope::Global,
};

#[distributed_slice(OPTIONS)]
static OPT_CURSORCOLUMN: OptionDef = OptionDef {
	name: "cursorcolumn",
	description: "Highlight the current column",
	value_type: OptionType::Bool,
	default: || OptionValue::Bool(false),
	scope: OptionScope::Global,
};

#[distributed_slice(OPTIONS)]
static OPT_COLORCOLUMN: OptionDef = OptionDef {
	name: "colorcolumn",
	description: "Column to highlight as margin guide",
	value_type: OptionType::Int,
	default: || OptionValue::Int(0),
	scope: OptionScope::Buffer,
};

#[distributed_slice(OPTIONS)]
static OPT_WHITESPACE_VISIBLE: OptionDef = OptionDef {
	name: "whitespace_visible",
	description: "Show whitespace characters",
	value_type: OptionType::Bool,
	default: || OptionValue::Bool(false),
	scope: OptionScope::Global,
};
