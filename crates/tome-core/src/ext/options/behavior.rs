//! Editor behavior options.

use linkme::distributed_slice;

use crate::ext::options::{OPTIONS, OptionDef, OptionScope, OptionType, OptionValue};

#[distributed_slice(OPTIONS)]
static OPT_MOUSE: OptionDef = OptionDef {
	name: "mouse",
	description: "Enable mouse support",
	value_type: OptionType::Bool,
	default: || OptionValue::Bool(true),
	scope: OptionScope::Global,
};

#[distributed_slice(OPTIONS)]
static OPT_LINE_ENDING: OptionDef = OptionDef {
	name: "line_ending",
	description: "Default line ending (lf, crlf, cr)",
	value_type: OptionType::String,
	default: || OptionValue::String("lf".to_string()),
	scope: OptionScope::Buffer,
};

#[distributed_slice(OPTIONS)]
static OPT_IDLE_TIMEOUT: OptionDef = OptionDef {
	name: "idle_timeout",
	description: "Milliseconds before triggering idle hooks",
	value_type: OptionType::Int,
	default: || OptionValue::Int(250),
	scope: OptionScope::Global,
};
