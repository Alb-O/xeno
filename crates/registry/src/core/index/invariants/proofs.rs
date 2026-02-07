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

/// CAS loop ensures no lost updates under concurrent registration.
#[cfg_attr(test, test)]
pub(crate) fn test_no_lost_updates() {
	use std::sync::Arc;
	use std::sync::atomic::{AtomicUsize, Ordering};
	use std::thread;

	let mut builder: RegistryBuilder<TestDef, TestEntry, ActionId> = RegistryBuilder::new("test");
	builder.push(Arc::new(make_def("base", 0)));
	let registry = Arc::new(RuntimeRegistry::new("test", builder.build()));

	let num_threads = 8;
	let regs_per_thread = 10;
	let total_regs = num_threads * regs_per_thread;

	// Track successful registrations
	let success_count = Arc::new(AtomicUsize::new(0));

	let handles: Vec<_> = (0..num_threads)
		.map(|thread_id| {
			let registry = Arc::clone(&registry);
			let success_count = Arc::clone(&success_count);
			thread::spawn(move || {
				for i in 0..regs_per_thread {
					// Create unique ID for this registration
					let id = format!("thread{}_reg{}", thread_id, i);
					let id_leak: &'static str = Box::leak(id.into_boxed_str());

					// Leak the def to get 'static reference (matches real usage where defs are static)
					let def: &'static TestDef = Box::leak(Box::new(TestDef {
						meta: RegistryMetaStatic {
							id: id_leak,
							name: id_leak,
							aliases: &[],
							description: "",
							priority: (thread_id * 100 + i) as i16,
							source: RegistrySource::Runtime,
							required_caps: &[],
							flags: 0,
						},
					}));

					if registry.register(def).is_ok() {
						success_count.fetch_add(1, Ordering::SeqCst);
					}
				}
			})
		})
		.collect();

	// Wait for all threads
	for handle in handles {
		handle.join().unwrap();
	}

	// All registrations should have succeeded (unique IDs, no conflicts)
	assert_eq!(
		success_count.load(Ordering::SeqCst),
		total_regs,
		"All concurrent registrations should succeed"
	);

	// Verify all entries are present and resolvable
	assert_eq!(registry.len(), 1 + total_regs);

	// Spot check a few registrations
	for thread_id in 0..num_threads {
		for i in [0, regs_per_thread - 1] {
			let id = format!("thread{}_reg{}", thread_id, i);
			let entry = registry.get(&id);
			assert!(entry.is_some(), "Entry {} should be resolvable", id);
			assert_eq!(entry.unwrap().priority(), (thread_id * 100 + i) as i16);
		}
	}
}

/// Symbol resolution remains stable across snapshot swaps.
#[cfg_attr(test, test)]
pub(crate) fn test_symbol_stability_across_swap() {
	let mut builder: RegistryBuilder<TestDef, TestEntry, ActionId> = RegistryBuilder::new("test");
	builder.push(Arc::new(make_def("stable", 10)));
	let registry = RuntimeRegistry::new("test", builder.build());

	// Hold a reference from before any swaps
	let stable_ref = registry.get("stable").expect("stable must resolve");
	let original_name = stable_ref.name_str().to_string();
	let original_id = stable_ref.id_str().to_string();

	// Register new entries to cause swaps
	for i in 0..5 {
		let id = format!("new{}", i);
		let id_leak: &'static str = Box::leak(id.into_boxed_str());
		let def: &'static TestDef = Box::leak(Box::new(TestDef {
			meta: RegistryMetaStatic {
				id: id_leak,
				name: id_leak,
				aliases: &[],
				description: "",
				priority: i as i16,
				source: RegistrySource::Runtime,
				required_caps: &[],
				flags: 0,
			},
		}));

		let _ = registry.register(def);
	}

	// Original reference must still resolve correctly
	assert_eq!(
		stable_ref.name_str(),
		original_name,
		"Name must be stable after swaps"
	);
	assert_eq!(
		stable_ref.id_str(),
		original_id,
		"ID must be stable after swaps"
	);
	assert_eq!(stable_ref.priority(), 10, "Priority must be unchanged");
}
