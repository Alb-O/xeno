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
