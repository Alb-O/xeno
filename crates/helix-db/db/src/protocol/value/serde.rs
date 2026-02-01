use std::collections::HashMap;
use std::fmt;

use serde::de::{DeserializeSeed, VariantAccess, Visitor};
use serde::{Deserializer, Serializer};
use sonic_rs::{Deserialize, Serialize};

use super::Value;

/// Custom serialisation implementation for Value that removes enum variant names in JSON
/// whilst preserving them for binary formats like postcard.
impl Serialize for Value {
	fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
	where
		S: Serializer,
	{
		if serializer.is_human_readable() {
			match self {
				Value::String(s) => s.serialize(serializer),
				Value::F32(f) => f.serialize(serializer),
				Value::F64(f) => f.serialize(serializer),
				Value::I8(i) => i.serialize(serializer),
				Value::I16(i) => i.serialize(serializer),
				Value::I32(i) => i.serialize(serializer),
				Value::I64(i) => i.serialize(serializer),
				Value::U8(i) => i.serialize(serializer),
				Value::U16(i) => i.serialize(serializer),
				Value::U32(i) => i.serialize(serializer),
				Value::U64(i) => i.serialize(serializer),
				Value::U128(i) => i.serialize(serializer),
				Value::Boolean(b) => b.serialize(serializer),
				Value::Date(d) => d.serialize(serializer),
				Value::Id(id) => id.serialize(serializer),
				Value::Array(arr) => {
					use serde::ser::SerializeSeq;
					let mut seq = serializer.serialize_seq(Some(arr.len()))?;
					for value in arr {
						seq.serialize_element(&value)?;
					}
					seq.end()
				}
				Value::Object(obj) => {
					use serde::ser::SerializeMap;
					let mut map = serializer.serialize_map(Some(obj.len()))?;
					for (k, v) in obj {
						map.serialize_entry(k, v)?;
					}
					map.end()
				}
				Value::Empty => serializer.serialize_none(),
			}
		} else {
			match self {
				Value::String(s) => serializer.serialize_newtype_variant("Value", 0, "String", s),
				Value::F32(f) => serializer.serialize_newtype_variant("Value", 1, "F32", f),
				Value::F64(f) => serializer.serialize_newtype_variant("Value", 2, "F64", f),
				Value::I8(i) => serializer.serialize_newtype_variant("Value", 3, "I8", i),
				Value::I16(i) => serializer.serialize_newtype_variant("Value", 4, "I16", i),
				Value::I32(i) => serializer.serialize_newtype_variant("Value", 5, "I32", i),
				Value::I64(i) => serializer.serialize_newtype_variant("Value", 6, "I64", i),
				Value::U8(i) => serializer.serialize_newtype_variant("Value", 7, "U8", i),
				Value::U16(i) => serializer.serialize_newtype_variant("Value", 8, "U16", i),
				Value::U32(i) => serializer.serialize_newtype_variant("Value", 9, "U32", i),
				Value::U64(i) => serializer.serialize_newtype_variant("Value", 10, "U64", i),
				Value::U128(i) => serializer.serialize_newtype_variant("Value", 11, "U128", i),
				Value::Date(d) => serializer.serialize_newtype_variant("Value", 12, "Date", d),
				Value::Boolean(b) => {
					serializer.serialize_newtype_variant("Value", 13, "Boolean", b)
				}
				Value::Id(id) => serializer.serialize_newtype_variant("Value", 14, "Id", id),
				Value::Array(a) => serializer.serialize_newtype_variant("Value", 15, "Array", a),
				Value::Object(obj) => {
					serializer.serialize_newtype_variant("Value", 16, "Object", obj)
				}
				Value::Empty => serializer.serialize_unit_variant("Value", 17, "Empty"),
			}
		}
	}
}

