#[cfg(test)]
mod tests {
	use bumpalo::Bump;
	use tempfile::TempDir;

	use crate::helix_engine::storage_core::HelixGraphStorage;
	use crate::helix_engine::storage_core::storage_methods::DBMethods;
	use crate::helix_engine::traversal_core::ops::g::G;
	use crate::helix_engine::traversal_core::ops::source::add_n::AddNAdapter;
	use crate::helix_engine::traversal_core::ops::util::update::UpdateAdapter;
	use crate::helix_engine::traversal_core::traversal_iter::RwTraversalIterator;
	use crate::helix_engine::traversal_core::traversal_value::TraversalValue;
	use crate::helix_engine::types::{EngineError, SecondaryIndex, StorageError};
	use crate::protocol::value::Value;

	fn setup_test_storage() -> (HelixGraphStorage, TempDir) {
		let temp_dir = TempDir::new().unwrap();
		let config = crate::helix_engine::traversal_core::config::Config::default();
		let version_info = crate::helix_engine::storage_core::version_info::VersionInfo::default();
		let storage =
			HelixGraphStorage::new(temp_dir.path().to_str().unwrap(), config, version_info)
				.unwrap();
		(storage, temp_dir)
	}

	#[test]
	fn test_atomic_traversal_failure() {
		let (mut storage, _temp_dir) = setup_test_storage();
		let arena = Bump::new();

		// 1. Create a unique secondary index on "name"
		let index_name = "name";
		storage
			.create_secondary_index(SecondaryIndex::Unique(index_name.to_string()))
			.unwrap();

		let mut txn = storage.graph_env.write_txn().unwrap();

		// 2. Add three nodes
		let n1 = G::new_mut(&storage, &arena, &mut txn)
			.add_n("person", None, None)
			.next()
			.unwrap()
			.unwrap();
		let n2 = G::new_mut(&storage, &arena, &mut txn)
			.add_n("person", None, None)
			.next()
			.unwrap()
			.unwrap();

		let n1 = match n1 {
			TraversalValue::Node(n) => n,
			_ => panic!("expected node"),
		};
		let n2 = match n2 {
			TraversalValue::Node(n) => n,
			_ => panic!("expected node"),
		};

		let props_mary = [("name", Value::from("Mary"))];

		// 3. Update [n1, n2] both to "Mary"
		// The first update (n1) should succeed.
		// The second update (n2) should fail due to uniqueness violation on "name".
		// The iterator should stop immediately after the failure.
		let items = vec![Ok(TraversalValue::Node(n1)), Ok(TraversalValue::Node(n2))];
		{
			let iter = RwTraversalIterator::new(&storage, &mut txn, &arena, items.into_iter());
			let mut results = iter.update(&props_mary);

			// First item succeeds
			let res1 = results.next().expect("expected first result");
			assert!(res1.is_ok(), "first update should succeed, got {:?}", res1);

			// Second item fails
			let res2 = results.next().expect("expected second result");
			assert!(
				res2.is_err(),
				"second update should fail due to duplicate key"
			);
			match res2 {
				Err(EngineError::Storage(StorageError::DuplicateKey(_))) => {}
				_ => panic!("Expected DuplicateKey error, got {:?}", res2),
			}

			// Iterator must stop
			assert!(
				results.next().is_none(),
				"iterator should have stopped after failure"
			);
		}
	}
}
