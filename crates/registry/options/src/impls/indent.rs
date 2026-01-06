//! Indentation-related options.

use linkme::distributed_slice;

use crate::{
	OptionDef, OptionScope, OptionType, OptionValue, RegistrySource, TypedOptionKey,
	validators::positive_int, OPTIONS,
};

/// Typed handle for the `TAB_WIDTH` option.
///
/// Number of spaces a tab character occupies for display.
pub const TAB_WIDTH: TypedOptionKey<i64> = TypedOptionKey::new(&__OPT_TAB_WIDTH);

#[allow(non_upper_case_globals)]
#[distributed_slice(OPTIONS)]
static __OPT_TAB_WIDTH: OptionDef = OptionDef {
	id: concat!(env!("CARGO_PKG_NAME"), "::TAB_WIDTH"),
	name: "TAB_WIDTH",
	kdl_key: "tab-width",
	description: "Number of spaces a tab character occupies for display.",
	value_type: OptionType::Int,
	default: || OptionValue::Int(4),
	scope: OptionScope::Buffer,
	priority: 0,
	source: RegistrySource::Crate(env!("CARGO_PKG_NAME")),
	validator: Some(positive_int),
};