/// Custom deserialisation implementation for Value that handles both JSON and binary formats.
/// For JSON, parses raw values directly.
/// For binary formats like postcard, reconstructs the full enum structure.
impl<'de> Deserialize<'de> for Value {
	fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
	where
		D: Deserializer<'de>,
	{
		/// Visitor implementation that handles conversion of raw values into Value enum variants.
		/// Supports both direct value parsing for JSON and enum variant parsing for binary formats.
		struct ValueVisitor;

		impl<'de> Visitor<'de> for ValueVisitor {
			type Value = Value;

			#[inline]
			fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
				formatter.write_str("a string, number, boolean, array, null, or Value enum")
			}

			#[inline]
			fn visit_str<E>(self, value: &str) -> Result<Value, E>
			where
				E: serde::de::Error,
			{
				Ok(Value::String(value.to_owned()))
			}

			#[inline]
			fn visit_string<E>(self, value: String) -> Result<Value, E>
			where
				E: serde::de::Error,
			{
				Ok(Value::String(value))
			}

			#[inline]
			fn visit_f32<E>(self, value: f32) -> Result<Value, E>
			where
				E: serde::de::Error,
			{
				Ok(Value::F32(value))
			}

			#[inline]
			fn visit_f64<E>(self, value: f64) -> Result<Value, E>
			where
				E: serde::de::Error,
			{
				Ok(Value::F64(value))
			}

			#[inline]
			fn visit_i8<E>(self, value: i8) -> Result<Value, E>
			where
				E: serde::de::Error,
			{
				Ok(Value::I8(value))
			}

			#[inline]
			fn visit_i16<E>(self, value: i16) -> Result<Value, E>
			where
				E: serde::de::Error,
			{
				Ok(Value::I16(value))
			}

			#[inline]
			fn visit_i32<E>(self, value: i32) -> Result<Value, E>
			where
				E: serde::de::Error,
			{
				Ok(Value::I32(value))
			}

			#[inline]
			fn visit_i64<E>(self, value: i64) -> Result<Value, E>
			where
				E: serde::de::Error,
			{
				Ok(Value::I64(value))
			}

			#[inline]
			fn visit_u8<E>(self, value: u8) -> Result<Value, E>
			where
				E: serde::de::Error,
			{
				Ok(Value::U8(value))
			}

			#[inline]
			fn visit_u16<E>(self, value: u16) -> Result<Value, E>
			where
				E: serde::de::Error,
			{
				Ok(Value::U16(value))
			}

			#[inline]
			fn visit_u32<E>(self, value: u32) -> Result<Value, E>
			where
				E: serde::de::Error,
			{
				Ok(Value::U32(value))
			}

			#[inline]
			fn visit_u64<E>(self, value: u64) -> Result<Value, E>
			where
				E: serde::de::Error,
			{
				Ok(Value::U64(value))
			}

			#[inline]
			fn visit_u128<E>(self, value: u128) -> Result<Value, E>
			where
				E: serde::de::Error,
			{
				Ok(Value::U128(value))
			}

			#[inline]
			fn visit_bool<E>(self, value: bool) -> Result<Value, E>
			where
				E: serde::de::Error,
			{
				Ok(Value::Boolean(value))
			}

			#[inline]
			fn visit_none<E>(self) -> Result<Value, E>
			where
				E: serde::de::Error,
			{
				Ok(Value::Empty)
			}

			/// Handles array values by recursively deserialising each element
			fn visit_seq<A>(self, mut seq: A) -> Result<Value, A::Error>
			where
				A: serde::de::SeqAccess<'de>,
			{
				let mut values = Vec::new();
				while let Some(value) = seq.next_element()? {
					values.push(value);
				}
				Ok(Value::Array(values))
			}

			/// Handles object values by recursively deserialising each key-value pair
			fn visit_map<A>(self, mut map: A) -> Result<Value, A::Error>
			where
				A: serde::de::MapAccess<'de>,
			{
				let mut object = HashMap::new();
				while let Some((key, value)) = map.next_entry()? {
					object.insert(key, value);
				}
				Ok(Value::Object(object))
			}

			/// Handles binary format deserialisation using numeric indices to identify variants
			/// Maps indices 0-5 to corresponding Value enum variants
			fn visit_enum<A>(self, data: A) -> Result<Self::Value, A::Error>
			where
				A: serde::de::EnumAccess<'de>,
			{
				let (variant_idx, variant_data) = data.variant_seed(VariantIdxDeserializer)?;
				match variant_idx {
					0 => Ok(Value::String(variant_data.newtype_variant()?)),
					1 => Ok(Value::F32(variant_data.newtype_variant()?)),
					2 => Ok(Value::F64(variant_data.newtype_variant()?)),
					3 => Ok(Value::I8(variant_data.newtype_variant()?)),
					4 => Ok(Value::I16(variant_data.newtype_variant()?)),
					5 => Ok(Value::I32(variant_data.newtype_variant()?)),
					6 => Ok(Value::I64(variant_data.newtype_variant()?)),
					7 => Ok(Value::U8(variant_data.newtype_variant()?)),
					8 => Ok(Value::U16(variant_data.newtype_variant()?)),
					9 => Ok(Value::U32(variant_data.newtype_variant()?)),
					10 => Ok(Value::U64(variant_data.newtype_variant()?)),
					11 => Ok(Value::U128(variant_data.newtype_variant()?)),
					12 => Ok(Value::Date(variant_data.newtype_variant()?)),
					13 => Ok(Value::Boolean(variant_data.newtype_variant()?)),
					14 => Ok(Value::Id(variant_data.newtype_variant()?)),
					15 => Ok(Value::Array(variant_data.newtype_variant()?)),
					16 => Ok(Value::Object(variant_data.newtype_variant()?)),
					17 => {
						variant_data.unit_variant()?;
						Ok(Value::Empty)
					}
					_ => Err(serde::de::Error::invalid_value(
						serde::de::Unexpected::Unsigned(variant_idx as u64),
						&"variant index 0 through 17",
					)),
				}
			}
		}

