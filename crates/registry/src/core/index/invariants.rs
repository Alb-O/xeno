use std::sync::Arc;

use crate::core::index::build::RegistryBuilder;
use crate::core::index::collision::DuplicatePolicy;
use crate::core::index::runtime::RuntimeRegistry;
use crate::core::index::test_fixtures::{TestDef, TestEntry, make_def};
use crate::core::symbol::ActionId;
use crate::core::traits::RegistryEntry;
use crate::core::{RegistryMetaStatic, RegistrySource};

/// Must maintain deterministic iteration order by dense ID (table index).
///
/// Builtins are built in canonical-ID order; runtime appends extend in registration order.
/// This test verifies build-time sorting; runtime appends maintain insertion order.
///
/// - Enforced in: `resolve_id_duplicates` (build-time), `RuntimeRegistry::register` (runtime)
/// - Failure symptom: Iterator order changes unpredictably.
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
	// Builtins are sorted by canonical ID at build time
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
pub(crate) fn test_key_collision_recording() {
	let mut builder: RegistryBuilder<TestDef, TestEntry, ActionId> = RegistryBuilder::new("test");
	let def_a = TestDef {
		meta: RegistryMetaStatic {
			id: "A",
			name: "A",
			keys: &["shared"],
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
			keys: &["shared"],
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

/// Must have unambiguous ID lookup (one winner per ID).
///
/// - Enforced in: `resolve_id_duplicates`, `RuntimeRegistry::register`
/// - Failure symptom: Panics or inconsistent lookups.
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

/// Must evict old definition on ID override (higher priority wins).
///
/// - Enforced in: `RuntimeRegistry::register`
/// - Failure symptom: Stale definition remains accessible after override.
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
			keys: &[],
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
			keys: &[],
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

/// Must keep owned definitions alive while reachable via `RegistryRef`.
///
/// - Enforced in: `Snapshot`, `RegistryRef` (holds `Arc<Snapshot>`)
/// - Failure symptom: Use-after-free in `RegistryRef` deref.
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

/// Must provide linearizable writes without lost updates.
///
/// - Enforced in: `RuntimeRegistry::register` (CAS loop)
/// - Failure symptom: Concurrent registrations silently dropped.
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
							keys: &[],
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

/// Must keep symbol resolution stable across snapshot swaps.
///
/// - Enforced in: interner prefix-copy in `RuntimeRegistry::register`
/// - Failure symptom: Interned strings resolve to wrong text after snapshot swap.
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
				keys: &[],
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

/// Must respect source precedence: Runtime > Crate > Builtin.
///
/// - Enforced in: `cmp_party`
/// - Failure symptom: Wrong definition wins a key binding or ID conflict.
#[cfg_attr(test, test)]
pub(crate) fn test_source_precedence() {
	// 1. Build-time precedence: Runtime beats Builtin at same priority
	let mut builder: RegistryBuilder<TestDef, TestEntry, ActionId> =
		RegistryBuilder::with_policy("test", DuplicatePolicy::ByPriority);

	let builtin = TestDef {
		meta: RegistryMetaStatic {
			id: "cmd",
			name: "builtin",
			keys: &[],
			description: "",
			priority: 10,
			source: RegistrySource::Builtin,
			required_caps: &[],
			flags: 0,
		},
	};
	let runtime = TestDef {
		meta: RegistryMetaStatic {
			id: "cmd",
			name: "runtime",
			keys: &[],
			description: "",
			priority: 10,
			source: RegistrySource::Runtime,
			required_caps: &[],
			flags: 0,
		},
	};

	// Order of push shouldn't matter for ByPriority
	builder.push(Arc::new(builtin));
	builder.push(Arc::new(runtime));

	let index = builder.build();
	assert_eq!(index.len(), 1);
	let entry = index.get("cmd").unwrap();
	assert_eq!(
		index.interner.resolve(entry.meta().name),
		"runtime",
		"Runtime source must win over Builtin at same priority (build-time)"
	);

	// 2. Runtime precedence: Runtime beats existing Builtin at same priority
	let mut builder2: RegistryBuilder<TestDef, TestEntry, ActionId> = RegistryBuilder::new("test2");
	let builtin2 = TestDef {
		meta: RegistryMetaStatic {
			id: "cmd2",
			name: "builtin2",
			keys: &[],
			description: "",
			priority: 10,
			source: RegistrySource::Builtin,
			required_caps: &[],
			flags: 0,
		},
	};
	builder2.push(Arc::new(builtin2));
	let registry = RuntimeRegistry::new("test2", builder2.build());

	let runtime_def: &'static TestDef = Box::leak(Box::new(TestDef {
		meta: RegistryMetaStatic {
			id: "cmd2",
			name: "runtime2",
			keys: &[],
			description: "",
			priority: 10,
			source: RegistrySource::Runtime,
			required_caps: &[],
			flags: 0,
		},
	}));

	let res = registry.register(runtime_def);
	assert!(res.is_ok(), "Runtime registration should succeed");
	let entry2 = registry.get("cmd2").unwrap();
	assert_eq!(
		entry2.name_str(),
		"runtime2",
		"Runtime source must win over Builtin at same priority (runtime)"
	);
}

