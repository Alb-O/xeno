use crate::helix_engine::traversal_core::decode_postcard_str_prefix;
use crate::helix_engine::traversal_core::traversal_iter::RoTraversalIterator;
use crate::helix_engine::traversal_core::traversal_value::TraversalValue;
use crate::helix_engine::types::{EngineError, VectorError};
use crate::helix_engine::vector_core::vector_without_data::VectorWithoutData;

pub trait VFromTypeAdapter<'db, 'arena, 'txn>:
	Iterator<Item = Result<TraversalValue<'arena>, EngineError>>
{
	/// Returns an iterator containing the vector with the given label.
	///
	/// Note that the `label` cannot be empty and must be a valid, existing vector label.
	fn v_from_type(
		self,
		label: &'arena str,
		get_vector_data: bool,
	) -> RoTraversalIterator<
		'db,
		'arena,
		'txn,
		impl Iterator<Item = Result<TraversalValue<'arena>, EngineError>>,
	>;
}

impl<'db, 'arena, 'txn, I: Iterator<Item = Result<TraversalValue<'arena>, EngineError>>>
	VFromTypeAdapter<'db, 'arena, 'txn> for RoTraversalIterator<'db, 'arena, 'txn, I>
{
	#[inline]
	fn v_from_type(
		self,
		label: &'arena str,
		get_vector_data: bool,
	) -> RoTraversalIterator<
		'db,
		'arena,
		'txn,
		impl Iterator<Item = Result<TraversalValue<'arena>, EngineError>>,
	> {
		let label_bytes = label.as_bytes();
		let iter = self
			.storage
			.vectors
			.vector_properties_db
			.iter(self.txn)
			.unwrap()
			.filter_map(move |item| {
				if let Ok((id, value)) = item {
					// get label via bytes directly
					let Some((label_in_lmdb, label_end)) = decode_postcard_str_prefix(value) else {
						return None;
					};

					// skip single byte for version
					let version_index = label_end;

					// get bool for deleted
					let deleted_index = version_index + 1;
					let deleted = value[deleted_index] == 1;

					if deleted {
						return None;
					}

					if label_in_lmdb == label_bytes {
						let vector_without_data =
							VectorWithoutData::from_bytes(self.arena, value, id)
								.map_err(|e| VectorError::ConversionError(e.to_string()))
								.ok()?;

						if get_vector_data {
							let mut vector = match self
								.storage
								.vectors
								.get_raw_vector_data(self.txn, id, label, self.arena)
							{
								Ok(bytes) => bytes,
								Err(VectorError::VectorDeleted) => return None,
								Err(e) => return Some(Err(EngineError::from(e))),
							};
							vector.expand_from_vector_without_data(vector_without_data);
							return Some(Ok(TraversalValue::Vector(vector)));
						} else {
							return Some(Ok(TraversalValue::VectorNodeWithoutVectorData(
								vector_without_data,
							)));
						}
					} else {
						return None;
					}
				}
				None
			});

		RoTraversalIterator {
			storage: self.storage,
			arena: self.arena,
			txn: self.txn,
			inner: iter,
		}
	}
}
