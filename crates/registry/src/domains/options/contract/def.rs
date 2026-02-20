use super::entry::OptionEntry;
use crate::core::index::{BuildEntry, RegistryMetaRef, StrListRef};
use crate::core::{OptionDefault, OptionType, OptionValue, RegistryMetaStatic, Symbol};

pub type OptionValidator = fn(&OptionValue) -> Result<(), String>;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OptionScope {
	Global,
	Buffer,
}

/// Definition of a configurable option (static input).
#[derive(Clone)]
pub struct OptionDef {
	pub meta: RegistryMetaStatic,
	pub key: &'static str,
	pub value_type: OptionType,
	pub default: OptionDefault,
	pub scope: OptionScope,
	pub validator: Option<OptionValidator>,
}

/// Handle to an option definition.
pub type OptionKey = crate::core::LookupKey<OptionEntry, crate::core::OptionId>;

impl core::fmt::Debug for OptionDef {
	fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
		f.debug_struct("OptionDef").field("name", &self.meta.name).field("key", &self.key).finish()
	}
}

/// Unified input for option registration — either a static `OptionDef`
/// (from `derive_option`) or a `LinkedOptionDef` assembled from spec metadata.
pub type OptionInput = crate::core::def_input::DefInput<OptionDef, crate::options::link::LinkedOptionDef>;

impl BuildEntry<OptionEntry> for OptionDef {
	fn meta_ref(&self) -> RegistryMetaRef<'_> {
		RegistryMetaRef {
			id: self.meta.id,
			name: self.meta.name,
			keys: StrListRef::Static(self.meta.keys),
			description: self.meta.description,
			priority: self.meta.priority,
			source: self.meta.source,
			required_caps: self.meta.required_caps,
			flags: self.meta.flags,
		}
	}

	fn short_desc_str(&self) -> &str {
		self.meta.name
	}

	fn collect_payload_strings<'b>(&'b self, collector: &mut crate::core::index::StringCollector<'_, 'b>) {
		collector.push(self.key);
	}

	fn build(&self, ctx: &mut dyn crate::core::index::BuildCtx, key_pool: &mut Vec<Symbol>) -> OptionEntry {
		let meta = crate::core::index::meta_build::build_meta(ctx, key_pool, self.meta_ref(), [self.key]);

		OptionEntry {
			meta,
			key: ctx.intern(self.key),
			value_type: self.value_type,
			default: self.default.clone(),
			scope: self.scope,
			validator: self.validator,
		}
	}
}
