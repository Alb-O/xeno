use super::def::{OptionScope, OptionValidator};
use crate::core::{OptionDefault, OptionType, RegistryMeta, Symbol};

/// Symbolized option entry.
pub struct OptionEntry {
	pub meta: RegistryMeta,
	pub kdl_key: Symbol,
	pub value_type: OptionType,
	pub default: OptionDefault,
	pub scope: OptionScope,
	pub validator: Option<OptionValidator>,
}

crate::impl_registry_entry!(OptionEntry);
