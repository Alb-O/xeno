use std::cmp::Ordering;

use chrono::{DateTime, Utc};

use super::Value;
use crate::utils::id::ID;

impl Ord for Value {
	fn cmp(&self, other: &Self) -> Ordering {
		let to_i128 = |value: &Value| -> Option<i128> {
			match value {
				Value::I8(v) => Some(*v as i128),
				Value::I16(v) => Some(*v as i128),
				Value::I32(v) => Some(*v as i128),
				Value::I64(v) => Some(*v as i128),
				Value::U8(v) => Some(*v as i128),
				Value::U16(v) => Some(*v as i128),
				Value::U32(v) => Some(*v as i128),
				Value::U64(v) => Some(*v as i128),
				Value::U128(v) => {
					if *v <= i128::MAX as u128 {
						Some(*v as i128)
					} else {
						None
					}
				}
				_ => None,
			}
		};
		let is_integer = |value: &Value| -> bool {
			matches!(
				value,
				Value::I8(_)
					| Value::I16(_) | Value::I32(_)
					| Value::I64(_) | Value::U8(_)
					| Value::U16(_) | Value::U32(_)
					| Value::U64(_) | Value::U128(_)
			)
		};

		match (self, other) {
			(Value::String(s), Value::String(o)) => s.cmp(o),
			(Value::F32(s), Value::F32(o)) => match s.partial_cmp(o) {
				Some(o) => o,
				None => Ordering::Equal,
			},
			(Value::F64(s), Value::F64(o)) => match s.partial_cmp(o) {
				Some(o) => o,
				None => Ordering::Equal,
			},
			(Value::Date(s), Value::Date(o)) => s.cmp(o),
			(Value::Boolean(s), Value::Boolean(o)) => s.cmp(o),
			(Value::Array(s), Value::Array(o)) => s.cmp(o),
			(Value::Empty, Value::Empty) => Ordering::Equal,
			(Value::Empty, _) => Ordering::Less,
			(_, Value::Empty) => Ordering::Greater,
			(s, o) if is_integer(s) && is_integer(o) => match (to_i128(s), to_i128(o)) {
				(Some(s), Some(o)) => s.cmp(&o),
				(None, Some(_)) => Ordering::Greater,
				(Some(_), None) => Ordering::Less,
				(None, None) => match (self, other) {
					(Value::U128(s), Value::U128(o)) => s.cmp(o),
					_ => unreachable!(),
				},
			},
			(_, _) => Ordering::Equal,
		}
	}
}

impl PartialOrd for Value {
	fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
		Some(self.cmp(other))
	}
}

impl Eq for Value {}

impl PartialEq<Value> for Value {
	fn eq(&self, other: &Value) -> bool {
		let to_f64 = |value: &Value| -> Option<f64> {
			match value {
				Value::I8(v) => Some(*v as f64),
				Value::I16(v) => Some(*v as f64),
				Value::I32(v) => Some(*v as f64),
				Value::I64(v) => Some(*v as f64),
				Value::U8(v) => Some(*v as f64),
				Value::U16(v) => Some(*v as f64),
				Value::U32(v) => Some(*v as f64),
				Value::U64(v) => Some(*v as f64),
				Value::U128(v) => Some(*v as f64),
				Value::F32(v) => Some(*v as f64),
				Value::F64(v) => Some(*v),
				_ => None,
			}
		};

		let is_numeric = |value: &Value| -> bool {
			matches!(
				value,
				Value::I8(_)
					| Value::I16(_) | Value::I32(_)
					| Value::I64(_) | Value::U8(_)
					| Value::U16(_) | Value::U32(_)
					| Value::U64(_) | Value::U128(_)
					| Value::F32(_) | Value::F64(_)
			)
		};

		match (self, other) {
			(Value::String(s), Value::String(o)) => s == o,
			(Value::Date(s), Value::Date(o)) => s == o,
			(Value::Boolean(s), Value::Boolean(o)) => s == o,
			(Value::Array(s), Value::Array(o)) => s == o,
			(Value::Empty, Value::Empty) => true,
			(Value::Empty, _) => false,
			(_, Value::Empty) => false,

			(s, o) if is_numeric(s) && is_numeric(o) => match (to_f64(s), to_f64(o)) {
				(Some(s_val), Some(o_val)) => {
					if !matches!(self, Value::F32(_) | Value::F64(_))
						&& !matches!(other, Value::F32(_) | Value::F64(_))
					{
						self.cmp(other) == Ordering::Equal
					} else {
						s_val == o_val
					}
				}
				_ => false,
			},

			_ => false,
		}
	}
}

