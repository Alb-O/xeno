use std::sync::Arc;

use super::test_fixtures::{TestDef, TestEntry, make_def, make_def_with_keyes, make_def_with_name};
use crate::core::index::build::RegistryBuilder;
use crate::core::index::runtime::RuntimeRegistry;
use crate::core::symbol::{ActionId, DenseId};
use crate::core::traits::RegistryEntry;
use crate::core::{CollisionKind, KeyKind, RegistryMetaStatic, RegistrySource, Resolution};

#[test]
fn test_noop_snapshot_stability() {
	let mut builder: RegistryBuilder<TestDef, TestEntry, ActionId> = RegistryBuilder::new("test");
	builder.push(Arc::new(make_def("A", 10)));
	let registry = RuntimeRegistry::new("test", builder.build());

	let snap_before = registry.snapshot();
	let snap_after = registry.snapshot();
	assert!(Arc::ptr_eq(&snap_before, &snap_after), "snapshot should remain stable for immutable registry");
}

#[test]
fn test_lookup_consistency() {
	let mut builder: RegistryBuilder<TestDef, TestEntry, ActionId> = RegistryBuilder::new("test");
	builder.push(Arc::new(make_def("alpha", 10)));
	builder.push(Arc::new(make_def("beta", 20)));
	let registry = RuntimeRegistry::new("test", builder.build());

	let alpha_ref = registry.get("alpha").expect("alpha should resolve");
	assert_eq!(alpha_ref.priority(), 10);
	let beta_ref = registry.get("beta").expect("beta should resolve");
	assert_eq!(beta_ref.priority(), 20);

	let alpha_by_id = registry.get_by_id(alpha_ref.dense_id()).expect("alpha by id should resolve");
	assert_eq!(alpha_by_id.priority(), 10);

	assert!(registry.get("missing").is_none());
	assert_eq!(registry.len(), 2);
}

#[test]
fn test_snapshot_guard_iteration() {
	let mut builder: RegistryBuilder<TestDef, TestEntry, ActionId> = RegistryBuilder::new("test");
	builder.push(Arc::new(make_def("A", 10)));
	builder.push(Arc::new(make_def("B", 20)));
	builder.push(Arc::new(make_def("C", 30)));
	let registry = RuntimeRegistry::new("test", builder.build());

	let guard = registry.snapshot_guard();
	assert_eq!(guard.len(), 3);
	assert!(!guard.is_empty());

	let priorities: Vec<i16> = guard.iter().map(|entry| entry.priority()).collect();
	assert_eq!(priorities, vec![10, 20, 30]);

	let item_ids: Vec<_> = guard.iter_items().map(|(id, _)| id).collect();
	assert_eq!(item_ids, vec![ActionId::from_u32(0), ActionId::from_u32(1), ActionId::from_u32(2)]);
}

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

#[test]
fn test_stage_blocking_collisions() {
	let mut builder: RegistryBuilder<TestDef, TestEntry, ActionId> = RegistryBuilder::new("test");
	builder.push(Arc::new(make_def("A", 10)));
	builder.push(Arc::new(make_def_with_keyes("B", 20, &["A"])));
	builder.push(Arc::new(make_def_with_name("C", "A", 30)));

	let registry = RuntimeRegistry::new("test", builder.build());
	let snap = registry.snapshot();
	assert_eq!(registry.get("A").expect("A should resolve").id_str(), "A");

	let a_collisions: Vec<_> = snap.collisions.iter().filter(|c| snap.interner.resolve(c.key) == "A").collect();
	assert_eq!(a_collisions.len(), 3);

	for collision in a_collisions {
		match &collision.kind {
			CollisionKind::KeyConflict { existing_kind, resolution, .. } => {
				assert_eq!(*existing_kind, KeyKind::Canonical);
				assert_eq!(*resolution, Resolution::KeptExisting);
			}
			other => panic!("unexpected collision kind: {other:?}"),
		}
	}
}

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

		fn collect_payload_strings<'b>(&'b self, _collector: &mut StringCollector<'_, 'b>) {}

		fn build(&self, ctx: &mut dyn BuildCtx, key_pool: &mut Vec<Symbol>) -> TestEntry {
			ctx.intern("secret");
			TestEntry {
				meta: crate::core::index::meta_build::build_meta(ctx, key_pool, self.meta_ref(), []),
			}
		}
	}

	let mut builder: RegistryBuilder<BadDef, TestEntry, ActionId> = RegistryBuilder::new("test");
	builder.push(Arc::new(BadDef));
	let _ = builder.build();
}