		/// Helper deserialiser for handling numeric variant indices in binary format
		struct VariantIdxDeserializer;

		impl<'de> DeserializeSeed<'de> for VariantIdxDeserializer {
			type Value = u32;
			#[inline]
			fn deserialize<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
			where
				D: Deserializer<'de>,
			{
				deserializer.deserialize_u32(self)
			}
		}

		impl<'de> Visitor<'de> for VariantIdxDeserializer {
			type Value = u32;

			#[inline]
			fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
				formatter.write_str("variant index")
			}

			#[inline]
			fn visit_u32<E>(self, v: u32) -> Result<Self::Value, E>
			where
				E: serde::de::Error,
			{
				Ok(v)
			}
		}
		// Choose deserialisation strategy based on format
		if deserializer.is_human_readable() {
			// For JSON, accept any value type
			deserializer.deserialize_any(ValueVisitor)
		} else {
			// For binary, use enum variant indices
			deserializer.deserialize_enum(
				"Value",
				&[
					"String", "F32", "F64", "I8", "I16", "I32", "I64", "U8", "U16", "U32", "U64",
					"U128", "Date", "Boolean", "Id", "Array", "Object", "Empty",
				],
				ValueVisitor,
			)
		}
	}
}

/// Module for custom serialisation of property hashmaps
/// Ensures consistent handling of Value enum serialisation within property maps
pub mod properties_format {
	use serde::ser::SerializeMap;

	use super::*;

	#[inline]
	pub fn serialize<S>(
		properties: &Option<HashMap<String, Value>>,
		serializer: S,
	) -> Result<S::Ok, S::Error>
	where
		S: Serializer,
	{
		match properties {
			Some(properties) => {
				let mut map = serializer.serialize_map(Some(properties.len()))?;
				for (k, v) in properties {
					map.serialize_entry(k, v)?;
				}
				map.end()
			}
			None => serializer.serialize_none(),
		}
	}

	#[inline]
	pub fn deserialize<'de, D>(deserializer: D) -> Result<Option<HashMap<String, Value>>, D::Error>
	where
		D: Deserializer<'de>,
	{
		match Option::<HashMap<String, Value>>::deserialize(deserializer) {
			Ok(properties) => Ok(properties),
			Err(e) => Err(e),
		}
	}
}
