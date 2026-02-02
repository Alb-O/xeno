use std::cmp::Ordering;

use bumpalo::Bump;

use super::binary_heap::BinaryHeap;
use super::utils::{Candidate, HeapOps, check_deleted};

// ============================================================================
// BinaryHeap
// ============================================================================

#[test]
fn test_log2_fast_zero() {
	// log2_fast is a private inner fn of rebuild_tail, so we exercise it
	// indirectly through append, which calls rebuild_tail.
	// This test verifies that appending to an empty heap (start == 0)
	// doesn't panic from a leading_zeros underflow.
	let arena = Bump::new();
	let mut a = BinaryHeap::<i32>::new(&arena);
	let mut b = BinaryHeap::<i32>::new(&arena);
	b.push(1);
	a.append(&mut b);
	assert_eq!(a.pop(), Some(1));
}

#[test]
fn test_empty_heap_operations() {
	let arena = Bump::new();
	let mut heap = BinaryHeap::<i32>::new(&arena);
	assert!(heap.is_empty());
	assert_eq!(heap.len(), 0);
	assert_eq!(heap.pop(), None);
	assert_eq!(heap.peek(), None);
}

#[test]
fn test_single_element_heap() {
	let arena = Bump::new();
	let mut heap = BinaryHeap::new(&arena);
	heap.push(42);
	assert_eq!(heap.len(), 1);
	assert_eq!(heap.peek(), Some(&42));
	assert_eq!(heap.pop(), Some(42));
	assert!(heap.is_empty());
}

#[test]
fn test_heap_ordering() {
	let arena = Bump::new();
	let mut heap = BinaryHeap::new(&arena);
	heap.push(3);
	heap.push(1);
	heap.push(4);
	heap.push(1);
	heap.push(5);

	let mut sorted = Vec::new();
	while let Some(v) = heap.pop() {
		sorted.push(v);
	}
	assert_eq!(sorted, vec![5, 4, 3, 1, 1]);
}

#[test]
fn test_append_rebuilds_correctly() {
	let arena = Bump::new();
	let mut a = BinaryHeap::new(&arena);
	a.push(1);
	a.push(3);

	let mut b = BinaryHeap::new(&arena);
	b.push(5);
	b.push(2);
	b.push(4);

	a.append(&mut b);
	assert!(b.is_empty());
	assert_eq!(a.len(), 5);
	assert_eq!(a.pop(), Some(5));
}

// ============================================================================
// Candidate Ord/PartialOrd
// ============================================================================

#[test]
fn test_candidate_ord_by_distance() {
	// Candidate uses reverse ordering (smaller distance = greater in ordering)
	// This is for min-heap behavior in a max-heap
	let c1 = Candidate {
		id: 1,
		distance: 0.5,
	};
	let c2 = Candidate {
		id: 2,
		distance: 1.0,
	};
	let c3 = Candidate {
		id: 3,
		distance: 0.2,
	};

	// c3 has smallest distance, so it should be "greatest" in ordering
	assert!(c3 > c1);
	assert!(c3 > c2);
	assert!(c1 > c2);

	// Verify the reverse: larger distance = smaller in ordering
	assert!(c2 < c1);
	assert!(c2 < c3);
}

#[test]
fn test_candidate_partial_ord_consistency() {
	let c1 = Candidate {
		id: 1,
		distance: 0.5,
	};
	let c2 = Candidate {
		id: 2,
		distance: 0.5,
	};

	// Same distance should be equal in ordering
	assert_eq!(c1.cmp(&c2), Ordering::Equal);
	assert_eq!(c1.partial_cmp(&c2), Some(Ordering::Equal));
}

#[test]
fn test_candidate_equality() {
	let c1 = Candidate {
		id: 1,
		distance: 0.5,
	};
	let c2 = Candidate {
		id: 1,
		distance: 0.5,
	};
	let c3 = Candidate {
		id: 2,
		distance: 0.5,
	};

	assert!(c1 == c2);
	// Different id but same distance - not equal
	assert!(c1 != c3);
}

// ============================================================================
// HeapOps
// ============================================================================

#[test]
fn test_heap_ops_take_inord() {
	let arena = Bump::new();
	let mut heap: BinaryHeap<i32> = BinaryHeap::new(&arena);

	// Push elements in random order
	heap.push(5);
	heap.push(1);
	heap.push(8);
	heap.push(3);
	heap.push(9);

	// Take top 3 elements
	let result = heap.take_inord(3);

	// Result should be a new heap with 3 elements
	assert_eq!(result.len(), 3);

	// Original heap should have remaining elements
	assert_eq!(heap.len(), 2);
}

#[test]
fn test_heap_ops_take_inord_more_than_available() {
	let arena = Bump::new();
	let mut heap: BinaryHeap<i32> = BinaryHeap::new(&arena);

	heap.push(5);
	heap.push(1);

	// Try to take more than available
	let result = heap.take_inord(10);

	// Should only take what's available
	assert_eq!(result.len(), 2);
	assert_eq!(heap.len(), 0);
}

#[test]
fn test_heap_ops_get_max() {
	let arena = Bump::new();
	let mut heap: BinaryHeap<i32> = BinaryHeap::new(&arena);

	heap.push(5);
	heap.push(1);
	heap.push(8);
	heap.push(3);

	// Get max without removal
	let max = heap.get_max();
	assert_eq!(max, Some(&8));

	// Heap should still have all elements
	assert_eq!(heap.len(), 4);
}

#[test]
fn test_heap_ops_get_max_empty() {
	let arena = Bump::new();
	let heap: BinaryHeap<i32> = BinaryHeap::new(&arena);

	let max = heap.get_max();
	assert_eq!(max, None);
}

// ============================================================================
// check_deleted
// ============================================================================

#[test]
fn test_check_deleted_returns_false() {
	let label = "test";
	let mut data = postcard::to_stdvec(&label).unwrap();
	data.push(0);
	data.push(0);

	assert!(!check_deleted(&data));
}

#[test]
fn test_check_deleted_returns_true() {
	let label = "test";
	let mut data = postcard::to_stdvec(&label).unwrap();
	data.push(0);
	data.push(1);

	assert!(check_deleted(&data));
}

// ============================================================================
// Vector Core Tests
// ============================================================================

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
	let result =
		core.insert::<fn(&HVector, &heed3::RoTxn) -> bool>(&mut wtxn, label, &vec256, None, &arena);

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
		.search::<fn(&HVector, &heed3::RoTxn) -> bool>(&rtxn, &vec, 10, label, None, false, &arena)
		.unwrap();

	for res in results {
		assert_ne!(
			res.id, id_a,
			"Deleted vector should not be returned in search results"
		);
	}
}
