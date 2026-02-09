use std::sync::Arc;

use super::test_fixtures::{
	TestDef, TestEntry, make_def, make_def_with_keyes, make_def_with_source,
};
use crate::core::index::build::RegistryBuilder;
use crate::core::index::runtime::{RegisterError, RuntimeRegistry};
use crate::core::symbol::{ActionId, DenseId};
use crate::core::traits::RegistryEntry;
use crate::core::{DuplicatePolicy, RegistryMetaStatic, RegistrySource};

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

/// Runtime registration of a lower-priority duplicate returns Rejected.
#[test]
fn test_register_rejected_by_priority() {
	let mut builder: RegistryBuilder<TestDef, TestEntry, ActionId> = RegistryBuilder::new("test");
	builder.push(Arc::new(make_def("X", 50)));
	let registry = RuntimeRegistry::new("test", builder.build());

	// Try to register a lower-priority entry with the same ID
	let low: &'static TestDef = Box::leak(Box::new(make_def_with_source(
		"X",
		10,
		RegistrySource::Runtime,
	)));
	let result = registry.register(low);

	assert!(result.is_err(), "Lower priority should be rejected");
	match result.unwrap_err() {
		RegisterError::Rejected {
			existing,
			incoming_id,
			policy,
		} => {
			assert_eq!(existing.priority(), 50);
			assert_eq!(incoming_id, "X");
			assert_eq!(policy, DuplicatePolicy::ByPriority);
		}
	}
	// Registry unchanged
	assert_eq!(registry.len(), 1);
	assert_eq!(registry.get("X").unwrap().priority(), 50);
}

/// Runtime registration of a higher-priority duplicate replaces the existing entry.
#[test]
fn test_register_replaces_existing() {
	let mut builder: RegistryBuilder<TestDef, TestEntry, ActionId> = RegistryBuilder::new("test");
	builder.push(Arc::new(make_def("X", 10)));
	builder.push(Arc::new(make_def("Y", 5)));
	let registry = RuntimeRegistry::new("test", builder.build());

	let high: &'static TestDef = Box::leak(Box::new(make_def_with_source(
		"X",
		99,
		RegistrySource::Runtime,
	)));
	let result = registry.register(high);
	assert!(result.is_ok(), "Higher priority should replace existing");

	let entry = registry.get("X").expect("X must still resolve");
	assert_eq!(entry.priority(), 99, "New entry should have replaced old");

	// Other entries unaffected
	assert_eq!(registry.get("Y").unwrap().priority(), 5);
	assert_eq!(registry.len(), 2);
}

/// Runtime registration with keyes makes the entry reachable by key.
#[test]
fn test_register_with_keys() {
	let mut builder: RegistryBuilder<TestDef, TestEntry, ActionId> = RegistryBuilder::new("test");
	builder.push(Arc::new(make_def("base", 0)));
	let registry = RuntimeRegistry::new("test", builder.build());

	let def: &'static TestDef = Box::leak(Box::new(make_def_with_keyes(
		"my_action",
		10,
		&["ma", "myact"],
	)));
	registry.register(def).expect("should succeed");

	// Reachable by canonical ID
	let by_id = registry
		.get("my_action")
		.expect("canonical ID must resolve");
	assert_eq!(by_id.priority(), 10);

	// Reachable by key
	let by_key1 = registry.get("ma").expect("key 'ma' must resolve");
	assert_eq!(by_key1.dense_id(), by_id.dense_id());

	let by_key2 = registry.get("myact").expect("key 'myact' must resolve");
	assert_eq!(by_key2.dense_id(), by_id.dense_id());
}

/// SnapshotGuard provides correct iteration and count.
#[test]
fn test_snapshot_guard() {
	let mut builder: RegistryBuilder<TestDef, TestEntry, ActionId> = RegistryBuilder::new("test");
	builder.push(Arc::new(make_def("A", 10)));
	builder.push(Arc::new(make_def("B", 20)));
	builder.push(Arc::new(make_def("C", 30)));
	let registry = RuntimeRegistry::new("test", builder.build());

	let guard = registry.snapshot_guard();
	assert_eq!(guard.len(), 3);
	assert!(!guard.is_empty());

	// iter() yields all entries
	let priorities: Vec<i16> = guard.iter().map(|e| e.priority()).collect();
	assert_eq!(priorities.len(), 3);

	// iter_items() yields (Id, &T) pairs with correct dense IDs
	let items: Vec<_> = guard.iter_items().collect();
	assert_eq!(items.len(), 3);
	for (i, (id, _entry)) in items.iter().enumerate() {
		assert_eq!(id.as_u32(), i as u32);
	}
}

