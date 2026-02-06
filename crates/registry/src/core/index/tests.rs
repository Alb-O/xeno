#[cfg(test)]
mod tests {
	use std::sync::Arc;
	use std::sync::atomic::AtomicUsize;

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
				counter.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
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
