//! Scroll-related options.

use linkme::distributed_slice;

use crate::validators::positive_int;
use crate::{
	OPTIONS, OptionDef, OptionScope, OptionType, OptionValue, RegistrySource, TypedOptionKey,
};

/// Typed handle for the `SCROLL_LINES` option.
///
/// Number of lines to scroll per mouse wheel tick.
pub const SCROLL_LINES: TypedOptionKey<i64> = TypedOptionKey::new(&__OPT_SCROLL_LINES);

#[allow(non_upper_case_globals)]
#[distributed_slice(OPTIONS)]
static __OPT_SCROLL_LINES: OptionDef = OptionDef {
	id: concat!(env!("CARGO_PKG_NAME"), "::SCROLL_LINES"),
	name: "SCROLL_LINES",
	kdl_key: "scroll-lines",
	description: "Number of lines to scroll per mouse wheel tick.",
	value_type: OptionType::Int,
	default: || OptionValue::Int(2),
	scope: OptionScope::Global,
	priority: 0,
	source: RegistrySource::Crate(env!("CARGO_PKG_NAME")),
	validator: Some(positive_int),
};