/// SnapshotGuard on empty registry.
#[test]
fn test_snapshot_guard_empty() {
	let builder: RegistryBuilder<TestDef, TestEntry, ActionId> = RegistryBuilder::new("test");
	let registry = RuntimeRegistry::new("test", builder.build());

	let guard = registry.snapshot_guard();
	assert_eq!(guard.len(), 0);
	assert!(guard.is_empty());
	assert_eq!(guard.iter().count(), 0);
}

/// get_by_id and get_sym work correctly after runtime extension.
#[test]
fn test_get_by_id_and_get_sym_after_register() {
	let mut builder: RegistryBuilder<TestDef, TestEntry, ActionId> = RegistryBuilder::new("test");
	builder.push(Arc::new(make_def("builtin", 0)));
	let registry = RuntimeRegistry::new("test", builder.build());

	let def: &'static TestDef = Box::leak(Box::new(make_def_with_source(
		"runtime_entry",
		10,
		RegistrySource::Runtime,
	)));
	let reg_ref = registry.register(def).expect("register must succeed");
	let dense_id = reg_ref.dense_id();

	// get_by_id with the returned dense ID
	let by_id = registry
		.get_by_id(dense_id)
		.expect("must resolve by dense ID");
	assert_eq!(by_id.priority(), 10);

	// get_sym with the interned symbol
	let snap = registry.snapshot();
	let sym = snap
		.interner
		.get("runtime_entry")
		.expect("must be interned");
	let by_sym = registry.get_sym(sym).expect("must resolve by symbol");
	assert_eq!(by_sym.priority(), 10);

	// Out-of-range dense ID returns None
	assert!(registry.get_by_id(ActionId::from_u32(9999)).is_none());
}

/// Runtime source wins over builtin source at equal priority.
#[test]
fn test_runtime_source_wins_at_equal_priority() {
	let mut builder: RegistryBuilder<TestDef, TestEntry, ActionId> = RegistryBuilder::new("test");
	builder.push(Arc::new(make_def("X", 10))); // Builtin, priority 10
	let registry = RuntimeRegistry::new("test", builder.build());

	// Register a Runtime-sourced entry with the same priority
	let def: &'static TestDef = Box::leak(Box::new(make_def_with_source(
		"X",
		10,
		RegistrySource::Runtime,
	)));
	let result = registry.register(def);
	assert!(
		result.is_ok(),
		"Runtime source should win at equal priority"
	);

	let entry = registry.get("X").unwrap();
	assert_eq!(entry.source(), RegistrySource::Runtime);
}

/// RegistryRef string resolution helpers work correctly.
#[test]
fn test_registry_ref_string_helpers() {
	let mut builder: RegistryBuilder<TestDef, TestEntry, ActionId> = RegistryBuilder::new("test");
	let def = TestDef {
		meta: RegistryMetaStatic {
			id: "test::my_action",
			name: "my_action",
			keys: &["ma"],
			description: "A test action",
			priority: 42,
			source: RegistrySource::Builtin,
			required_caps: &[],
			flags: 0,
		},
	};
	builder.push(Arc::new(def));
	let registry = RuntimeRegistry::new("test", builder.build());

	let r = registry.get("test::my_action").expect("must resolve");
	assert_eq!(r.id_str(), "test::my_action");
	assert_eq!(r.name_str(), "my_action");
	assert_eq!(r.description_str(), "A test action");
	assert_eq!(r.keys_resolved(), vec!["ma"]);
	assert_eq!(r.dense_id(), ActionId::from_u32(0));
}

