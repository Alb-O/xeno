#[cfg(test)]
mod tests {
	use tempfile::TempDir;

	use crate::helix_engine::storage_core::HelixGraphStorage;
	use crate::helix_engine::storage_core::storage_methods::DBMethods;
	use crate::helix_engine::storage_core::version_info::VersionInfo;
	use crate::helix_engine::traversal_core::config::Config;
	use crate::helix_engine::types::{EngineError, SecondaryIndex, StorageError};

	fn setup_test_storage() -> (HelixGraphStorage, TempDir) {
		let temp_dir = TempDir::new().unwrap();
		let config = Config::default();
		let version_info = VersionInfo::default();
		let storage =
			HelixGraphStorage::new(temp_dir.path().to_str().unwrap(), config, version_info)
				.unwrap();
		(storage, temp_dir)
	}

	#[test]
	fn test_pack_unpack_edge_data_roundtrip() {
		let test_cases = [
			(0, 0),
			(1, 2),
			(u128::MAX, u128::MAX - 1),
			(
				0x0102030405060708090A0B0C0D0E0F10,
				0x1112131415161718191A1B1C1D1E1F20,
			),
		];

		for (edge_id, node_id) in test_cases {
			let packed = HelixGraphStorage::pack_edge_data(&edge_id, &node_id);
			assert_eq!(packed.len(), 32);

			let (unpacked_edge_id, unpacked_node_id) =
				HelixGraphStorage::unpack_adj_edge_data(&packed).expect("failed to unpack");

			assert_eq!(
				unpacked_edge_id, edge_id,
				"EdgeId mismatch for case ({:X}, {:X})",
				edge_id, node_id
			);
			assert_eq!(
				unpacked_node_id, node_id,
				"NodeId mismatch for case ({:X}, {:X})",
				edge_id, node_id
			);
		}
	}

	#[test]
	fn test_out_edge_key_layout() {
		let from_node_id = 0x0102030405060708090A0B0C0D0E0F10u128;
		let label = [0xAA, 0xBB, 0xCC, 0xDD];

		let key = HelixGraphStorage::out_edge_key(&from_node_id, &label);

		assert_eq!(key.len(), 20);
		assert_eq!(&key[0..16], &from_node_id.to_be_bytes());
		assert_eq!(&key[16..20], &label);
	}

	#[test]
	fn test_in_edge_key_layout() {
		let to_node_id = 0x0102030405060708090A0B0C0D0E0F10u128;
		let label = [0xEE, 0xFF, 0x00, 0x11];

		let key = HelixGraphStorage::in_edge_key(&to_node_id, &label);

		assert_eq!(key.len(), 20);
		assert_eq!(&key[0..16], &to_node_id.to_be_bytes());
		assert_eq!(&key[16..20], &label);
	}

	#[test]
	fn test_unique_index_rejects_duplicate() {
		let (mut storage, _temp_dir) = setup_test_storage();

		let index_name = "unique_name";
		let index = SecondaryIndex::Unique(index_name.to_string());
		storage
			.create_secondary_index(index)
			.expect("failed to create index");

		let (db, active) = storage.secondary_indices.get(index_name).unwrap();
		let active = active.clone();
		let db = *db;

		let mut txn = storage.graph_env.write_txn().unwrap();
		let key = b"duplicate_key";
		let id1 = 1u128;
		let id2 = 2u128;

		// Insert first ID
		active
			.insert(&db, &mut txn, key, &id1)
			.expect("first insert should succeed");

		// Insert second ID for same key -> should fail
		let result = active.insert(&db, &mut txn, key, &id2);
		assert!(result.is_err());
		match result {
			Err(EngineError::Storage(StorageError::DuplicateKey(_))) => {}
			_ => panic!("Expected DuplicateKey error, got {:?}", result),
		}

		// Insert same ID for same key -> should succeed (idempotent)
		active
			.insert(&db, &mut txn, key, &id1)
			.expect("idempotent insert should succeed");

		// Delete first ID
		active
			.delete(&db, &mut txn, key, &id1)
			.expect("delete should succeed");

		// Insert second ID again -> should succeed now
		active
			.insert(&db, &mut txn, key, &id2)
			.expect("insert after delete should succeed");
	}
}
