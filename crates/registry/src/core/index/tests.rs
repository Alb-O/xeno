use std::sync::Arc;

use crate::core::index::build::{BuildEntry, RegistryBuilder, RegistryMetaRef};
use crate::core::index::runtime::RuntimeRegistry;
use crate::core::symbol::ActionId;
use crate::core::{
	CapabilitySet, FrozenInterner, RegistryEntry, RegistryMeta, RegistryMetaStatic, RegistrySource,
	Symbol, SymbolList,
};

/// Runtime entry produced by the builder.
struct TestEntry {
	meta: RegistryMeta,
}

impl RegistryEntry for TestEntry {
	fn meta(&self) -> &RegistryMeta {
		&self.meta
	}
}

/// Static definition used as builder input.
#[derive(Clone)]
struct TestDef {
	meta: RegistryMetaStatic,
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

fn make_def(id: &'static str, priority: i16) -> TestDef {
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

/// Verifies that building a registry and creating a RuntimeRegistry produces
/// a stable snapshot that does not change when no mutations occur.
#[test]
fn test_noop_snapshot_stability() {
	let mut builder: RegistryBuilder<TestDef, TestEntry, ActionId> = RegistryBuilder::new("test");
	builder.push(Arc::new(make_def("A", 10)));
	let registry = RuntimeRegistry::new("test", builder.build());

	let snap_before = registry.snapshot();
	let snap_after = registry.snapshot();

	// Consecutive snapshot loads without mutation should return the same Arc
	assert!(
		Arc::ptr_eq(&snap_before, &snap_after),
		"Snapshot should not change when no mutations occur"
	);
}

/// Verifies that the RuntimeRegistry correctly indexes entries and supports
/// lookup by key string and by dense ID.
#[test]
fn test_lookup_consistency() {
	let mut builder: RegistryBuilder<TestDef, TestEntry, ActionId> = RegistryBuilder::new("test");
	builder.push(Arc::new(make_def("alpha", 10)));
	builder.push(Arc::new(make_def("beta", 20)));
	let registry = RuntimeRegistry::new("test", builder.build());

	// Lookup by key string
	let alpha_ref = registry.get("alpha").expect("alpha should be found");
	assert_eq!(alpha_ref.priority(), 10);

	let beta_ref = registry.get("beta").expect("beta should be found");
	assert_eq!(beta_ref.priority(), 20);

	// Lookup by dense ID
	let alpha_by_id = registry
		.get_by_id(alpha_ref.dense_id())
		.expect("alpha by id");
	assert_eq!(alpha_by_id.priority(), 10);

	// Non-existent key returns None
	assert!(registry.get("nonexistent").is_none());

	// Total count
	assert_eq!(registry.len(), 2);
}
