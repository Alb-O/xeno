//! Shared test helpers for registry index tests.

use crate::core::index::build::{BuildEntry, RegistryMetaRef};
use crate::core::{
	CapabilitySet, FrozenInterner, RegistryEntry, RegistryMeta, RegistryMetaStatic, RegistrySource,
	Symbol, SymbolList,
};

/// Minimal test entry type for proofs.
pub(crate) struct TestEntry {
	pub meta: RegistryMeta,
}

impl RegistryEntry for TestEntry {
	fn meta(&self) -> &RegistryMeta {
		&self.meta
	}
}

/// Minimal test def type for proofs.
#[derive(Clone)]
pub(crate) struct TestDef {
	pub meta: RegistryMetaStatic,
}

impl BuildEntry<TestEntry> for TestDef {
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
		sink.push(self.meta.id);
		sink.push(self.meta.name);
		sink.push(self.meta.description);
		for &alias in self.meta.aliases {
			sink.push(alias);
		}
	}
	fn build(&self, interner: &FrozenInterner, alias_pool: &mut Vec<Symbol>) -> TestEntry {
		let meta_ref = self.meta_ref();
		let start = alias_pool.len() as u32;
		for &alias in meta_ref.aliases {
			alias_pool.push(interner.get(alias).expect("missing interned alias"));
		}
		let len = (alias_pool.len() as u32 - start) as u16;

		TestEntry {
			meta: RegistryMeta {
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
			},
		}
	}
}

pub(crate) fn make_def(id: &'static str, priority: i16) -> TestDef {
	TestDef {
		meta: RegistryMetaStatic {
			id,
			name: id,
			aliases: &[],
			description: "",
			priority,
			source: RegistrySource::Builtin,
			required_caps: &[],
			flags: 0,
		},
	}
}

pub(crate) fn make_def_with_source(
	id: &'static str,
	priority: i16,
	source: RegistrySource,
) -> TestDef {
	TestDef {
		meta: RegistryMetaStatic {
			id,
			name: id,
			aliases: &[],
			description: "",
			priority,
			source,
			required_caps: &[],
			flags: 0,
		},
	}
}

pub(crate) fn make_def_with_aliases(
	id: &'static str,
	priority: i16,
	aliases: &'static [&'static str],
) -> TestDef {
	TestDef {
		meta: RegistryMetaStatic {
			id,
			name: id,
			aliases,
			description: "",
			priority,
			source: RegistrySource::Builtin,
			required_caps: &[],
			flags: 0,
		},
	}
}
