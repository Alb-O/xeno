use std::borrow::Cow;
use std::collections::HashMap;

use sonic_rs::{Deserialize, Serialize};

pub use self::conv::*;
pub use self::serde::*;
use crate::helix_engine::types::EngineError;
use crate::protocol::date::Date;
use crate::protocol::value_error::{ValueError, ValueKind};
use crate::utils::id::ID;

pub mod cmp;
pub mod conv;
pub mod ops;
pub mod serde;

#[cfg(test)]
mod tests;

/// A flexible value type that can represent various property values in nodes and edges.
/// Handles both JSON and binary serialisation formats via custom implementaions of the Serialize and Deserialize traits.
#[derive(Clone, Debug, Default)]
pub enum Value {
	String(String),
	F32(f32),
	F64(f64),
	I8(i8),
	I16(i16),
	I32(i32),
	I64(i64),
	U8(u8),
	U16(u16),
	U32(u32),
	U64(u64),
	U128(u128),
	Date(Date),
	Boolean(bool),
	Id(ID),
	Array(Vec<Value>),
	Object(HashMap<String, Value>),
	#[default]
	Empty,
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize)]
pub enum Operator {
	#[serde(rename = "==")]
	Eq,
	#[serde(rename = "!=")]
	Neq,
	#[serde(rename = ">")]
	Gt,
	#[serde(rename = "<")]
	Lt,
	#[serde(rename = ">=")]
	Gte,
	#[serde(rename = "<=")]
	Lte,
}

impl Operator {
	#[inline]
	pub fn execute(&self, lhs: &Value, rhs: &Value) -> bool {
		match self {
			Operator::Eq => lhs == rhs,
			Operator::Neq => lhs != rhs,
			Operator::Gt => lhs > rhs,
			Operator::Lt => lhs < rhs,
			Operator::Gte => lhs >= rhs,
			Operator::Lte => lhs <= rhs,
		}
	}
}

pub trait FilterValues {
	fn compare(&self, value: &Value, operator: Option<Operator>) -> bool;
}

impl Value {
	pub fn kind(&self) -> ValueKind {
		match self {
			Value::String(_) => ValueKind::String,
			Value::F32(_) => ValueKind::F32,
			Value::F64(_) => ValueKind::F64,
			Value::I8(_) => ValueKind::I8,
			Value::I16(_) => ValueKind::I16,
			Value::I32(_) => ValueKind::I32,
			Value::I64(_) => ValueKind::I64,
			Value::U8(_) => ValueKind::U8,
			Value::U16(_) => ValueKind::U16,
			Value::U32(_) => ValueKind::U32,
			Value::U64(_) => ValueKind::U64,
			Value::U128(_) => ValueKind::U128,
			Value::Date(_) => ValueKind::Date,
			Value::Boolean(_) => ValueKind::Bool,
			Value::Id(_) => ValueKind::Id,
			Value::Array(_) => ValueKind::Array,
			Value::Object(_) => ValueKind::Object,
			Value::Empty => ValueKind::Empty,
		}
	}

	pub fn try_as_str(&self) -> Result<&str, ValueError> {
		match self {
			Value::String(s) => Ok(s.as_str()),
			_ => Err(ValueError::NotString { got: self.kind() }),
		}
	}