/// Verifies the semantic distinction between canonical IDs and secondary keys
/// during runtime collision detection. A secondary key match is NOT a canonical
/// ID collision.
#[test]
fn test_canonical_collision_semantic_distinction() {
	let mut builder: RegistryBuilder<TestDef, TestEntry, ActionId> = RegistryBuilder::new("test");
	// Entry A has canonical ID "A" and secondary key "B"
	builder.push(Arc::new(make_def_with_keyes("A", 10, &["B"])));
	let registry = RuntimeRegistry::new("test", builder.build());

	// Initial state: "B" resolves to A (via secondary key mapping)
	let b_ref = registry.get("B").expect("B should resolve to A");
	assert_eq!(b_ref.id_str(), "A");

	// Register entry with canonical ID "B"
	let def_b: &'static TestDef = Box::leak(Box::new(make_def("B", 5)));
	registry
		.register(def_b)
		.expect("register B should succeed (no canonical collision)");

	// Now "B" should resolve to B (canonical ID Stage A beats prior secondary key Stage C)
	let b_ref_new = registry.get("B").expect("B should resolve to B");
	assert_eq!(b_ref_new.id_str(), "B");
	assert_eq!(registry.len(), 2);

	// "A" still resolves to A
	assert_eq!(registry.get("A").unwrap().id_str(), "A");
}

/// Verifies that incremental updates (append and replace) produce identical
/// lookup maps and collision lists as a full rebuild.
#[test]
fn test_incremental_reference_equivalence() {
	let mut builder: RegistryBuilder<TestDef, TestEntry, ActionId> = RegistryBuilder::new("test");
	builder.push(Arc::new(make_def("builtin_1", 10)));
	builder.push(Arc::new(make_def_with_keyes("builtin_2", 20, &["shared"])));
	let registry = RuntimeRegistry::new("test", builder.build());

	// Deterministic sequence of operations
	let ops = vec![
		// 1. Simple append
		make_def("append_1", 30),
		// 2. Append with key conflict (loser)
		make_def_with_keyes("append_2", 5, &["shared"]),
		// 3. Append with key conflict (winner)
		make_def_with_keyes("append_3", 50, &["shared"]),
		// 4. Replace existing (canonical collision)
		make_def_with_source("append_1", 100, RegistrySource::Runtime),
		// 5. Replace that changes keys
		make_def_with_keyes("builtin_2", 100, &["new_key"]),
	];

	for def in ops {
		let static_def: &'static TestDef = Box::leak(Box::new(def));
		registry
			.register(static_def)
			.expect("register should succeed");

		// Compare current snapshot against full rebuild
		let current_snap = registry.snapshot();

		// Build reference maps using the same logic as initial build
		let (ref_by_name, ref_by_key, ref_collisions) =
			crate::core::index::lookup::build_stage_maps(
				"test",
				&current_snap.table,
				&current_snap.parties,
				&current_snap.key_pool,
				&current_snap.by_id,
			);

		assert_eq!(
			*current_snap.by_name, ref_by_name,
			"by_name mismatch after register"
		);
		assert_eq!(
			*current_snap.by_key, ref_by_key,
			"by_key mismatch after register"
		);
		assert_eq!(
			*current_snap.collisions, ref_collisions,
			"collisions mismatch after register"
		);
	}

	// Verification of final state
	let _snap = registry.snapshot();
	// Check that Stage A always wins
	// builtin_2 was replaced and lost "shared" key.
	// append_3 (priority 50) should own "shared".
	assert_eq!(registry.get("shared").unwrap().id_str(), "append_3");

	// Check that replaced entries keep their dense IDs
	assert_eq!(registry.get("append_1").unwrap().priority(), 100);
}

