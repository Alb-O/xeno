use super::entry::OptionEntry;
use crate::core::index::{BuildEntry, RegistryMetaRef, StrListRef};
use crate::core::{
	FrozenInterner, LinkedDef, LinkedPayload, OptionDefault, OptionType, OptionValue, RegistryMeta,
	RegistryMetaStatic, Symbol,
};

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
	pub kdl_key: &'static str,
	pub value_type: OptionType,
	pub default: OptionDefault,
	pub scope: OptionScope,
	pub validator: Option<OptionValidator>,
}

/// Handle to an option definition.
pub type OptionKey = crate::core::LookupKey<OptionEntry, crate::core::OptionId>;

impl core::fmt::Debug for OptionDef {
	fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
		f.debug_struct("OptionDef")
			.field("name", &self.meta.name)
			.field("kdl_key", &self.kdl_key)
			.finish()
	}
}

/// Unified input for option registration — either a static `OptionDef`
/// (from `derive_option`) or a `LinkedOptionDef` assembled from KDL metadata.
pub type OptionInput = crate::core::def_input::DefInput<OptionDef, LinkedOptionDef>;

/// An option definition assembled from KDL metadata + Rust validator.
pub type LinkedOptionDef = LinkedDef<OptionPayload>;

#[derive(Clone)]
pub struct OptionPayload {
	pub kdl_key: String,
	pub value_type: OptionType,
	pub default: OptionDefault,
	pub scope: OptionScope,
	pub validator: Option<OptionValidator>,
}

impl LinkedPayload<OptionEntry> for OptionPayload {
	fn collect_extra_keys<'a>(&'a self, sink: &mut Vec<&'a str>) {
		sink.push(self.kdl_key.as_str());
	}

	fn build_entry(
		&self,
		interner: &FrozenInterner,
		meta: RegistryMeta,
		_short_desc: Symbol,
	) -> OptionEntry {
		OptionEntry {
			meta,
			kdl_key: interner
				.get(&self.kdl_key)
				.expect("missing interned kdl_key"),
			value_type: self.value_type,
			default: self.default.clone(),
			scope: self.scope,
			validator: self.validator,
		}
	}
}

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

	fn collect_strings<'a>(&'a self, sink: &mut Vec<&'a str>) {
		crate::core::index::meta_build::collect_meta_strings(
			&self.meta_ref(),
			sink,
			[self.kdl_key],
		);
	}

	fn build(&self, interner: &FrozenInterner, key_pool: &mut Vec<Symbol>) -> OptionEntry {
		let meta = crate::core::index::meta_build::build_meta(
			interner,
			key_pool,
			self.meta_ref(),
			[self.kdl_key],
		);

		OptionEntry {
			meta,
			kdl_key: interner
				.get(self.kdl_key)
				.expect("missing interned kdl_key"),
			value_type: self.value_type,
			default: self.default.clone(),
			scope: self.scope,
			validator: self.validator,
		}
	}
}
