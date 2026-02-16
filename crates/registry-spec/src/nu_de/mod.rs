//! Serde deserializer from `xeno_nu_protocol::Value` (compile-time only).
//!
//! Replaces the previous JSON bridge (`Value` → `serde_json::Value` → `T`).
//! Handles the subset of Nushell types used in asset files:
//! Bool, Int, Float, String, List, Record, Nothing.

use serde::de::{self, DeserializeSeed, IntoDeserializer, MapAccess, SeqAccess, Visitor};
use serde::forward_to_deserialize_any;
use xeno_nu_protocol::{Record, Value};

/// Deserialize `T` directly from a `xeno_nu_protocol::Value`.
pub fn from_nu_value<T: de::DeserializeOwned>(value: &Value) -> Result<T, Error> {
	T::deserialize(NuDe(value))
}

#[derive(Debug)]
pub struct Error(String);

impl std::fmt::Display for Error {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		f.write_str(&self.0)
	}
}

impl std::error::Error for Error {}

impl de::Error for Error {
	fn custom<T: std::fmt::Display>(msg: T) -> Self {
		Error(msg.to_string())
	}
}

struct NuDe<'a>(&'a Value);

impl<'de, 'a> de::Deserializer<'de> for NuDe<'a> {
	type Error = Error;

	fn deserialize_any<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Error> {
		match self.0 {
			Value::Bool { val, .. } => visitor.visit_bool(*val),
			Value::Int { val, .. } => visitor.visit_i64(*val),
			Value::Float { val, .. } => visitor.visit_f64(*val),
			Value::String { val, .. } => visitor.visit_str(val),
			Value::List { vals, .. } => visitor.visit_seq(NuSeq { iter: vals.iter() }),
			Value::Record { val, .. } => visitor.visit_map(NuMap::new(val)),
			Value::Nothing { .. } => visitor.visit_none(),
			other => Err(de::Error::custom(format!(
				"unsupported NUON type: {:?} (quote scalars; don't use duration/filesize literals)",
				other.get_type()
			))),
		}
	}

	fn deserialize_option<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Error> {
		match self.0 {
			Value::Nothing { .. } => visitor.visit_none(),
			_ => visitor.visit_some(self),
		}
	}

	fn deserialize_enum<V: Visitor<'de>>(self, _name: &'static str, _variants: &'static [&'static str], visitor: V) -> Result<V::Value, Error> {
		match self.0 {
			// Externally tagged: { VariantName: { fields... } } or { VariantName: value }
			Value::Record { val, .. } if val.len() == 1 => {
				let (variant, value) = val.iter().next().unwrap();
				visitor.visit_enum(NuEnum { variant, value })
			}
			// Unit variant as plain string
			Value::String { val, .. } => visitor.visit_enum(val.as_str().into_deserializer()),
			other => Err(de::Error::custom(format!(
				"expected record with one key (externally tagged enum) or string, got {:?}",
				other.get_type()
			))),
		}
	}

	fn deserialize_newtype_struct<V: Visitor<'de>>(self, _name: &'static str, visitor: V) -> Result<V::Value, Error> {
		visitor.visit_newtype_struct(self)
	}

	fn deserialize_unit<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Error> {
		visitor.visit_unit()
	}

	fn deserialize_unit_struct<V: Visitor<'de>>(self, _name: &'static str, visitor: V) -> Result<V::Value, Error> {
		visitor.visit_unit()
	}

	forward_to_deserialize_any! {
		bool i8 i16 i32 i64 i128 u8 u16 u32 u64 u128 f32 f64 char str string
		bytes byte_buf seq tuple tuple_struct map struct identifier ignored_any
	}
}

// --- Sequence ---

struct NuSeq<'a> {
	iter: std::slice::Iter<'a, Value>,
}

impl<'de, 'a> SeqAccess<'de> for NuSeq<'a> {
	type Error = Error;

	fn next_element_seed<T: DeserializeSeed<'de>>(&mut self, seed: T) -> Result<Option<T::Value>, Error> {
		match self.iter.next() {
			Some(val) => seed.deserialize(NuDe(val)).map(Some),
			None => Ok(None),
		}
	}
}

// --- Map ---

struct NuMap<'a> {
	iter: Box<dyn Iterator<Item = (&'a str, &'a Value)> + 'a>,
	pending_value: Option<&'a Value>,
}

impl<'a> NuMap<'a> {
	fn new(record: &'a Record) -> Self {
		Self {
			iter: Box::new(record.iter().map(|(k, v)| (k.as_str(), v))),
			pending_value: None,
		}
	}
}

impl<'de, 'a> MapAccess<'de> for NuMap<'a> {
	type Error = Error;

	fn next_key_seed<K: DeserializeSeed<'de>>(&mut self, seed: K) -> Result<Option<K::Value>, Error> {
		match self.iter.next() {
			Some((key, value)) => {
				self.pending_value = Some(value);
				seed.deserialize(key.into_deserializer()).map(Some)
			}
			None => Ok(None),
		}
	}

	fn next_value_seed<V: DeserializeSeed<'de>>(&mut self, seed: V) -> Result<V::Value, Error> {
		let value = self.pending_value.take().expect("next_value_seed called before next_key_seed");
		seed.deserialize(NuDe(value))
	}
}

// --- Enum ---

struct NuEnum<'a> {
	variant: &'a str,
	value: &'a Value,
}

impl<'de, 'a> de::EnumAccess<'de> for NuEnum<'a> {
	type Error = Error;
	type Variant = NuVariant<'a>;

	fn variant_seed<V: DeserializeSeed<'de>>(self, seed: V) -> Result<(V::Value, Self::Variant), Error> {
		let variant = seed.deserialize(self.variant.into_deserializer())?;
		Ok((variant, NuVariant(self.value)))
	}
}

struct NuVariant<'a>(&'a Value);

impl<'de, 'a> de::VariantAccess<'de> for NuVariant<'a> {
	type Error = Error;

	fn unit_variant(self) -> Result<(), Error> {
		Ok(())
	}

	fn newtype_variant_seed<T: DeserializeSeed<'de>>(self, seed: T) -> Result<T::Value, Error> {
		seed.deserialize(NuDe(self.0))
	}

	fn tuple_variant<V: Visitor<'de>>(self, _len: usize, visitor: V) -> Result<V::Value, Error> {
		de::Deserializer::deserialize_seq(NuDe(self.0), visitor)
	}

	fn struct_variant<V: Visitor<'de>>(self, _fields: &'static [&'static str], visitor: V) -> Result<V::Value, Error> {
		de::Deserializer::deserialize_any(NuDe(self.0), visitor)
	}
}

#[cfg(test)]
mod tests;
