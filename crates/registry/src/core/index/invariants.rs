use std::sync::Arc;

use crate::core::index::build::RegistryBuilder;
use crate::core::index::collision::DuplicatePolicy;
use crate::core::index::test_fixtures::{TestDef, TestEntry, make_def};
use crate::core::symbol::ActionId;
use crate::core::traits::RegistryEntry;
use crate::core::{RegistryMetaStatic, RegistrySource};

/// Must maintain deterministic iteration order by dense ID (table index).
///
/// Builtins are built in canonical-ID order.
///
/// * Enforced in: `resolve_id_duplicates`
/// * Failure symptom: Iterator order changes unpredictably.
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
	let ids: Vec<_> = index.iter().map(|e| index.interner.resolve(e.meta().id)).collect();
	assert_eq!(ids, vec!["A", "B"]);
}

/// Must panic on duplicate canonical IDs in `DuplicatePolicy::Panic` mode.
///
/// * Enforced in: `crate::core::index::build::resolve_id_duplicates`
/// * Failure symptom: Conflicting canonical IDs silently co-exist in one registry build.
#[cfg_attr(test, test)]
#[should_panic(expected = "Duplicate registry key")]
pub(crate) fn test_duplicate_id_panics_in_debug() {
	let mut builder: RegistryBuilder<TestDef, TestEntry, ActionId> = RegistryBuilder::new("test");
	builder.push(std::sync::Arc::new(make_def("X", 10)));
	builder.push(std::sync::Arc::new(make_def("X", 20)));
	let _ = builder.build();
}

/// Must record alias/key conflicts in collision metadata.
///
/// * Enforced in: `crate::core::index::build::resolve_key_duplicates`
/// * Failure symptom: Collision diagnostics are lost and conflict debugging becomes opaque.
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
			mutates_buffer: false,
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
			mutates_buffer: false,
			flags: 0,
		},
	};
	builder.push(std::sync::Arc::new(def_a));
	builder.push(std::sync::Arc::new(def_b));

	let index = builder.build();
	assert_eq!(index.len(), 2);
	assert!(!index.collisions().is_empty(), "Alias collision should be recorded");
}

/// Must evict old definition on ID override (higher priority wins).
///
/// * Enforced in: `crate::core::index::build::resolve_id_duplicates`
/// * Failure symptom: Stale definition remains accessible after override.
#[cfg_attr(test, test)]
pub(crate) fn test_id_override_eviction() {
	let mut builder: RegistryBuilder<TestDef, TestEntry, ActionId> = RegistryBuilder::with_policy("test", DuplicatePolicy::ByPriority);
	let low = TestDef {
		meta: RegistryMetaStatic {
			id: "X",
			name: "X",
			keys: &[],
			description: "low priority",
			priority: 5,
			source: RegistrySource::Builtin,
			mutates_buffer: false,
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
			mutates_buffer: false,
			flags: 0,
		},
	};
	builder.push(Arc::new(low));
	builder.push(Arc::new(high));

	let index = builder.build();
	assert_eq!(index.len(), 1);
	let entry = index.get("X").expect("X must resolve");
	assert_eq!(entry.priority(), 50, "Higher priority entry must win");
}

/// Must use ingest ordinal as tie-breaker for canonical ID conflicts with equal precedence.
///
/// * Enforced in: `crate::core::index::collision::cmp_party`, `crate::core::index::build::resolve_id_duplicates`
/// * Failure symptom: Equal-priority/equal-source duplicate IDs resolve nondeterministically.
#[cfg_attr(test, test)]
pub(crate) fn test_canonical_id_ordinal_tiebreaker() {
	let mut builder: RegistryBuilder<TestDef, TestEntry, ActionId> = RegistryBuilder::with_policy("test", DuplicatePolicy::ByPriority);

	let first = TestDef {
		meta: RegistryMetaStatic {
			id: "tie",
			name: "first",
			keys: &[],
			description: "",
			priority: 10,
			source: RegistrySource::Builtin,
			mutates_buffer: false,
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
			mutates_buffer: false,
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

/// Must use ingest ordinal as tie-breaker for key/name conflicts with equal precedence.
///
/// * Enforced in: `crate::core::index::collision::cmp_party`, `crate::core::index::build::resolve_key_duplicates`
/// * Failure symptom: Key bindings flip unpredictably across rebuilds.
#[cfg_attr(test, test)]
pub(crate) fn test_key_conflict_ordinal_tiebreaker() {
	let mut builder: RegistryBuilder<TestDef, TestEntry, ActionId> = RegistryBuilder::with_policy("test", DuplicatePolicy::ByPriority);

	let def_first = TestDef {
		meta: RegistryMetaStatic {
			id: "Z",
			name: "Z",
			keys: &["shared"],
			description: "",
			priority: 10,
			source: RegistrySource::Builtin,
			mutates_buffer: false,
			flags: 0,
		},
	};
	let def_second = TestDef {
		meta: RegistryMetaStatic {
			id: "A",
			name: "A",
			keys: &["shared"],
			description: "",
			priority: 10,
			source: RegistrySource::Builtin,
			mutates_buffer: false,
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

/// Must preserve name/key ownership for the winning entry after canonical-ID override ties.
///
/// * Enforced in: `crate::core::index::build::resolve_id_duplicates`, `crate::core::index::build::resolve_key_duplicates`
/// * Failure symptom: Overriding entry wins ID but loses name/key lookups to stale entry.
#[cfg_attr(test, test)]
pub(crate) fn test_id_override_keeps_name_binding_on_tie() {
	let mut builder: RegistryBuilder<TestDef, TestEntry, ActionId> = RegistryBuilder::with_policy("test", DuplicatePolicy::ByPriority);

	let def_z = TestDef {
		meta: RegistryMetaStatic {
			id: "Z",
			name: "shared",
			keys: &[],
			description: "",
			priority: 10,
			source: RegistrySource::Builtin,
			mutates_buffer: false,
			flags: 0,
		},
	};
	let def_a_v1 = TestDef {
		meta: RegistryMetaStatic {
			id: "A",
			name: "shared",
			keys: &[],
			description: "v1",
			priority: 10,
			source: RegistrySource::Builtin,
			mutates_buffer: false,
			flags: 0,
		},
	};
	let def_a_v2 = TestDef {
		meta: RegistryMetaStatic {
			id: "A",
			name: "shared",
			keys: &[],
			description: "v2",
			priority: 10,
			source: RegistrySource::Builtin,
			mutates_buffer: false,
			flags: 0,
		},
	};

	builder.push(Arc::new(def_z));
	builder.push(Arc::new(def_a_v1));
	builder.push(Arc::new(def_a_v2));

	let index = builder.build();
	assert_eq!(index.len(), 2);

	let shared = index.get("shared").unwrap();
	assert_eq!(
		index.interner.resolve(shared.meta().id),
		"A",
		"Overriding entry must win its name binding via ordinal tie-break"
	);
	assert_eq!(index.interner.resolve(shared.meta().description), "v2", "Must be the latest version of A");
}
