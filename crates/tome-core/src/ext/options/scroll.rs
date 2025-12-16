//! Scrolling behavior options.

use linkme::distributed_slice;

use crate::ext::options::{OptionDef, OptionScope, OptionType, OptionValue, OPTIONS};

#[distributed_slice(OPTIONS)]
static OPT_SCROLL_MARGIN: OptionDef = OptionDef {
    name: "scroll_margin",
    description: "Minimum lines to keep above/below cursor when scrolling",
    value_type: OptionType::Int,
    default: || OptionValue::Int(3),
    scope: OptionScope::Global,
};

#[distributed_slice(OPTIONS)]
static OPT_SCROLL_SMOOTH: OptionDef = OptionDef {
    name: "scroll_smooth",
    description: "Enable smooth scrolling animations",
    value_type: OptionType::Bool,
    default: || OptionValue::Bool(false),
    scope: OptionScope::Global,
};
