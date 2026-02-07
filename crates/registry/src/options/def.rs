use super::entry::OptionEntry;
use crate::core::index::{BuildEntry, RegistryMetaRef};
use crate::core::{
	CapabilitySet, FrozenInterner, OptionDefault, OptionType, OptionValue, RegistryMeta,
	RegistryMetaStatic, Symbol, SymbolList,
};

pub type OptionValidator = fn(&OptionValue) -> Result<(), String>;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OptionScope {
	Global,
	Buffer,
}

/// Definition of a configurable option (static input).
#[derive(Clone, Copy)]
pub struct OptionDef {
	pub meta: RegistryMetaStatic,
	pub kdl_key: &'static str,
	pub value_type: OptionType,
	pub default: OptionDefault,
	pub scope: OptionScope,
	pub validator: Option<OptionValidator>,
}

/// Handle to an option definition.
pub type OptionKey = &'static OptionDef;

impl core::fmt::Debug for OptionDef {
	fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
		f.debug_struct("OptionDef")
			.field("name", &self.meta.name)
			.field("kdl_key", &self.kdl_key)
			.finish()
	}
}

impl crate::core::RegistryEntry for OptionDef {
	fn meta(&self) -> &RegistryMeta {
		panic!("Called meta() on static OptionDef")
	}
}

impl BuildEntry<OptionEntry> for OptionDef {
	fn meta_ref(&self) -> RegistryMetaRef<'_> {
		RegistryMetaRef {
			id: self.meta.id,
			name: self.meta.name,
			aliases: self.meta.aliases,
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
		let meta = self.meta_ref();
		sink.push(meta.id);
		sink.push(meta.name);
		sink.push(meta.description);
		for &alias in meta.aliases {
			sink.push(alias);
		}
		sink.push(self.kdl_key);
	}

	fn build(&self, interner: &FrozenInterner, alias_pool: &mut Vec<Symbol>) -> OptionEntry {
		let meta_ref = self.meta_ref();
		let start = alias_pool.len() as u32;
		for &alias in meta_ref.aliases {
			alias_pool.push(interner.get(alias).expect("missing interned alias"));
		}
		// kdl_key acts as an implicit alias for option lookup
		alias_pool.push(
			interner
				.get(self.kdl_key)
				.expect("missing interned kdl_key"),
		);
		let len = (alias_pool.len() as u32 - start) as u16;

		let meta = RegistryMeta {
			id: interner.get(meta_ref.id).expect("missing interned id"),
			name: interner.get(meta_ref.name).expect("missing interned name"),
			description: interner
				.get(meta_ref.description)
				.expect("missing interned description"),
			aliases: SymbolList { start, len },
			priority: meta_ref.priority,
			source: meta_ref.source,
			required_caps: CapabilitySet::from_iter(meta_ref.required_caps.iter().cloned()),
			flags: meta_ref.flags,
		};

		OptionEntry {
			meta,
			kdl_key: interner
				.get(self.kdl_key)
				.expect("missing interned kdl_key"),
			value_type: self.value_type,
			default: self.default,
			scope: self.scope,
			validator: self.validator,
		}
	}
}
