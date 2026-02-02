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