/// Verifies that the 3-stage key model correctly records block collisions.
#[test]
fn test_stage_blocking_collisions() {
	use crate::core::{CollisionKind, KeyKind, Resolution};

	let mut builder: RegistryBuilder<TestDef, TestEntry, ActionId> = RegistryBuilder::new("test");
	// A: id="A", name="A"
	builder.push(Arc::new(make_def("A", 10)));
	// B: id="B", name="B", keys=["A"] -> blocked by Stage A identity of "A"
	builder.push(Arc::new(make_def_with_keyes("B", 20, &["A"])));
	// C: id="C", name="A" -> blocked by Stage A identity of "A"
	builder.push(Arc::new(super::test_fixtures::make_def_with_name(
		"C", "A", 30,
	)));

	let registry = RuntimeRegistry::new("test", builder.build());
	let snap = registry.snapshot();

	// 1. Verify "A" is owned by identity "A"
	assert_eq!(registry.get("A").unwrap().id_str(), "A");

	// 2. Verify collisions for "A"
	let a_collisions: Vec<_> = snap
		.collisions
		.iter()
		.filter(|c| snap.interner.resolve(c.key) == "A")
		.collect();

	// Should have 3 collisions for "A":
	// - A blocks its own name (Stage A vs Stage B)
	// - A blocks B's secondary key (Stage A vs Stage C)
	// - A blocks C's name (Stage A vs Stage B)
	assert_eq!(a_collisions.len(), 3);

	for c in a_collisions {
		match &c.kind {
			CollisionKind::KeyConflict {
				existing_kind,
				resolution,
				..
			} => {
				assert_eq!(*existing_kind, KeyKind::Canonical);
				assert_eq!(*resolution, Resolution::KeptExisting);
			}
			_ => panic!("Expected key conflict"),
		}
	}

	// 3. Runtime append: D: id="D", name="B" -> blocked by Stage A identity of "B"
	registry
		.register(Box::leak(Box::new(
			super::test_fixtures::make_def_with_name("D", "B", 40),
		)))
		.unwrap();

	let snap = registry.snapshot();
	let b_collisions: Vec<_> = snap
		.collisions
		.iter()
		.filter(|c| snap.interner.resolve(c.key) == "B")
		.collect();

	// - B blocks its own name (Stage A vs Stage B)
	// - B blocks D's name (Stage A identity vs Stage B name conflict)
	// B (id="B", name="B"). D (id="D", name="B").
	// Identity "B" always wins over Stage B name "B".
	assert_eq!(b_collisions.len(), 2);
	for c in b_collisions {
		match &c.kind {
			CollisionKind::KeyConflict {
				existing_kind,
				resolution,
				..
			} => {
				assert_eq!(*existing_kind, KeyKind::Canonical);
				assert_eq!(*resolution, Resolution::KeptExisting);
			}
			_ => panic!("Expected key conflict"),
		}
	}

	// 4. Verify lookup for "B" still returns B
	assert_eq!(registry.get("B").unwrap().id_str(), "B");

	// 5. Runtime append: E: id="E", name="E", keys=["B"] -> blocked by Stage A identity of "B"
	registry
		.register(Box::leak(Box::new(make_def_with_keyes("E", 50, &["B"]))))
		.unwrap();

	let snap = registry.snapshot();
	let b_collisions: Vec<_> = snap
		.collisions
		.iter()
		.filter(|c| snap.interner.resolve(c.key) == "B")
		.collect();

	// Should have 3 collisions for "B" now:
	// - B blocks its own name (Stage A vs Stage B)
	// - B blocks D's name (Stage A vs Stage B)
	// - B blocks E's secondary key (Stage A vs Stage C)
	assert_eq!(b_collisions.len(), 3);
}

/// Verifies that BuildEntry::build() is enforced to only use strings
/// collected by collect_strings() in debug builds.
#[test]
#[cfg(any(debug_assertions, feature = "registry-contracts"))]
#[should_panic(expected = "not in collect_strings()")]
fn test_build_ctx_enforcement() {
	use crate::core::index::{BuildCtx, BuildEntry, RegistryMetaRef, StrListRef, StringCollector};
	use crate::core::meta::RegistrySource;
	use crate::core::symbol::Symbol;

	struct BadDef;
	impl BuildEntry<TestEntry> for BadDef {
		fn meta_ref(&self) -> RegistryMetaRef<'_> {
			RegistryMetaRef {
				id: "bad",
				name: "bad",
				keys: StrListRef::Static(&[]),
				description: "",
				priority: 0,
				source: RegistrySource::Builtin,
				required_caps: &[],
				flags: 0,
			}
		}
		fn short_desc_str(&self) -> &str {
			"bad"
		}
		fn collect_payload_strings<'b>(&'b self, _collector: &mut StringCollector<'_, 'b>) {
			// Deliberately don't collect "secret"
		}
		fn build(&self, ctx: &mut dyn BuildCtx, key_pool: &mut Vec<Symbol>) -> TestEntry {
			ctx.intern("secret"); // This should panic
			TestEntry {
				meta: crate::core::index::meta_build::build_meta(
					ctx,
					key_pool,
					self.meta_ref(),
					[],
				),
			}
		}
	}

	let mut builder: RegistryBuilder<BadDef, TestEntry, ActionId> = RegistryBuilder::new("test");
	builder.push(Arc::new(BadDef));
	let _ = builder.build();
}
