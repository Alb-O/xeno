#![allow(dead_code)]

use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use crate::core::index::build::RegistryBuilder;
use crate::core::index::collision::{self, DuplicatePolicy, KeyKind, KeyStore};
use crate::core::index::insert::{insert_id_key_runtime, insert_typed_key};
use crate::core::index::runtime::RuntimeRegistry;
use crate::{RegistryEntry, RegistryMeta};

struct TestDef {
	meta: RegistryMeta,
	drop_counter: Option<Arc<AtomicUsize>>,
}

impl RegistryEntry for TestDef {
	fn meta(&self) -> &RegistryMeta {
		&self.meta
	}
}

impl Drop for TestDef {
	fn drop(&mut self) {
		if let Some(counter) = &self.drop_counter {
			counter.fetch_add(1, Ordering::SeqCst);
		}
	}
}

fn make_meta(id: &'static str, priority: i16) -> RegistryMeta {
	RegistryMeta {
		id,
		name: id,
		aliases: &[],
		description: "",
		priority,
		source: crate::RegistrySource::Builtin,
		required_caps: &[],
		flags: 0,
	}
}

fn choose_winner_any<'a, 'b, 'c>(
	_kind: KeyKind,
	_key: &str,
	_existing: &'b TestDef,
	_candidate: &'c TestDef,
) -> bool {
	true
}

/// Invariant: Snapshot/definition liveness across ArcSwap swaps.
///
/// Owned runtime definitions MUST remain alive for as long as they are reachable from the snapshot.
pub(crate) fn inv_snapshot_liveness_across_swap() {
	let drop_counter = Arc::new(AtomicUsize::new(0));

	let def_a = TestDef {
		meta: make_meta("X", 10),
		drop_counter: Some(Arc::clone(&drop_counter)),
	};

	let builder = RegistryBuilder::<TestDef>::new("test");
	let registry =
		RuntimeRegistry::with_policy("test", builder.build(), DuplicatePolicy::ByPriority);

	// 1. Register owned "A" under ID "X"
	registry.register_owned(def_a);
	assert_eq!(registry.len(), 1);

	// Hold a reference to the snapshot and the definition
	// This simulates a reader holding a ref while a write happens
	let snap = registry.snapshot();
	let ref_a = snap.get_def("X").unwrap(); // DefRef<T>
	assert_eq!(ref_a.as_entry().id(), "X");

	// 2. Override with static "B" under ID "X" where "B" wins (priority 20 > 10)
	static DEF_B: TestDef = TestDef {
		meta: RegistryMeta {
			id: "X",
			name: "X",
			aliases: &[],
			description: "",
			priority: 20,
			source: crate::RegistrySource::Builtin,
			required_caps: &[],
			flags: 0,
		},
		drop_counter: None,
	};
	registry
		.try_register_many_override(std::iter::once(&DEF_B))
		.unwrap();

	// 3. Verify registry has new value
	assert_eq!(registry.get("X").unwrap().priority(), 20);

	// 4. Verify A is NOT dropped yet because we hold `ref_a` (and `snap`)
	assert_eq!(
		drop_counter.load(Ordering::SeqCst),
		0,
		"Owned def A should be kept alive by snapshot ref"
	);

	// 5. Drop ref_a and snap, A should be dropped now
	drop(ref_a);
	drop(snap);

	assert_eq!(
		drop_counter.load(Ordering::SeqCst),
		1,
		"Owned def A should be dropped after last ref release"
	);
}

#[cfg_attr(test, test)]
pub(crate) fn test_snapshot_liveness_across_swap() {
	inv_snapshot_liveness_across_swap()
}

/// Invariant: ID lookup MUST be unambiguous (one winner per ID).
pub(crate) fn inv_unambiguous_id_lookup() {
	// Mock KeyStore to intercept insertions
	struct MockStore {
		ids: std::collections::HashMap<Box<str>, crate::core::index::types::DefRef<TestDef>>,
	}
	impl KeyStore<TestDef> for MockStore {
		fn get_id_owner(&self, id: &str) -> Option<crate::core::index::types::DefRef<TestDef>> {
			self.ids.get(id).cloned()
		}
		fn insert_id(
			&mut self,
			id: &str,
			def: crate::core::index::types::DefRef<TestDef>,
		) -> Option<crate::core::index::types::DefRef<TestDef>> {
			self.ids.insert(Box::from(id), def)
		}
		fn get_key_winner(&self, _key: &str) -> Option<crate::core::index::types::DefRef<TestDef>> {
			None
		}
		fn set_key_winner(&mut self, _key: &str, _def: crate::core::index::types::DefRef<TestDef>) {
		}
		fn set_id_owner(&mut self, id: &str, def: crate::core::index::types::DefRef<TestDef>) {
			self.ids.insert(Box::from(id), def);
		}
		fn evict_def(&mut self, _def: crate::core::index::types::DefRef<TestDef>) {}
		fn push_collision(&mut self, _c: crate::core::index::collision::Collision) {}
	}

	let mut store = MockStore {
		ids: Default::default(),
	};
	let choose_winner: collision::ChooseWinner<TestDef> = choose_winner_any;

	static DEF_A: TestDef = TestDef {
		meta: RegistryMeta {
			id: "X",
			name: "X",
			aliases: &[],
			description: "",
			priority: 10,
			source: crate::RegistrySource::Builtin,
			required_caps: &[],
			flags: 0,
		},
		drop_counter: None,
	};
	static DEF_B: TestDef = TestDef {
		meta: RegistryMeta {
			id: "X",
			name: "X",
			aliases: &[],
			description: "",
			priority: 20,
			source: crate::RegistrySource::Builtin,
			required_caps: &[],
			flags: 0,
		},
		drop_counter: None,
	};
	let ref_a = crate::core::index::types::DefRef::Builtin(&DEF_A);
	let ref_b = crate::core::index::types::DefRef::Builtin(&DEF_B);

	// 1. Build-time insertion of duplicates should fail or be resolved
	// insert_typed_key for ID returns duplicate error
	let res = insert_typed_key(
		&mut store,
		"test",
		choose_winner,
		KeyKind::Id,
		"X",
		ref_a.clone(),
	);
	assert!(res.is_ok(), "First insert should succeed");

	let res = insert_typed_key(
		&mut store,
		"test",
		choose_winner,
		KeyKind::Id,
		"X",
		ref_b.clone(),
	);
	// In strict build mode, duplicate IDs are fatal
	assert!(res.is_err(), "Duplicate ID at build time should be fatal");

	// 2. Runtime insertion allows override
	let res = insert_id_key_runtime(&mut store, "test", choose_winner, "X", ref_b.clone());
	assert!(res.is_ok(), "Runtime override should succeed");
	assert_eq!(
		store.ids.get("X").unwrap().as_entry().priority(),
		20,
		"New definition should win"
	);
	assert_eq!(store.ids.len(), 1, "Only one winner per ID");
}

