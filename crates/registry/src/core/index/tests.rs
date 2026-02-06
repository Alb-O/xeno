#[cfg(test)]
mod tests {
	use std::sync::Arc;
	use std::sync::atomic::{AtomicUsize, Ordering};

	use crate::core::index::build::RegistryBuilder;
	use crate::core::index::collision::DuplicatePolicy;
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

	#[test]
	fn test_uaf_trace_elimination() {
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
		assert_eq!(registry.get("X").unwrap().id(), "X");

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

		// 3. Verify A is no longer reachable
		assert_eq!(registry.len(), 1);
		assert_eq!(registry.get("X").unwrap().priority(), 20);

		// 4. Verify A is dropped
		assert_eq!(drop_counter.load(Ordering::SeqCst), 1);
	}

	#[test]
	fn test_batch_dedup_id_order() {
		let builder = RegistryBuilder::<TestDef>::new("test");
		let registry =
			RuntimeRegistry::with_policy("test", builder.build(), DuplicatePolicy::ByPriority);

		// Register two DIFFERENT static instances with SAME ID in one batch
		static DEF1: TestDef = TestDef {
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
		static DEF2: TestDef = TestDef {
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
			.try_register_many_override(vec![&DEF1, &DEF2])
			.unwrap();

		// id_order should only have one "X", and it should be the winner (20)
		assert_eq!(registry.len(), 1);
		let snap = registry.snapshot();
		assert_eq!(snap.id_order.len(), 1);
		assert_eq!(snap.id_order[0].as_ref(), "X");
		assert_eq!(registry.get("X").unwrap().priority(), 20);
	}

	#[test]
	fn test_override_preserves_order() {
		let mut builder = RegistryBuilder::<TestDef>::new("test");
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

		// Initial order: A, B
		let ids: Vec<_> = registry.all().iter().map(|r| r.id()).collect();
		assert_eq!(ids, vec!["A", "B"]);

		// Override A with higher priority static def
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
		registry
			.try_register_many_override(std::iter::once(&DEF_A_NEW))
			.unwrap();

		// Order should still be A, B
		let ids: Vec<_> = registry.all().iter().map(|r| r.id()).collect();
		assert_eq!(ids, vec!["A", "B"]);
		assert_eq!(registry.get("A").unwrap().priority(), 20);
	}

	#[test]
	fn test_noop_registration_avoids_snapshot_churn() {
		let mut builder = RegistryBuilder::<TestDef>::new("test");
		static DEF_A: TestDef = TestDef {
			meta: RegistryMeta {
				id: "A",
				name: "A",
				aliases: &[],
				description: "",
				priority: 10,
				source: crate::RegistrySource::Builtin,
				required_caps: &[],
				flags: 0,
			},
			drop_counter: None,
		};
		builder.push(&DEF_A);
		let registry =
			RuntimeRegistry::with_policy("test", builder.build(), DuplicatePolicy::ByPriority);

		let snap_before = registry.snapshot();

		// Try registering the exact same def again
		registry.try_register(&DEF_A).unwrap();

		let snap_after = registry.snapshot();

		// Should point to the EXACT SAME Arc allocation (ptr equality)
		assert!(
			Arc::ptr_eq(&snap_before, &snap_after),
			"Snapshot should not change on no-op registration"
		);
	}

	#[test]
	fn test_invariants_hold() {
		let mut builder = RegistryBuilder::<TestDef>::new("test");
		let def_a = TestDef {
			meta: make_meta("A", 10),
			drop_counter: None,
		};
		builder.push(Box::leak(Box::new(def_a)));
		let registry =
			RuntimeRegistry::with_policy("test", builder.build(), DuplicatePolicy::ByPriority);

		registry.debug_assert_invariants();

		// Add runtime override
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

		registry.debug_assert_invariants();
	}
}
