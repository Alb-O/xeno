use std::cmp::Ordering;

use heed3::byteorder::BE;
use heed3::types::{Bytes, U128};
use heed3::{Database, RoTxn};

use super::binary_heap::BinaryHeap;
use crate::helix_engine::traversal_core::decode_postcard_str_prefix;
use crate::helix_engine::types::VectorError;
use crate::helix_engine::vector_core::vector::HVector;
use crate::helix_engine::vector_core::vector_without_data::VectorWithoutData;

#[derive(PartialEq)]
pub(super) struct Candidate {
	pub id: u128,
	pub distance: f64,
}

impl Eq for Candidate {}

impl PartialOrd for Candidate {
	fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
		Some(self.cmp(other))
	}
}

impl Ord for Candidate {
	fn cmp(&self, other: &Self) -> Ordering {
		other
			.distance
			.partial_cmp(&self.distance)
			.unwrap_or(Ordering::Equal)
	}
}

pub(super) trait HeapOps<'a, T> {
	/// Take the top k elements from the heap
	/// Used because using `.iter()` does not keep the order
	fn take_inord(&mut self, k: usize) -> BinaryHeap<'a, T>
	where
		T: Ord;

	/// Get the maximum element from the heap
	fn get_max<'q>(&'q self) -> Option<&'a T>
	where
		T: Ord,
		'q: 'a;
}

impl<'a, T> HeapOps<'a, T> for BinaryHeap<'a, T> {
	#[inline(always)]
	fn take_inord(&mut self, k: usize) -> BinaryHeap<'a, T>
	where
		T: Ord,
	{
		let mut result = BinaryHeap::with_capacity(self.arena, k);
		for _ in 0..k {
			if let Some(item) = self.pop() {
				result.push(item);
			} else {
				break;
			}
		}
		result
	}

	#[inline(always)]
	fn get_max<'q>(&'q self) -> Option<&'a T>
	where
		T: Ord,
		'q: 'a,
	{
		self.iter().max()
	}
}

pub trait VectorFilter<'db, 'arena, 'txn, 'q> {
	fn to_vec_with_filter<F, const SHOULD_CHECK_DELETED: bool>(
		self,
		k: usize,
		filter: Option<&'arena [F]>,
		label: &'arena str,
		txn: &'txn RoTxn<'db>,
		db: Database<U128<BE>, Bytes>,
		arena: &'arena bumpalo::Bump,
	) -> Result<bumpalo::collections::Vec<'arena, HVector<'arena>>, VectorError>
	where
		F: Fn(&HVector<'arena>, &'txn RoTxn<'db>) -> bool;
}

impl<'db, 'arena, 'txn, 'q> VectorFilter<'db, 'arena, 'txn, 'q>
	for BinaryHeap<'arena, HVector<'arena>>
{
	#[inline(always)]
	fn to_vec_with_filter<F, const SHOULD_CHECK_DELETED: bool>(
		mut self,
		k: usize,
		filter: Option<&'arena [F]>,
		label: &'arena str,
		txn: &'txn RoTxn<'db>,
		db: Database<U128<BE>, Bytes>,
		arena: &'arena bumpalo::Bump,
	) -> Result<bumpalo::collections::Vec<'arena, HVector<'arena>>, VectorError>
	where
		F: Fn(&HVector<'arena>, &'txn RoTxn<'db>) -> bool,
	{
		let mut result = bumpalo::collections::Vec::with_capacity_in(k, arena);
		for _ in 0..k {
			// while pop check filters and pop until one passes
			while let Some(mut item) = self.pop() {
				let properties = match db.get(txn, &item.id)? {
					Some(bytes) => Some(VectorWithoutData::from_bytes(arena, bytes, item.id)?),
					None => None, // TODO: maybe should be an error?
				};

				let Some(properties) = properties else {
					continue;
				};

				if SHOULD_CHECK_DELETED && properties.deleted {
					continue;
				}

				if properties.label == label
					&& (filter.is_none() || filter.unwrap().iter().all(|f| f(&item, txn)))
				{
					item.expand_from_vector_without_data(properties);
					result.push(item);
					break;
				}
			}
		}

		Ok(result)
	}
}

pub fn check_deleted(data: &[u8]) -> bool {
	let (_, label_end) = decode_postcard_str_prefix(data)
		.expect("value too short: label field missing from vector on insertion");

	// version is a single byte immediately after the label
	let deleted_index = label_end + 1;

	assert!(
		data.len() > deleted_index,
		"data too short to contain deleted flag after label+version"
	);
	data[deleted_index] == 1
}
