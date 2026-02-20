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
/// * Enforced in: `resolve_id_duplicates`
/// * Failure symptom: Iterator order changes unpredictably across builds.
#[cfg_attr(test, test)]
pub(crate) fn test_deterministic_iteration() {
	let mut builder: RegistryBuilder<TestDef, TestEntry, ActionId> = RegistryBuilder::new("test");
	builder.push(Arc::new(TestDef {
		meta: RegistryMetaStatic::minimal("B", "B", ""),
	}));
	builder.push(Arc::new(TestDef {
		meta: RegistryMetaStatic::minimal("A", "A", ""),
	}));

	let registry = RuntimeRegistry::new("test", builder.build());
	let ids: Vec<_> = registry.snapshot_guard().iter().map(|entry| entry.id()).collect();
	assert_eq!(ids.len(), 2);
	let snap = registry.snapshot();
	assert_eq!(snap.interner.resolve(ids[0]), "A");
	assert_eq!(snap.interner.resolve(ids[1]), "B");
}

/// Must have unambiguous ID lookup (one winner per canonical ID).
///
/// * Enforced in: `resolve_id_duplicates`
/// * Failure symptom: Duplicate canonical IDs remain addressable simultaneously.
#[cfg_attr(test, test)]
pub(crate) fn test_unambiguous_id_lookup() {
	let mut builder: RegistryBuilder<TestDef, TestEntry, ActionId> = RegistryBuilder::new("test");
	builder.push(Arc::new(make_def("alpha", 10)));
	builder.push(Arc::new(make_def("beta", 20)));

	let registry = RuntimeRegistry::new("test", builder.build());
	let alpha = registry.get("alpha").expect("alpha must resolve");
	let beta = registry.get("beta").expect("beta must resolve");
	assert_ne!(alpha.dense_id(), beta.dense_id());
}

/// Must keep snapshot-backed refs valid for the process lifetime.
///
/// * Enforced in: `RegistryRef` holding `Arc<Snapshot<...>>`
/// * Failure symptom: stale refs panic or read invalid memory.
#[cfg_attr(test, test)]
pub(crate) fn test_snapshot_liveness_for_registry_ref() {
	let mut builder: RegistryBuilder<TestDef, TestEntry, ActionId> = RegistryBuilder::new("test");
	builder.push(Arc::new(make_def("stable", 10)));
	let registry = RuntimeRegistry::new("test", builder.build());

	let stable_ref = registry.get("stable").expect("stable should resolve");
	let snap_a = registry.snapshot();
	let snap_b = registry.snapshot();

	assert!(Arc::ptr_eq(&snap_a, &snap_b));
	assert_eq!(stable_ref.name_str(), "stable");
	assert_eq!(stable_ref.priority(), 10);
}

/// Must preserve source precedence Runtime > Crate > Builtin on equal priority.
///
/// * Enforced in: `cmp_party`, `resolve_id_duplicates`
/// * Failure symptom: builtin definition wins over runtime definition unexpectedly.
#[cfg_attr(test, test)]
pub(crate) fn test_source_precedence() {
	let mut builder: RegistryBuilder<TestDef, TestEntry, ActionId> = RegistryBuilder::with_policy("test", DuplicatePolicy::ByPriority);
	builder.push(Arc::new(TestDef {
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
	}));
	builder.push(Arc::new(TestDef {
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
	}));

	let registry = RuntimeRegistry::new("test", builder.build());
	let resolved = registry.get("cmd").expect("cmd should resolve");
	assert_eq!(resolved.name_str(), "runtime");
	assert_eq!(resolved.source(), RegistrySource::Runtime);
}

/// Must use ingest ordinal as tie-breaker when priority and source are equal.
///
/// * Enforced in: `cmp_party`, `resolve_id_duplicates`
/// * Failure symptom: equal-precedence duplicates resolve nondeterministically.
#[cfg_attr(test, test)]
pub(crate) fn test_canonical_id_ordinal_tiebreaker() {
	let mut builder: RegistryBuilder<TestDef, TestEntry, ActionId> = RegistryBuilder::with_policy("test", DuplicatePolicy::ByPriority);
	builder.push(Arc::new(TestDef {
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
	}));
	builder.push(Arc::new(TestDef {
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
	}));

	let registry = RuntimeRegistry::new("test", builder.build());
	let resolved = registry.get("tie").expect("tie should resolve");
	assert_eq!(resolved.name_str(), "second");
}