#[cfg_attr(test, test)]
pub(crate) fn test_unambiguous_id_lookup() {
	inv_unambiguous_id_lookup()
}

/// Invariant: MUST evict old definitions on ID override.
pub(crate) fn inv_id_override_eviction() {
	let builder = RegistryBuilder::<TestDef>::new("test");
	let registry =
		RuntimeRegistry::with_policy("test", builder.build(), DuplicatePolicy::ByPriority);

	// 1. Register A with name "NAME"
	static DEF_A: TestDef = TestDef {
		meta: RegistryMeta {
			id: "A",
			name: "NAME",
			aliases: &[],
			description: "",
			priority: 10,
			source: crate::RegistrySource::Builtin,
			required_caps: &[],
			flags: 0,
		},
		drop_counter: None,
	};
	registry.try_register(&DEF_A).unwrap();
	assert_eq!(registry.get("A").unwrap().id(), "A");
	assert_eq!(registry.get("NAME").unwrap().id(), "A");

	// 2. Override A with B (same ID "A", different priority)
	// Even though B doesn't claim "NAME" explicitly in this test setup (to isolate eviction logic),
	// the key store eviction should remove pointers to the OLD definition A.
	// However, real world: new def usually claims same names.
	// Here we check that "NAME" no longer points to A.
	static DEF_B: TestDef = TestDef {
		meta: RegistryMeta {
			id: "A",
			name: "NAME", // Re-claims name
			aliases: &[],
			description: "",
			priority: 20,
			source: crate::RegistrySource::Builtin,
			required_caps: &[],
			flags: 0,
		},
		drop_counter: None,
	};
	registry.try_register_override(&DEF_B).unwrap();

	let winner = registry.get("NAME").unwrap();
	assert_eq!(winner.id(), "A");
	assert_eq!(winner.priority(), 20, "NAME should resolve to B (prio 20)");

	// Use address identity check on the &TestDef since both are static builtins here
	let winner_ptr = &*winner as *const TestDef;
	let def_a_ptr = &DEF_A as *const TestDef;
	assert_ne!(winner_ptr, def_a_ptr, "NAME should NOT resolve to A");
}

#[cfg_attr(test, test)]
pub(crate) fn test_id_override_eviction() {
	inv_id_override_eviction()
}

/// Invariant: MUST maintain deterministic iteration order.
pub(crate) fn inv_deterministic_iteration() {
	let mut builder = RegistryBuilder::<TestDef>::new("test");

	// Pre-seed in order A, B
	let def_a = TestDef {
		meta: make_meta("A", 10),
		drop_counter: None,
	};
	let def_b = TestDef {
		meta: make_meta("B", 10),
		drop_counter: None,
	};
	builder.push(Box::leak(Box::new(def_a)));
	builder.push(Box::leak(Box::new(def_b)));

	let registry =
		RuntimeRegistry::with_policy("test", builder.build(), DuplicatePolicy::ByPriority);

	// 1. Initial order A, B
	let ids: Vec<_> = registry.all().iter().map(|r| r.id()).collect();
	assert_eq!(ids, vec!["A", "B"]);

	// 2. Override A (first item) with higher priority
	static DEF_A_NEW: TestDef = TestDef {
		meta: RegistryMeta {
			id: "A",
			name: "A",
			aliases: &[],
			description: "",
			priority: 20,
			source: crate::RegistrySource::Builtin,
			required_caps: &[],
			flags: 0,
		},
		drop_counter: None,
	};
	registry.try_register_override(&DEF_A_NEW).unwrap();

	// 3. Order MUST remain A, B (replacement doesn't move position)
	let ids: Vec<_> = registry.all().iter().map(|r| r.id()).collect();
	assert_eq!(ids, vec!["A", "B"]);
	assert_eq!(registry.get("A").unwrap().priority(), 20);

	// 4. Insert new C
	static DEF_C: TestDef = TestDef {
		meta: RegistryMeta {
			id: "C",
			name: "C",
			aliases: &[],
			description: "",
			priority: 10,
			source: crate::RegistrySource::Builtin,
			required_caps: &[],
			flags: 0,
		},
		drop_counter: None,
	};
	registry.try_register(&DEF_C).unwrap();

	// 5. Order MUST be A, B, C (append new)
	let ids: Vec<_> = registry.all().iter().map(|r| r.id()).collect();
	assert_eq!(ids, vec!["A", "B", "C"]);
}

#[cfg_attr(test, test)]
pub(crate) fn test_deterministic_iteration() {
	inv_deterministic_iteration()
}
