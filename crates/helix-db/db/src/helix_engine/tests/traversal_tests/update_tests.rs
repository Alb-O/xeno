use std::sync::Arc;

use bumpalo::Bump;
use heed3::RoTxn;
use tempfile::TempDir;

use super::test_utils::props_option;
use crate::helix_engine::storage_core::HelixGraphStorage;
use crate::helix_engine::traversal_core::ops::g::G;
use crate::helix_engine::traversal_core::ops::source::add_n::AddNAdapter;
use crate::helix_engine::traversal_core::ops::source::n_from_id::NFromIdAdapter;
use crate::helix_engine::traversal_core::ops::source::v_from_id::VFromIdAdapter;
use crate::helix_engine::traversal_core::ops::util::update::UpdateAdapter;
use crate::helix_engine::traversal_core::ops::vectors::insert::InsertVAdapter;
use crate::helix_engine::traversal_core::traversal_value::TraversalValue;
use crate::helix_engine::vector_core::vector::HVector;
use crate::props;
use crate::protocol::value::Value;
use crate::utils::properties::ImmutablePropertiesMap;

type Filter = fn(&HVector, &RoTxn) -> bool;

fn setup_test_db() -> (TempDir, Arc<HelixGraphStorage>) {
	let temp_dir = TempDir::new().unwrap();
	let db_path = temp_dir.path().to_str().unwrap();
	let storage = HelixGraphStorage::new(
		db_path,
		crate::helix_engine::traversal_core::config::Config::default(),
		Default::default(),
	)
	.unwrap();
	(temp_dir, Arc::new(storage))
}

#[test]
fn test_update_node() {
	let (_temp_dir, storage) = setup_test_db();
	let arena = Bump::new();
	let mut txn = storage.graph_env.write_txn().unwrap();

	let node = G::new_mut(&storage, &arena, &mut txn)
		.add_n(
			"person",
			props_option(&arena, props!("name" => "test")),
			None,
		)
		.collect_to_obj()
		.unwrap();
	G::new_mut(&storage, &arena, &mut txn)
		.add_n(
			"person",
			props_option(&arena, props!("name" => "test2")),
			None,
		)
		.collect_to_obj()
		.unwrap();
	txn.commit().unwrap();

	let arena_read = Bump::new();
	let txn = storage.graph_env.read_txn().unwrap();
	let traversal = G::new(&storage, &txn, &arena_read)
		.n_from_id(&node.id())
		.collect::<Result<Vec<_>, _>>()
		.unwrap();
	drop(txn);

	let arena = Bump::new();
	let mut txn = storage.graph_env.write_txn().unwrap();
	G::new_mut_from_iter(&storage, &mut txn, traversal.into_iter(), &arena)
		.update(&[("name", Value::from("john"))])
		.collect::<Result<Vec<_>, _>>()
		.unwrap();
	txn.commit().unwrap();

	let arena = Bump::new();
	let txn = storage.graph_env.read_txn().unwrap();
	let updated = G::new(&storage, &txn, &arena)
		.n_from_id(&node.id())
		.collect::<Result<Vec<_>, _>>()
		.unwrap();
	assert_eq!(updated.len(), 1);

	match &updated[0] {
		TraversalValue::Node(node) => {
			match node.properties.as_ref().unwrap().get("name").unwrap() {
				Value::String(name) => assert_eq!(name, "john"),
				other => panic!("unexpected value {other:?}"),
			}
		}
		other => panic!("unexpected traversal value: {other:?}"),
	}
}

#[test]
fn test_update_vector_properties() {
	let (_temp_dir, storage) = setup_test_db();
	let arena = Bump::new();
	let mut txn = storage.graph_env.write_txn().unwrap();

	let props_map = ImmutablePropertiesMap::new(
		1,
		std::iter::once(("name", Value::from("original"))),
		&arena,
	);

	let vector = G::new_mut(&storage, &arena, &mut txn)
		.insert_v::<Filter>(&[1.0, 2.0, 3.0], "embedding", Some(props_map))
		.collect_to_obj()
		.unwrap();
	let vector_id = vector.id();
	txn.commit().unwrap();

	// Read back the vector (with data)
	let arena = Bump::new();
	let txn = storage.graph_env.read_txn().unwrap();
	let traversal = G::new(&storage, &txn, &arena)
		.v_from_id(&vector_id, true)
		.collect::<Result<Vec<_>, _>>()
		.unwrap();
	drop(txn);

	// Update properties
	let arena = Bump::new();
	let mut txn = storage.graph_env.write_txn().unwrap();
	G::new_mut_from_iter(&storage, &mut txn, traversal.into_iter(), &arena)
		.update(&[("name", Value::from("updated"))])
		.collect::<Result<Vec<_>, _>>()
		.unwrap();
	txn.commit().unwrap();

	// Verify the update persisted
	let arena = Bump::new();
	let txn = storage.graph_env.read_txn().unwrap();
	let fetched = G::new(&storage, &txn, &arena)
		.v_from_id(&vector_id, true)
		.collect::<Result<Vec<_>, _>>()
		.unwrap();
	assert_eq!(fetched.len(), 1);

	match &fetched[0] {
		TraversalValue::Vector(v) => {
			let name = v.properties.as_ref().unwrap().get("name").unwrap();
			assert_eq!(name, &Value::from("updated"));
			assert_eq!(v.data.len(), 3);
			assert_eq!(v.data[0], 1.0);
		}
		other => panic!("expected Vector, got {other:?}"),
	}
}

#[test]
fn test_update_vector_preserves_deleted_and_level() {
	let (_temp_dir, storage) = setup_test_db();
	let arena = Bump::new();
	let mut txn = storage.graph_env.write_txn().unwrap();

	let vector = G::new_mut(&storage, &arena, &mut txn)
		.insert_v::<Filter>(&[4.0, 5.0, 6.0], "embedding", None)
		.collect_to_obj()
		.unwrap();
	let vector_id = vector.id();

	let (orig_deleted, orig_level) = match &vector {
		TraversalValue::Vector(v) => (v.deleted, v.level),
		_ => panic!("expected Vector"),
	};
	txn.commit().unwrap();

	// Read back and update
	let arena = Bump::new();
	let txn = storage.graph_env.read_txn().unwrap();
	let traversal = G::new(&storage, &txn, &arena)
		.v_from_id(&vector_id, true)
		.collect::<Result<Vec<_>, _>>()
		.unwrap();
	drop(txn);

	let arena = Bump::new();
	let mut txn = storage.graph_env.write_txn().unwrap();
	G::new_mut_from_iter(&storage, &mut txn, traversal.into_iter(), &arena)
		.update(&[("tag", Value::from("test"))])
		.collect::<Result<Vec<_>, _>>()
		.unwrap();
	txn.commit().unwrap();

	// Verify structural fields are preserved
	let arena = Bump::new();
	let txn = storage.graph_env.read_txn().unwrap();
	let fetched = G::new(&storage, &txn, &arena)
		.v_from_id(&vector_id, true)
		.collect::<Result<Vec<_>, _>>()
		.unwrap();

	match &fetched[0] {
		TraversalValue::Vector(v) => {
			assert_eq!(v.deleted, orig_deleted);
			assert_eq!(v.level, orig_level);
			assert_eq!(
				v.properties.as_ref().unwrap().get("tag").unwrap(),
				&Value::from("test")
			);
		}
		other => panic!("expected Vector, got {other:?}"),
	}
}
