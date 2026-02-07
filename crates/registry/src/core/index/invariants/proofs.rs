//! Invariant proof tests for the new builder-based registry architecture.
//!
//! The previous proofs referenced the old insert/KeyStore/DefRef API which has
//! been replaced by the RegistryBuilder + RegistryIndex + RuntimeRegistry stack.
//! These proofs need to be rewritten against the new architecture.

use std::sync::Arc;

use crate::core::index::build::{BuildEntry, RegistryBuilder, RegistryMetaRef};
use crate::core::index::runtime::RuntimeRegistry;
use crate::core::{
	CapabilitySet, FrozenInterner, RegistryEntry, RegistryMeta, RegistryMetaStatic, RegistrySource,
	Symbol, SymbolList,
};

/// Minimal test entry type for proofs.
struct TestEntry {
	meta: RegistryMeta,
}

impl RegistryEntry for TestEntry {
	fn meta(&self) -> &RegistryMeta {
		&self.meta
	}
}

/// Minimal test def type for proofs.
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

use crate::core::symbol::ActionId;

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

/// Builder produces deterministic index ordering by canonical ID.
#[cfg_attr(test, test)]
pub(crate) fn test_deterministic_iteration() {
	let mut builder: RegistryBuilder<TestDef, TestEntry, ActionId> = RegistryBuilder::new("test");
	let def_a = TestDef {
		meta: RegistryMetaStatic::minimal("A", "A", ""),
	};
	let def_b = TestDef {
		meta: RegistryMetaStatic::minimal("B", "B", ""),
	};
	builder.push(std::sync::Arc::new(def_b));
	builder.push(std::sync::Arc::new(def_a));

	let index = builder.build();
	assert_eq!(index.len(), 2);
	// Items should be sorted by canonical ID
	let ids: Vec<_> = index
		.iter()
		.map(|e| index.interner.resolve(e.meta().id))
		.collect();
	assert_eq!(ids, vec!["A", "B"]);
}

/// Duplicate IDs in debug mode trigger panic (DuplicatePolicy::Panic).
#[cfg_attr(test, test)]
#[should_panic(expected = "Duplicate registry key")]
pub(crate) fn test_duplicate_id_panics_in_debug() {
	let mut builder: RegistryBuilder<TestDef, TestEntry, ActionId> = RegistryBuilder::new("test");
	builder.push(std::sync::Arc::new(make_def("X", 10)));
	builder.push(std::sync::Arc::new(make_def("X", 20)));
	let _ = builder.build(); // Should panic
}

/// Alias conflicts are recorded as collisions.
#[cfg_attr(test, test)]
pub(crate) fn test_alias_collision_recording() {
	let mut builder: RegistryBuilder<TestDef, TestEntry, ActionId> = RegistryBuilder::new("test");
	let def_a = TestDef {
		meta: RegistryMetaStatic {
			id: "A",
			name: "A",
			aliases: &["shared"],
			description: "",
			priority: 10,
			source: RegistrySource::Builtin,
			required_caps: &[],
			flags: 0,
		},
	};
	let def_b = TestDef {
		meta: RegistryMetaStatic {
			id: "B",
			name: "B",
			aliases: &["shared"],
			description: "",
			priority: 20,
			source: RegistrySource::Builtin,
			required_caps: &[],
			flags: 0,
		},
	};
	builder.push(std::sync::Arc::new(def_a));
	builder.push(std::sync::Arc::new(def_b));

	let index = builder.build();
	assert_eq!(index.len(), 2);
	assert!(
		!index.collisions().is_empty(),
		"Alias collision should be recorded"
	);
}

/// Each unique ID resolves to exactly one entry in the built index.
#[cfg_attr(test, test)]
pub(crate) fn test_unambiguous_id_lookup() {
	let mut builder: RegistryBuilder<TestDef, TestEntry, ActionId> = RegistryBuilder::new("test");
	builder.push(Arc::new(make_def("alpha", 10)));
	builder.push(Arc::new(make_def("beta", 20)));

	let index = builder.build();
	let registry = RuntimeRegistry::new("test", index);

	let alpha = registry.get("alpha").expect("alpha must resolve");
	let beta = registry.get("beta").expect("beta must resolve");

	// Each ID maps to a single, distinct dense slot
	assert_ne!(alpha.dense_id(), beta.dense_id());
	assert_eq!(alpha.priority(), 10);
	assert_eq!(beta.priority(), 20);
}

/// When duplicate IDs are ingested with ByPriority policy, the
/// higher-priority entry wins and the loser is evicted from key maps.
#[cfg_attr(test, test)]
pub(crate) fn test_id_override_eviction() {
	use crate::core::index::collision::DuplicatePolicy;
	// Use ByPriority explicitly to test priority-based eviction.
	let mut builder: RegistryBuilder<TestDef, TestEntry, ActionId> =
		RegistryBuilder::with_policy("test", DuplicatePolicy::ByPriority);
	let low = TestDef {
		meta: RegistryMetaStatic {
			id: "X",
			name: "X",
			aliases: &[],
			description: "low priority",
			priority: 5,
			source: RegistrySource::Builtin,
			required_caps: &[],
			flags: 0,
		},
	};
	let high = TestDef {
		meta: RegistryMetaStatic {
			id: "X",
			name: "X",
			aliases: &[],
			description: "high priority",
			priority: 50,
			source: RegistrySource::Builtin,
			required_caps: &[],
			flags: 0,
		},
	};
	builder.push(Arc::new(low));
	builder.push(Arc::new(high));

	let index = builder.build();
	// Only one winner for the duplicate ID
	assert_eq!(index.len(), 1);
	let entry = index.get("X").expect("X must resolve");
	assert_eq!(entry.priority(), 50, "Higher priority entry must win");
}

/// Snapshot-backed RegistryRefs remain valid even after another snapshot is taken.
#[cfg_attr(test, test)]
pub(crate) fn test_snapshot_liveness_across_swap() {
	let mut builder: RegistryBuilder<TestDef, TestEntry, ActionId> = RegistryBuilder::new("test");
	builder.push(Arc::new(make_def("A", 10)));
	let registry = RuntimeRegistry::new("test", builder.build());

	// Pin a snapshot-backed ref
	let ref_a = registry.get("A").expect("A must resolve");
	let snap = registry.snapshot();

	// Take another snapshot (would trigger ArcSwap store in runtime mutation path)
	let snap2 = registry.snapshot();

	// The original ref and snapshot must still be valid and readable
	assert_eq!(ref_a.priority(), 10);
	assert_eq!(snap.table.len(), 1);
	assert!(
		Arc::ptr_eq(&snap, &snap2),
		"No mutation means same snapshot"
	);
}