/// On canonical ID conflicts with identical priority and source, later ingest (higher ordinal) wins.
#[cfg_attr(test, test)]
pub(crate) fn test_canonical_id_ordinal_tiebreaker() {
	let mut builder: RegistryBuilder<TestDef, TestEntry, ActionId> =
		RegistryBuilder::with_policy("test", DuplicatePolicy::ByPriority);

	let first = TestDef {
		meta: RegistryMetaStatic {
			id: "tie",
			name: "first",
			keys: &[],
			description: "",
			priority: 10,
			source: RegistrySource::Builtin,
			required_caps: &[],
			flags: 0,
		},
	};
	let second = TestDef {
		meta: RegistryMetaStatic {
			id: "tie",
			name: "second",
			keys: &[],
			description: "",
			priority: 10,
			source: RegistrySource::Builtin,
			required_caps: &[],
			flags: 0,
		},
	};

	builder.push(Arc::new(first));
	builder.push(Arc::new(second));

	let index = builder.build();
	assert_eq!(index.len(), 1);
	let entry = index.get("tie").unwrap();
	assert_eq!(
		index.interner.resolve(entry.meta().name),
		"second",
		"Later ingest must win canonical-ID tie-break"
	);
}

/// On key conflicts (name/key) with identical priority and source, later ingest wins (ordinal tie-break).
#[cfg_attr(test, test)]
pub(crate) fn test_key_conflict_ordinal_tiebreaker() {
	let mut builder: RegistryBuilder<TestDef, TestEntry, ActionId> =
		RegistryBuilder::with_policy("test", DuplicatePolicy::ByPriority);

	let def_first = TestDef {
		meta: RegistryMetaStatic {
			id: "Z", // Alphabetically last, but ingested first
			name: "Z",
			keys: &["shared"],
			description: "",
			priority: 10,
			source: RegistrySource::Builtin,
			required_caps: &[],
			flags: 0,
		},
	};
	let def_second = TestDef {
		meta: RegistryMetaStatic {
			id: "A", // Alphabetically first, but ingested second
			name: "A",
			keys: &["shared"],
			description: "",
			priority: 10,
			source: RegistrySource::Builtin,
			required_caps: &[],
			flags: 0,
		},
	};

	builder.push(Arc::new(def_first));
	builder.push(Arc::new(def_second));

	let index = builder.build();
	assert_eq!(index.len(), 2);

	let shared = index.get("shared").unwrap();
	assert_eq!(
		index.interner.resolve(shared.meta().id),
		"A",
		"Later ingest must win key conflict tie-break (ordinal wins over symbol ID)"
	);
}

/// Regression test: ID override must keep its name/key bindings even on tie.
///
/// This verifies that when a definition is overridden in Stage A (Canonical ID),
/// it also consistently wins its Stage B (Name) and Stage C (Key) bindings
/// because it has the higher ordinal.
#[cfg_attr(test, test)]
pub(crate) fn test_id_override_keeps_name_binding_on_tie() {
	let mut builder: RegistryBuilder<TestDef, TestEntry, ActionId> =
		RegistryBuilder::with_policy("test", DuplicatePolicy::ByPriority);

	// Def Z owns "shared" name
	let def_z = TestDef {
		meta: RegistryMetaStatic {
			id: "Z",
			name: "shared",
			keys: &[],
			description: "",
			priority: 10,
			source: RegistrySource::Builtin,
			required_caps: &[],
			flags: 0,
		},
	};
	// Def A(v1) also wants "shared" name
	let def_a_v1 = TestDef {
		meta: RegistryMetaStatic {
			id: "A",
			name: "shared",
			keys: &[],
			description: "v1",
			priority: 10,
			source: RegistrySource::Builtin,
			required_caps: &[],
			flags: 0,
		},
	};
	// Def A(v2) overrides A(v1) and still wants "shared" name
	let def_a_v2 = TestDef {
		meta: RegistryMetaStatic {
			id: "A",
			name: "shared",
			keys: &[],
			description: "v2",
			priority: 10,
			source: RegistrySource::Builtin,
			required_caps: &[],
			flags: 0,
		},
	};

	builder.push(Arc::new(def_z));
	builder.push(Arc::new(def_a_v1));
	builder.push(Arc::new(def_a_v2));

	let index = builder.build();
	// Only 2 entries: Z and A(v2)
	assert_eq!(index.len(), 2);

	// "shared" name should be won by A(v2) because it was ingested last
	let shared = index.get("shared").unwrap();
	assert_eq!(
		index.interner.resolve(shared.meta().id),
		"A",
		"Overriding entry must win its name binding via ordinal tie-break"
	);
	assert_eq!(
		index.interner.resolve(shared.meta().description),
		"v2",
		"Must be the latest version of A"
	);
}

/// Runtime overrides of canonical IDs must be recorded as DuplicateId collisions.
#[cfg_attr(test, test)]
pub(crate) fn test_runtime_duplicate_id_records_collision() {
	let mut builder: RegistryBuilder<TestDef, TestEntry, ActionId> = RegistryBuilder::new("test");
	builder.push(Arc::new(make_def("X", 10)));
	let registry = RuntimeRegistry::new("test", builder.build());

	// Runtime override
	let override_def: &'static TestDef = Box::leak(Box::new(TestDef {
		meta: RegistryMetaStatic {
			id: "X",
			name: "X_runtime",
			keys: &[],
			description: "runtime version",
			priority: 10, // Same priority, Runtime source wins
			source: RegistrySource::Runtime,
			required_caps: &[],
			flags: 0,
		},
	}));

	let _ = registry
		.register(override_def)
		.expect("Override must succeed");

	let snap = registry.snapshot();
	let collisions = snap.collisions.as_ref();

	use crate::core::index::collision::CollisionKind;
	let dup_collision = collisions
		.iter()
		.find(|c| {
			matches!(c.kind, CollisionKind::DuplicateId { .. })
				&& snap.interner.resolve(c.key) == "X"
		})
		.expect("Must record DuplicateId collision on runtime override");

	if let CollisionKind::DuplicateId { winner, loser, .. } = dup_collision.kind {
		assert_eq!(winner.source, RegistrySource::Runtime);
		assert_eq!(loser.source, RegistrySource::Builtin);
	} else {
		panic!("Wrong collision kind");
	}
}
