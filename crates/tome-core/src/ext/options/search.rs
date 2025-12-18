//! Search-related options.

use linkme::distributed_slice;

use crate::ext::options::{OPTIONS, OptionDef, OptionScope, OptionType, OptionValue};

#[distributed_slice(OPTIONS)]
static OPT_SEARCH_CASE_SENSITIVE: OptionDef = OptionDef {
	name: "search_case_sensitive",
	description: "Case-sensitive search by default",
	value_type: OptionType::Bool,
	default: || OptionValue::Bool(false),
	scope: OptionScope::Global,
};

#[distributed_slice(OPTIONS)]
static OPT_SEARCH_SMART_CASE: OptionDef = OptionDef {
	name: "search_smart_case",
	description: "Smart case: case-sensitive if pattern has uppercase",
	value_type: OptionType::Bool,
	default: || OptionValue::Bool(true),
	scope: OptionScope::Global,
};

#[distributed_slice(OPTIONS)]
static OPT_SEARCH_WRAP: OptionDef = OptionDef {
	name: "search_wrap",
	description: "Wrap search around document boundaries",
	value_type: OptionType::Bool,
	default: || OptionValue::Bool(true),
	scope: OptionScope::Global,
};

#[distributed_slice(OPTIONS)]
static OPT_INCREMENTAL_SEARCH: OptionDef = OptionDef {
	name: "incremental_search",
	description: "Show matches while typing search pattern",
	value_type: OptionType::Bool,
	default: || OptionValue::Bool(true),
	scope: OptionScope::Global,
};
