#[cfg(test)]
mod tests {
	use bumpalo::Bump;
	use heed3::EnvOpenOptions;
	use tempfile::TempDir;

	use crate::helix_engine::types::VectorError;
	use crate::helix_engine::vector_core::hnsw::HNSW;
	use crate::helix_engine::vector_core::vector::HVector;
	use crate::helix_engine::vector_core::vector_core::{HNSWConfig, VectorCore};

	fn setup_vector_core() -> (VectorCore, TempDir, heed3::Env) {
		let temp_dir = TempDir::new().unwrap();
		let env = unsafe {
			EnvOpenOptions::new()
				.map_size(10 * 1024 * 1024)
				.max_dbs(10)
				.open(temp_dir.path())
				.unwrap()
		};
		let mut wtxn = env.write_txn().unwrap();
		let core = VectorCore::new(&env, &mut wtxn, HNSWConfig::new(None, None, None)).unwrap();
		wtxn.commit().unwrap();
		(core, temp_dir, env)
	}

	#[test]
	fn test_reject_dimension_mismatch() {
		let (core, _temp_dir, env) = setup_vector_core();
		let arena = Bump::new();
		let mut wtxn = env.write_txn().unwrap();

		let label = "test";
		let vec128 = vec![1.0f64; 128];
		let vec256 = vec![1.0f64; 256];

		// First insertion (empty index) - succeeds
		core.insert::<fn(&HVector, &heed3::RoTxn) -> bool>(&mut wtxn, label, &vec128, None, &arena)
			.expect("first insert should succeed");

		// Second insertion with different dimension - should fail
		let result = core
			.insert::<fn(&HVector, &heed3::RoTxn) -> bool>(&mut wtxn, label, &vec256, None, &arena);

		assert!(result.is_err());
		match result {
			Err(VectorError::InvalidVectorLength) => {}
			_ => panic!("Expected InvalidVectorLength error, got {:?}", result),
		}
	}

	#[test]
	fn test_portable_key_lengths() {
		let id = 123u128;
		let level = 5u64;
		let sink_id = 456u128;

		let v_key = VectorCore::vector_key(id, level);
		assert_eq!(
			v_key.len(),
			26,
			"Vector key should be exactly 26 bytes (2 prefix + 16 id + 8 level)"
		);

		let e_key = VectorCore::out_edges_key(id, level, Some(sink_id));
		assert_eq!(
			e_key.len(),
			40,
			"Edge key should be exactly 40 bytes (16 source + 8 level + 16 sink)"
		);

		let e_prefix = VectorCore::out_edges_key(id, level, None);
		assert_eq!(
			e_prefix.len(),
			24,
			"Edge prefix should be exactly 24 bytes (16 source + 8 level)"
		);
	}

	#[test]
	fn test_id_not_reused_after_delete() {
		let (core, _temp_dir, env) = setup_vector_core();
		let arena = Bump::new();
		let mut wtxn = env.write_txn().unwrap();

		let label = "test";
		let vec = vec![1.0f64; 128];

		// 1. Insert vector A
		let v_a = core
			.insert::<fn(&HVector, &heed3::RoTxn) -> bool>(&mut wtxn, label, &vec, None, &arena)
			.unwrap();
		let id_a = v_a.id;

		// 2. Delete vector A
		core.delete(&mut wtxn, id_a, &arena)
			.expect("delete should succeed");

		// 3. Insert vector B
		let v_b = core
			.insert::<fn(&HVector, &heed3::RoTxn) -> bool>(&mut wtxn, label, &vec, None, &arena)
			.unwrap();
		let id_b = v_b.id;

		// 4. Verify IDs are different
		assert_ne!(id_a, id_b, "Vector IDs must not be reused");

		// 5. Verify search doesn't return A
		wtxn.commit().unwrap();
		let rtxn = env.read_txn().unwrap();
		let results = core
			.search::<fn(&HVector, &heed3::RoTxn) -> bool>(
				&rtxn, &vec, 10, label, None, false, &arena,
			)
			.unwrap();

		for res in results {
			assert_ne!(
				res.id, id_a,
				"Deleted vector should not be returned in search results"
			);
		}
	}
}