	/// String -> borrowed; primitive numeric/bool/Date/Id -> owned; Object/Array/Empty -> Err.
	pub fn try_stringify_primitive(&self) -> Result<Cow<'_, str>, ValueError> {
		match self {
			Value::String(s) => Ok(Cow::Borrowed(s.as_str())),
			Value::F32(f) => Ok(Cow::Owned(f.to_string())),
			Value::F64(f) => Ok(Cow::Owned(f.to_string())),
			Value::I8(i) => Ok(Cow::Owned(i.to_string())),
			Value::I16(i) => Ok(Cow::Owned(i.to_string())),
			Value::I32(i) => Ok(Cow::Owned(i.to_string())),
			Value::I64(i) => Ok(Cow::Owned(i.to_string())),
			Value::U8(u) => Ok(Cow::Owned(u.to_string())),
			Value::U16(u) => Ok(Cow::Owned(u.to_string())),
			Value::U32(u) => Ok(Cow::Owned(u.to_string())),
			Value::U64(u) => Ok(Cow::Owned(u.to_string())),
			Value::U128(u) => Ok(Cow::Owned(u.to_string())),
			Value::Date(d) => Ok(Cow::Owned(d.to_string())),
			Value::Boolean(b) => Ok(Cow::Owned(b.to_string())),
			Value::Id(id) => Ok(Cow::Owned(id.stringify())),
			_ => Err(ValueError::NotPrimitive { got: self.kind() }),
		}
	}

	pub fn try_contains(&self, needle: &str) -> Result<bool, ValueError> {
		let text = self.try_stringify_primitive()?;
		Ok(text.contains(needle))
	}

	pub fn inner_stringify(&self) -> String {
		self.try_stringify_primitive()
			.unwrap_or_else(|err| panic!("Value::inner_stringify failed: {err}"))
			.into_owned()
	}

	pub fn inner_str(&self) -> Cow<'_, str> {
		self.try_stringify_primitive()
			.unwrap_or_else(|err| panic!("Value::inner_str failed: {err}"))
	}

	pub fn to_variant_string(&self) -> &str {
		match self {
			Value::String(_) => "String",
			Value::F32(_) => "F32",
			Value::F64(_) => "F64",
			Value::I8(_) => "I8",
			Value::I16(_) => "I16",
			Value::I32(_) => "I32",
			Value::I64(_) => "I64",
			Value::U8(_) => "U8",
			Value::U16(_) => "U16",
			Value::U32(_) => "U32",
			Value::U64(_) => "U64",
			Value::U128(_) => "U128",
			Value::Date(_) => "Date",
			Value::Boolean(_) => "Boolean",
			Value::Id(_) => "Id",
			Value::Array(_) => "Array",
			Value::Object(_) => "Object",
			Value::Empty => "Empty",
		}
	}

	pub fn as_str(&self) -> &str {
		self.try_as_str()
			.unwrap_or_else(|err| panic!("Value::as_str failed: {err}"))
	}

	/// Checks if this value contains the needle value (as strings).
	/// Converts both values to their string representations and performs substring matching.
	pub fn contains(&self, needle: &str) -> bool {
		self.try_contains(needle)
			.unwrap_or_else(|err| panic!("Value::contains failed: {err}"))
	}

	#[inline]
	#[allow(unused_variables)] // default is not used but needed for function signature
	pub fn map_value_or(
		self,
		_default: bool,
		f: impl Fn(&Value) -> Result<bool, ValueError>,
	) -> Result<bool, EngineError> {
		f(&self).map_err(EngineError::from)
	}

	#[inline]
	pub fn is_in<T>(&self, values: &[T]) -> bool
	where
		T: PartialEq,
		Value: IntoPrimitive<T> + Into<T>,
	{
		values.contains(self.into_primitive())
	}
}

impl FilterValues for Value {
	#[inline]
	fn compare(&self, value: &Value, operator: Option<Operator>) -> bool {
		tracing::trace!(value1 = ?self, value2 = ?value, "value comparison");
		let comparison = match (self, value) {
			(Value::Array(a1), Value::Array(a2)) => a1
				.iter()
				.any(|a1_item| a2.iter().any(|a2_item| a1_item.compare(a2_item, operator))),
			(value, Value::Array(a)) => a.iter().any(|a_item| value.compare(a_item, operator)),
			(value1, value2) => match operator {
				Some(op) => op.execute(value1, value2),
				None => value1 == value2,
			},
		};
		tracing::trace!(comparison, "value comparison result");
		comparison
	}
}
