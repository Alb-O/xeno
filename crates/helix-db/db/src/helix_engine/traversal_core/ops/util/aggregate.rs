use std::collections::HashMap;

use crate::helix_engine::traversal_core::traversal_iter::RoTraversalIterator;
use crate::helix_engine::traversal_core::traversal_value::TraversalValue;
use crate::helix_engine::types::EngineError;
use crate::utils::aggregate::{Aggregate, AggregateItem};

pub trait AggregateAdapter<'arena>: Iterator {
	fn aggregate_by(
		self,
		properties: &[String],
		should_count: bool,
	) -> Result<Aggregate<'arena>, EngineError>;
}

impl<'db, 'arena, 'txn, I: Iterator<Item = Result<TraversalValue<'arena>, EngineError>>>
	AggregateAdapter<'arena> for RoTraversalIterator<'db, 'arena, 'txn, I>
{
	fn aggregate_by(
		self,
		properties: &[String],
		should_count: bool,
	) -> Result<Aggregate<'arena>, EngineError> {
		let mut groups: HashMap<String, AggregateItem> = HashMap::new();

		let properties_len = properties.len();

		for item in self.inner {
			let item = item?;

			// TODO HANDLE COUNT
			// Pre-allocate with exact capacity - size is known from properties.len()
			let mut kvs = Vec::with_capacity(properties_len);
			let mut key_parts = Vec::with_capacity(properties_len);

			for property in properties {
				match item.get_property(property) {
					Some(val) => {
						key_parts.push(val.try_stringify_primitive()?.into_owned());
						kvs.push((property.to_string(), val.clone()));
					}
					None => {
						key_parts.push("null".to_string());
					}
				}
			}
			let key = key_parts.join("_");

			let group = groups.entry(key).or_default();
			group.values.insert(item);
			group.count += 1;
		}

		if should_count {
			Ok(Aggregate::Count(groups))
		} else {
			Ok(Aggregate::Group(groups))
		}
	}
}