impl PartialEq<ID> for Value {
	fn eq(&self, other: &ID) -> bool {
		match self {
			Value::Id(id) => id == other,
			Value::String(s) => &ID::from(s) == other,
			Value::U128(u) => &ID::from(*u) == other,
			_ => false,
		}
	}
}
impl PartialEq<u8> for Value {
	fn eq(&self, other: &u8) -> bool {
		self == &Value::from(*other)
	}
}
impl PartialEq<u16> for Value {
	fn eq(&self, other: &u16) -> bool {
		self == &Value::from(*other)
	}
}
impl PartialEq<u32> for Value {
	fn eq(&self, other: &u32) -> bool {
		self == &Value::from(*other)
	}
}
impl PartialEq<u64> for Value {
	fn eq(&self, other: &u64) -> bool {
		self == &Value::from(*other)
	}
}
impl PartialEq<u128> for Value {
	fn eq(&self, other: &u128) -> bool {
		self == &Value::from(*other)
	}
}
impl PartialEq<i8> for Value {
	fn eq(&self, other: &i8) -> bool {
		self == &Value::from(*other)
	}
}
impl PartialEq<i16> for Value {
	fn eq(&self, other: &i16) -> bool {
		self == &Value::from(*other)
	}
}
impl PartialEq<i32> for Value {
	fn eq(&self, other: &i32) -> bool {
		self == &Value::from(*other)
	}
}
impl PartialEq<i64> for Value {
	fn eq(&self, other: &i64) -> bool {
		self == &Value::from(*other)
	}
}

impl PartialEq<f32> for Value {
	fn eq(&self, other: &f32) -> bool {
		self == &Value::from(*other)
	}
}
impl PartialEq<f64> for Value {
	fn eq(&self, other: &f64) -> bool {
		self == &Value::from(*other)
	}
}

impl PartialEq<String> for Value {
	fn eq(&self, other: &String) -> bool {
		match self {
			Value::String(s) => s == other,
			_ => false,
		}
	}
}

impl PartialEq<bool> for Value {
	fn eq(&self, other: &bool) -> bool {
		self == &Value::from(*other)
	}
}

impl PartialEq<&str> for Value {
	fn eq(&self, other: &&str) -> bool {
		self == &Value::from(*other)
	}
}

impl PartialEq<DateTime<Utc>> for Value {
	fn eq(&self, other: &DateTime<Utc>) -> bool {
		self == &Value::from(*other)
	}
}

impl PartialOrd<i8> for Value {
	fn partial_cmp(&self, other: &i8) -> Option<Ordering> {
		self.partial_cmp(&Value::from(*other))
	}
}
impl PartialOrd<i16> for Value {
	fn partial_cmp(&self, other: &i16) -> Option<Ordering> {
		self.partial_cmp(&Value::from(*other))
	}
}
impl PartialOrd<i32> for Value {
	fn partial_cmp(&self, other: &i32) -> Option<Ordering> {
		self.partial_cmp(&Value::from(*other))
	}
}
impl PartialOrd<i64> for Value {
	fn partial_cmp(&self, other: &i64) -> Option<Ordering> {
		self.partial_cmp(&Value::from(*other))
	}
}
impl PartialOrd<f32> for Value {
	fn partial_cmp(&self, other: &f32) -> Option<Ordering> {
		self.partial_cmp(&Value::from(*other))
	}
}
impl PartialOrd<f64> for Value {
	fn partial_cmp(&self, other: &f64) -> Option<Ordering> {
		self.partial_cmp(&Value::from(*other))
	}
}
impl PartialOrd<u8> for Value {
	fn partial_cmp(&self, other: &u8) -> Option<Ordering> {
		self.partial_cmp(&Value::from(*other))
	}
}
impl PartialOrd<u16> for Value {
	fn partial_cmp(&self, other: &u16) -> Option<Ordering> {
		self.partial_cmp(&Value::from(*other))
	}
}
impl PartialOrd<u32> for Value {
	fn partial_cmp(&self, other: &u32) -> Option<Ordering> {
		self.partial_cmp(&Value::from(*other))
	}
}
impl PartialOrd<u64> for Value {
	fn partial_cmp(&self, other: &u64) -> Option<Ordering> {
		self.partial_cmp(&Value::from(*other))
	}
}
impl PartialOrd<u128> for Value {
	fn partial_cmp(&self, other: &u128) -> Option<Ordering> {
		self.partial_cmp(&Value::from(*other))
	}
}

impl PartialOrd<ID> for Value {
	fn partial_cmp(&self, other: &ID) -> Option<Ordering> {
		match self {
			Value::Id(id) => id.partial_cmp(other),
			Value::String(s) => Some(ID::from(s).partial_cmp(other)?),
			Value::U128(u) => Some(u.partial_cmp(other)?),
			_ => None,
		}
	}
}

impl PartialOrd<DateTime<Utc>> for Value {
	fn partial_cmp(&self, other: &DateTime<Utc>) -> Option<Ordering> {
		self.partial_cmp(&Value::from(*other))
	}
}
