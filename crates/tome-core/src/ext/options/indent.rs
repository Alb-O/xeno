//! Indentation-related options.

use linkme::distributed_slice;

use crate::ext::options::{OPTIONS, OptionDef, OptionScope, OptionType, OptionValue};

#[distributed_slice(OPTIONS)]
static OPT_TAB_WIDTH: OptionDef = OptionDef {
	name: "tab_width",
	description: "Width of a tab character for display",
	value_type: OptionType::Int,
	default: || OptionValue::Int(4),
	scope: OptionScope::Buffer,
};

#[distributed_slice(OPTIONS)]
static OPT_INDENT_WIDTH: OptionDef = OptionDef {
	name: "indent_width",
	description: "Number of spaces for each indent level",
	value_type: OptionType::Int,
	default: || OptionValue::Int(4),
	scope: OptionScope::Buffer,
};

#[distributed_slice(OPTIONS)]
static OPT_USE_TABS: OptionDef = OptionDef {
	name: "use_tabs",
	description: "Use tabs instead of spaces for indentation",
	value_type: OptionType::Bool,
	default: || OptionValue::Bool(false),
	scope: OptionScope::Buffer,
};
