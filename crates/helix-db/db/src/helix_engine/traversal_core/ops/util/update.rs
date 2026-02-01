use itertools::Itertools;

use crate::helix_engine::traversal_core::traversal_iter::RwTraversalIterator;
use crate::helix_engine::traversal_core::traversal_value::TraversalValue;
use crate::helix_engine::types::{EngineError, TraversalError};
use crate::protocol::value::Value;
use crate::utils::properties::ImmutablePropertiesMap;

pub struct Update<I> {
	iter: I,
}

impl<'arena, I> Iterator for Update<I>
where
	I: Iterator<Item = Result<TraversalValue<'arena>, EngineError>>,
{
	type Item = Result<TraversalValue<'arena>, EngineError>;

	fn next(&mut self) -> Option<Self::Item> {
		self.iter.next()
	}
}

pub trait UpdateAdapter<'db, 'arena, 'txn>: Iterator {
	fn update(
		self,
		props: &[(&'static str, Value)],
	) -> RwTraversalIterator<
		'db,
		'arena,
		'txn,
		impl Iterator<Item = Result<TraversalValue<'arena>, EngineError>>,
	>;
}

impl<'db, 'arena, 'txn, I: Iterator<Item = Result<TraversalValue<'arena>, EngineError>>>
	UpdateAdapter<'db, 'arena, 'txn> for RwTraversalIterator<'db, 'arena, 'txn, I>
{
	fn update(
		self,
		props: &[(&'static str, Value)],
	) -> RwTraversalIterator<
		'db,
		'arena,
		'txn,
		impl Iterator<Item = Result<TraversalValue<'arena>, EngineError>>,
	> {
		let mut results = bumpalo::collections::Vec::new_in(self.arena);

		for item in self.inner {
			let res = (|| -> Result<TraversalValue<'arena>, EngineError> {
				match item? {
					TraversalValue::Node(mut node) => {
						match node.properties {
							None => {
								// Insert secondary indices
								for (k, v) in props.iter() {
									if let Some((db, secondary_index)) =
										self.storage.secondary_indices.get(*k)
									{
										let v_serialized = postcard::to_stdvec(v)?;
										secondary_index.insert(
											db,
											self.txn,
											&v_serialized,
											&node.id,
										)?;
									}
								}

								// Create properties map and insert node
								let map = ImmutablePropertiesMap::new(
									props.len(),
									props.iter().map(|(k, v)| (*k, v.clone())),
									self.arena,
								);

								node.properties = Some(map);
							}
							Some(old) => {
								for (k, v) in props.iter() {
									if let Some((db, secondary_index)) =
										self.storage.secondary_indices.get(*k)
									{
										// delete secondary indexes for the props changed
										if let Some(old_value) = old.get(k) {
											let old_serialized = postcard::to_stdvec(old_value)?;
											secondary_index.delete(
												db,
												self.txn,
												&old_serialized,
												&node.id,
											)?;
										}

										// create new secondary indexes for the props changed
										let v_serialized = postcard::to_stdvec(v)?;
										secondary_index.insert(
											db,
											self.txn,
											&v_serialized,
											&node.id,
										)?;
									}
								}

								let diff = props.iter().filter(|(k, _)| {
									!old.iter().map(|(old_k, _)| old_k).contains(k)
								});

								// find out how many new properties we'll need space for
								let len_diff = diff.clone().count();

								let merged = old
									.iter()
									.map(|(old_k, old_v)| {
										props
											.iter()
											.find_map(|(k, v)| old_k.eq(*k).then_some(v))
											.map_or_else(
												|| (old_k, old_v.clone()),
												|v| (old_k, v.clone()),
											)
									})
									.chain(diff.cloned());

								// make new props, updated by current props
								let new_map = ImmutablePropertiesMap::new(
									old.len() + len_diff,
									merged,
									self.arena,
								);

								node.properties = Some(new_map);
							}
						}

						let serialized_node = postcard::to_stdvec(&node)?;
						self.storage
							.nodes_db
							.put(self.txn, &node.id, &serialized_node)?;
						Ok(TraversalValue::Node(node))
					}
					TraversalValue::Edge(mut edge) => {
						match edge.properties {
							None => {
								// Create properties map and insert edge
								let map = ImmutablePropertiesMap::new(
									props.len(),
									props.iter().map(|(k, v)| (*k, v.clone())),
									self.arena,
								);

								edge.properties = Some(map);
							}
							Some(old) => {
								let diff = props.iter().filter(|(k, _)| {
									!old.iter().map(|(old_k, _)| old_k).contains(k)
								});

								// find out how many new properties we'll need space for
								let len_diff = diff.clone().count();

								let merged = old
									.iter()
									.map(|(old_k, old_v)| {
										props
											.iter()
											.find_map(|(k, v)| old_k.eq(*k).then_some(v))
											.map_or_else(
												|| (old_k, old_v.clone()),
												|v| (old_k, v.clone()),
											)
									})
									.chain(diff.cloned());

								// make new props, updated by current props
								let new_map = ImmutablePropertiesMap::new(
									old.len() + len_diff,
									merged,
									self.arena,
								);

								edge.properties = Some(new_map);
							}
						}

						let serialized_edge = postcard::to_stdvec(&edge)?;
						self.storage
							.edges_db
							.put(self.txn, &edge.id, &serialized_edge)?;
						Ok(TraversalValue::Edge(edge))
					}
					TraversalValue::Vector(mut vector) => {
						match vector.properties {
							None => {
								for (k, v) in props.iter() {
									if let Some((db, secondary_index)) =
										self.storage.secondary_indices.get(*k)
									{
										let v_serialized = postcard::to_stdvec(v)?;
										secondary_index.insert(
											db,
											self.txn,
											&v_serialized,
											&vector.id,
										)?;
									}
								}

								let map = ImmutablePropertiesMap::new(
									props.len(),
									props.iter().map(|(k, v)| (*k, v.clone())),
									self.arena,
								);

								vector.properties = Some(map);
							}
							Some(old) => {
								for (k, v) in props.iter() {
									if let Some((db, secondary_index)) =
										self.storage.secondary_indices.get(*k)
									{
										if let Some(old_value) = old.get(k) {
											let old_serialized = postcard::to_stdvec(old_value)?;
											secondary_index.delete(
												db,
												self.txn,
												&old_serialized,
												&vector.id,
											)?;
										}

										let v_serialized = postcard::to_stdvec(v)?;
										secondary_index.insert(
											db,
											self.txn,
											&v_serialized,
											&vector.id,
										)?;
									}
								}

								let diff = props.iter().filter(|(k, _)| {
									!old.iter().map(|(old_k, _)| old_k).contains(k)
								});

								let len_diff = diff.clone().count();

								let merged = old
									.iter()
									.map(|(old_k, old_v)| {
										props
											.iter()
											.find_map(|(k, v)| old_k.eq(*k).then_some(v))
											.map_or_else(
												|| (old_k, old_v.clone()),
												|v| (old_k, v.clone()),
											)
									})
									.chain(diff.cloned());

								let new_map = ImmutablePropertiesMap::new(
									old.len() + len_diff,
									merged,
									self.arena,
								);

								vector.properties = Some(new_map);
							}
						}

						self.storage.vectors.put_vector(self.txn, &vector)?;
						Ok(TraversalValue::Vector(vector))
					}
					TraversalValue::VectorNodeWithoutVectorData(mut vwd) => {
						match vwd.properties {
							None => {
								for (k, v) in props.iter() {
									if let Some((db, secondary_index)) =
										self.storage.secondary_indices.get(*k)
									{
										let v_serialized = postcard::to_stdvec(v)?;
										secondary_index.insert(
											db,
											self.txn,
											&v_serialized,
											&vwd.id,
										)?;
									}
								}

								let map = ImmutablePropertiesMap::new(
									props.len(),
									props.iter().map(|(k, v)| (*k, v.clone())),
									self.arena,
								);

								vwd.properties = Some(map);
							}
							Some(old) => {
								for (k, v) in props.iter() {
									if let Some((db, secondary_index)) =
										self.storage.secondary_indices.get(*k)
									{
										if let Some(old_value) = old.get(k) {
											let old_serialized = postcard::to_stdvec(old_value)?;
											secondary_index.delete(
												db,
												self.txn,
												&old_serialized,
												&vwd.id,
											)?;
										}

										let v_serialized = postcard::to_stdvec(v)?;
										secondary_index.insert(
											db,
											self.txn,
											&v_serialized,
											&vwd.id,
										)?;
									}
								}

								let diff = props.iter().filter(|(k, _)| {
									!old.iter().map(|(old_k, _)| old_k).contains(k)
								});

								let len_diff = diff.clone().count();

								let merged = old
									.iter()
									.map(|(old_k, old_v)| {
										props
											.iter()
											.find_map(|(k, v)| old_k.eq(*k).then_some(v))
											.map_or_else(
												|| (old_k, old_v.clone()),
												|v| (old_k, v.clone()),
											)
									})
									.chain(diff.cloned());

								let new_map = ImmutablePropertiesMap::new(
									old.len() + len_diff,
									merged,
									self.arena,
								);

								vwd.properties = Some(new_map);
							}
						}

						let serialized = postcard::to_stdvec(&vwd)?;
						self.storage.vectors.vector_properties_db.put(
							self.txn,
							&vwd.id,
							&serialized,
						)?;
						Ok(TraversalValue::VectorNodeWithoutVectorData(vwd))
					}
					_ => Err(TraversalError::UnsupportedValueType.into()),
				}
			})();

			let is_err = res.is_err();
			results.push(res);
			if is_err {
				break;
			}
		}

		RwTraversalIterator {
			inner: Update {
				iter: results.into_iter(),
			},
			storage: self.storage,
			arena: self.arena,
			txn: self.txn,
		}
	}
}
