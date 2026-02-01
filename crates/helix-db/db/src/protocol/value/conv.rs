use std::collections::HashMap;

use super::Value;
use crate::helixc::generator::utils::GenRef;
use crate::protocol::date::Date;
use crate::utils::id::ID;

impl From<&str> for Value {
	#[inline]
	fn from(s: &str) -> Self {
		Value::String(s.trim_matches('"').to_string())
	}
}

impl From<String> for Value {
	#[inline]
	fn from(s: String) -> Self {
		Value::String(s.trim_matches('"').to_string())
	}
}
impl From<bool> for Value {
	#[inline]
	fn from(b: bool) -> Self {
		Value::Boolean(b)
	}
}

impl From<f32> for Value {
	#[inline]
	fn from(f: f32) -> Self {
		Value::F32(f)
	}
}

impl From<f64> for Value {
	#[inline]
	fn from(f: f64) -> Self {
		Value::F64(f)
	}
}

impl From<i8> for Value {
	#[inline]
	fn from(i: i8) -> Self {
		Value::I8(i)
	}
}

impl From<i16> for Value {
	#[inline]
	fn from(i: i16) -> Self {
		Value::I16(i)
	}
}

impl From<i32> for Value {
	#[inline]
	fn from(i: i32) -> Self {
		Value::I32(i)
	}
}

impl From<i64> for Value {
	#[inline]
	fn from(i: i64) -> Self {
		Value::I64(i)
	}
}

impl From<u8> for Value {
	#[inline]
	fn from(i: u8) -> Self {
		Value::U8(i)
	}
}

impl From<u16> for Value {
	#[inline]
	fn from(i: u16) -> Self {
		Value::U16(i)
	}
}

impl From<u32> for Value {
	#[inline]
	fn from(i: u32) -> Self {
		Value::U32(i)
	}
}

impl From<u64> for Value {
	#[inline]
	fn from(i: u64) -> Self {
		Value::U64(i)
	}
}

impl From<u128> for Value {
	#[inline]
	fn from(i: u128) -> Self {
		Value::U128(i)
	}
}

impl From<Vec<Value>> for Value {
	#[inline]
	fn from(v: Vec<Value>) -> Self {
		Value::Array(v)
	}
}

impl From<Vec<bool>> for Value {
	#[inline(always)]
	fn from(v: Vec<bool>) -> Self {
		Value::Array(v.into_iter().map(|v| v.into()).collect())
	}
}

impl From<Vec<String>> for Value {
	#[inline(always)]
	fn from(v: Vec<String>) -> Self {
		Value::Array(v.into_iter().map(|v| v.into()).collect())
	}
}

impl From<Vec<i64>> for Value {
	#[inline(always)]
	fn from(v: Vec<i64>) -> Self {
		Value::Array(v.into_iter().map(|v| v.into()).collect())
	}
}

impl From<Vec<i32>> for Value {
	#[inline(always)]
	fn from(v: Vec<i32>) -> Self {
		Value::Array(v.into_iter().map(|v| v.into()).collect())
	}
}

impl From<Vec<i16>> for Value {
	#[inline(always)]
	fn from(v: Vec<i16>) -> Self {
		Value::Array(v.into_iter().map(|v| v.into()).collect())
	}
}

impl From<Vec<i8>> for Value {
	#[inline(always)]
	fn from(v: Vec<i8>) -> Self {
		Value::Array(v.into_iter().map(|v| v.into()).collect())
	}
}

impl From<Vec<u128>> for Value {
	#[inline(always)]
	fn from(v: Vec<u128>) -> Self {
		Value::Array(v.into_iter().map(|v| v.into()).collect())
	}
}

impl From<Vec<u64>> for Value {
	#[inline(always)]
	fn from(v: Vec<u64>) -> Self {
		Value::Array(v.into_iter().map(|v| v.into()).collect())
	}
}

impl From<Vec<u32>> for Value {
	#[inline(always)]
	fn from(v: Vec<u32>) -> Self {
		Value::Array(v.into_iter().map(|v| v.into()).collect())
	}
}

impl From<Vec<u16>> for Value {
	#[inline(always)]
	fn from(v: Vec<u16>) -> Self {
		Value::Array(v.into_iter().map(|v| v.into()).collect())
	}
}

impl From<Vec<u8>> for Value {
	#[inline(always)]
	fn from(v: Vec<u8>) -> Self {
		Value::Array(v.into_iter().map(|v| v.into()).collect())
	}
}

impl From<Vec<f64>> for Value {
	#[inline(always)]
	fn from(v: Vec<f64>) -> Self {
		Value::Array(v.into_iter().map(|v| v.into()).collect())
	}
}

impl From<Vec<f32>> for Value {
	#[inline(always)]
	fn from(v: Vec<f32>) -> Self {
		Value::Array(v.into_iter().map(|v| v.into()).collect())
	}
}

impl From<usize> for Value {
	#[inline]
	fn from(v: usize) -> Self {
		if cfg!(target_pointer_width = "64") {
			Value::U64(v as u64)
		} else {
			Value::U128(v as u128)
		}
	}
}

impl From<Value> for String {
	#[inline]
	fn from(v: Value) -> Self {
		match v {
			Value::String(s) => s,
			_ => panic!("Value is not a string"),
		}
	}
}

impl From<ID> for Value {
	#[inline]
	fn from(id: ID) -> Self {
		Value::String(id.to_string())
	}
}

impl<'a, K> From<&'a K> for Value
where
	K: Into<Value> + serde::Serialize + Clone,
{
	#[inline]
	fn from(k: &'a K) -> Self {
		k.clone().into()
	}
}

impl From<chrono::DateTime<chrono::Utc>> for Value {
	#[inline]
	fn from(dt: chrono::DateTime<chrono::Utc>) -> Self {
		Value::String(dt.to_rfc3339())
	}
}

impl From<Value> for GenRef<String> {
	fn from(v: Value) -> Self {
		match v {
			Value::String(s) => GenRef::Literal(s),
			Value::I8(i) => GenRef::Std(format!("{i}")),
			Value::I16(i) => GenRef::Std(format!("{i}")),
			Value::I32(i) => GenRef::Std(format!("{i}")),
			Value::I64(i) => GenRef::Std(format!("{i}")),
			Value::F32(f) => GenRef::Std(format!("{f:?}")), // {:?} forces decimal point
			Value::F64(f) => GenRef::Std(format!("{f:?}")),
			Value::Boolean(b) => GenRef::Std(format!("{b}")),
			Value::U8(u) => GenRef::Std(format!("{u}")),
			Value::U16(u) => GenRef::Std(format!("{u}")),
			Value::U32(u) => GenRef::Std(format!("{u}")),
			Value::U64(u) => GenRef::Std(format!("{u}")),
			Value::U128(u) => GenRef::Std(format!("{u}")),
			Value::Date(d) => GenRef::Std(format!("{d:?}")),
			Value::Id(id) => GenRef::Literal(id.stringify()),
			Value::Array(_a) => unimplemented!(),
			Value::Object(_o) => unimplemented!(),
			Value::Empty => GenRef::Literal("".to_string()),
		}
	}
}

impl From<Value> for i8 {
	fn from(val: Value) -> Self {
		match val {
			Value::I8(i) => i,
			Value::I16(i) => i as i8,
			Value::I32(i) => i as i8,
			Value::I64(i) => i as i8,
			Value::U8(i) => i as i8,
			Value::U16(i) => i as i8,
			Value::U32(i) => i as i8,
			Value::U64(i) => i as i8,
			Value::U128(i) => i as i8,
			Value::F32(i) => i as i8,
			Value::F64(i) => i as i8,
			Value::Boolean(i) => i as i8,
			Value::String(s) => s.parse::<i8>().unwrap(),
			_ => panic!("Value cannot be cast to i8"),
		}
	}
}

impl From<Value> for i16 {
	fn from(val: Value) -> Self {
		match val {
			Value::I16(i) => i,
			Value::I8(i) => i as i16,
			Value::I32(i) => i as i16,
			Value::I64(i) => i as i16,
			Value::U8(i) => i as i16,
			Value::U16(i) => i as i16,
			Value::U32(i) => i as i16,
			Value::U64(i) => i as i16,
			Value::U128(i) => i as i16,
			Value::F32(i) => i as i16,
			Value::F64(i) => i as i16,
			Value::Boolean(i) => i as i16,
			Value::String(s) => s.parse::<i16>().unwrap(),
			_ => panic!("Value cannot be cast to i16"),
		}
	}
}

impl From<Value> for i32 {
	fn from(val: Value) -> Self {
		match val {
			Value::I32(i) => i,
			Value::I8(i) => i as i32,
			Value::I16(i) => i as i32,
			Value::I64(i) => i as i32,
			Value::U8(i) => i as i32,
			Value::U16(i) => i as i32,
			Value::U32(i) => i as i32,
			Value::U64(i) => i as i32,
			Value::U128(i) => i as i32,
			Value::F32(i) => i as i32,
			Value::F64(i) => i as i32,
			Value::Boolean(i) => i as i32,
			Value::String(s) => s.parse::<i32>().unwrap(),
			_ => panic!("Value cannot be cast to i32"),
		}
	}
}

impl From<Value> for i64 {
	fn from(val: Value) -> Self {
		match val {
			Value::I64(i) => i,
			Value::I8(i) => i as i64,
			Value::I16(i) => i as i64,
			Value::I32(i) => i as i64,
			Value::U8(i) => i as i64,
			Value::U16(i) => i as i64,
			Value::U32(i) => i as i64,
			Value::U64(i) => i as i64,
			Value::U128(i) => i as i64,
			Value::F32(i) => i as i64,
			Value::F64(i) => i as i64,
			Value::Boolean(i) => i as i64,
			Value::String(s) => s.parse::<i64>().unwrap(),
			_ => panic!("Value cannot be cast to i64"),
		}
	}
}

impl From<Value> for u8 {
	fn from(val: Value) -> Self {
		match val {
			Value::U8(i) => i,
			Value::I8(i) => i as u8,
			Value::I16(i) => i as u8,
			Value::I32(i) => i as u8,
			Value::I64(i) => i as u8,
			Value::U16(i) => i as u8,
			Value::U32(i) => i as u8,
			Value::U64(i) => i as u8,
			Value::U128(i) => i as u8,
			Value::F32(i) => i as u8,
			Value::F64(i) => i as u8,
			Value::Boolean(i) => i as u8,
			Value::String(s) => s.parse::<u8>().unwrap(),
			_ => panic!("Value cannot be cast to u8"),
		}
	}
}

impl From<Value> for u16 {
	fn from(val: Value) -> Self {
		match val {
			Value::U16(i) => i,
			Value::I8(i) => i as u16,
			Value::I16(i) => i as u16,
			Value::I32(i) => i as u16,
			Value::I64(i) => i as u16,
			Value::U8(i) => i as u16,
			Value::U32(i) => i as u16,
			Value::U64(i) => i as u16,
			Value::U128(i) => i as u16,
			Value::F32(i) => i as u16,
			Value::F64(i) => i as u16,
			Value::Boolean(i) => i as u16,
			Value::String(s) => s.parse::<u16>().unwrap(),
			_ => panic!("Value cannot be cast to u16"),
		}
	}
}

impl From<Value> for u32 {
	fn from(val: Value) -> Self {
		match val {
			Value::U32(i) => i,
			Value::I8(i) => i as u32,
			Value::I16(i) => i as u32,
			Value::I32(i) => i as u32,
			Value::I64(i) => i as u32,
			Value::U8(i) => i as u32,
			Value::U16(i) => i as u32,
			Value::U64(i) => i as u32,
			Value::U128(i) => i as u32,
			Value::F32(i) => i as u32,
			Value::F64(i) => i as u32,
			Value::Boolean(i) => i as u32,
			Value::String(s) => s.parse::<u32>().unwrap(),
			_ => panic!("Value cannot be cast to u32"),
		}
	}
}

impl From<Value> for u64 {
	fn from(val: Value) -> Self {
		match val {
			Value::U64(i) => i,
			Value::I8(i) => i as u64,
			Value::I16(i) => i as u64,
			Value::I32(i) => i as u64,
			Value::U8(i) => i as u64,
			Value::U16(i) => i as u64,
			Value::U32(i) => i as u64,
			Value::U128(i) => i as u64,
			Value::F32(i) => i as u64,
			Value::F64(i) => i as u64,
			Value::Boolean(i) => i as u64,
			Value::String(s) => s.parse::<u64>().unwrap(),
			_ => panic!("Value cannot be cast to u64"),
		}
	}
}

impl From<Value> for u128 {
	fn from(val: Value) -> Self {
		match val {
			Value::U128(i) => i,
			Value::I8(i) => i as u128,
			Value::I16(i) => i as u128,
			Value::I32(i) => i as u128,
			Value::I64(i) => i as u128,
			Value::U8(i) => i as u128,
			Value::U16(i) => i as u128,
			Value::U32(i) => i as u128,
			Value::U64(i) => i as u128,
			Value::F32(i) => i as u128,
			Value::F64(i) => i as u128,
			Value::Boolean(i) => i as u128,
			Value::String(s) => s.parse::<u128>().unwrap(),
			_ => panic!("Value cannot be cast to u128"),
		}
	}
}

impl From<Value> for Date {
	fn from(val: Value) -> Self {
		match val {
			Value::String(s) => Date::new(&Value::String(s)).unwrap(),
			Value::I64(i) => Date::new(&Value::I64(i)).unwrap(),
			Value::U64(i) => Date::new(&Value::U64(i)).unwrap(),
			_ => panic!("Value cannot be cast to date"),
		}
	}
}
impl From<Value> for bool {
	fn from(val: Value) -> Self {
		match val {
			Value::Boolean(b) => b,
			_ => panic!("Value cannot be cast to boolean"),
		}
	}
}

impl From<Value> for ID {
	fn from(val: Value) -> Self {
		match val {
			Value::Id(id) => id,
			Value::String(s) => ID::from(s),
			Value::U128(i) => ID::from(i),
			_ => panic!("Value cannot be cast to id"),
		}
	}
}

impl From<Value> for Vec<Value> {
	fn from(val: Value) -> Self {
		match val {
			Value::Array(a) => a,
			_ => panic!("Value cannot be cast to array"),
		}
	}
}

impl From<Value> for HashMap<String, Value> {
	fn from(val: Value) -> Self {
		match val {
			Value::Object(o) => o,
			_ => panic!("Value cannot be cast to object"),
		}
	}
}

impl From<Value> for f32 {
	fn from(val: Value) -> Self {
		match val {
			Value::F32(f) => f,
			Value::F64(f) => f as f32,
			Value::I8(i) => i as f32,
			Value::I16(i) => i as f32,
			Value::I32(i) => i as f32,
			Value::I64(i) => i as i32 as f32, // Adjusted to avoid float overflow check issues if any
			Value::U8(i) => i as f32,
			Value::U16(i) => i as f32,
			Value::U32(i) => i as f32,
			Value::U64(i) => i as f32,
			Value::U128(i) => i as f32,
			Value::String(s) => s.parse::<f32>().unwrap(),
			_ => panic!("Value cannot be cast to f32"),
		}
	}
}

impl From<Value> for f64 {
	fn from(val: Value) -> Self {
		match val {
			Value::F64(f) => f,
			Value::F32(f) => f as f64,
			Value::I8(i) => i as f64,
			Value::I16(i) => i as f64,
			Value::I32(i) => i as f64,
			Value::I64(i) => i as f64,
			Value::U8(i) => i as f64,
			Value::U16(i) => i as f64,
			Value::U32(i) => i as f64,
			Value::U64(i) => i as f64,
			Value::U128(i) => i as f64,
			Value::String(s) => s.parse::<f64>().unwrap(),
			_ => panic!("Value cannot be cast to f64"),
		}
	}
}

pub mod casting {
	use super::*;
	use crate::helixc::parser::types::FieldType;

	#[derive(Debug)]
	pub enum CastType {
		String,
		I8,
		I16,
		I32,
		I64,
		U8,
		U16,
		U32,
		U64,
		U128,
		F32,
		F64,
		Date,
		Boolean,
		Id,
		Array,
		Object,
		Empty,
	}

	pub fn cast(value: Value, cast_type: CastType) -> Value {
		match cast_type {
			CastType::String => Value::String(value.inner_stringify()),
			CastType::I8 => Value::I8(value.into()),
			CastType::I16 => Value::I16(value.into()),
			CastType::I32 => Value::I32(value.into()),
			CastType::I64 => Value::I64(value.into()),
			CastType::U8 => Value::U8(value.into()),
			CastType::U16 => Value::U16(value.into()),
			CastType::U32 => Value::U32(value.into()),
			CastType::U64 => Value::U64(value.into()),
			CastType::U128 => Value::U128(value.into()),
			CastType::F32 => Value::F32(value.into()),
			CastType::F64 => Value::F64(value.into()),
			CastType::Date => Value::Date(value.into()),
			CastType::Boolean => Value::Boolean(value.into()),
			CastType::Id => Value::Id(value.into()),
			CastType::Array => Value::Array(value.into()),
			CastType::Object => Value::Object(value.into()),
			CastType::Empty => Value::Empty,
		}
	}

	impl std::fmt::Display for CastType {
		fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
			match self {
				CastType::String => write!(f, "String"),
				CastType::I8 => write!(f, "I8"),
				CastType::I16 => write!(f, "I16"),
				CastType::I32 => write!(f, "I32"),
				CastType::I64 => write!(f, "I64"),
				CastType::U8 => write!(f, "U8"),
				CastType::U16 => write!(f, "U16"),
				CastType::U32 => write!(f, "U32"),
				CastType::U64 => write!(f, "U64"),
				CastType::U128 => write!(f, "U128"),
				CastType::F32 => write!(f, "F32"),
				CastType::F64 => write!(f, "F64"),
				CastType::Date => write!(f, "Date"),
				CastType::Boolean => write!(f, "Boolean"),
				CastType::Id => write!(f, "Id"),
				CastType::Array => write!(f, "Array"),
				CastType::Object => write!(f, "Object"),
				CastType::Empty => write!(f, "Empty"),
			}
		}
	}

	impl From<FieldType> for CastType {
		fn from(value: FieldType) -> Self {
			match value {
				FieldType::String => CastType::String,
				FieldType::I8 => CastType::I8,
				FieldType::I16 => CastType::I16,
				FieldType::I32 => CastType::I32,
				FieldType::I64 => CastType::I64,
				FieldType::U8 => CastType::U8,
				FieldType::U16 => CastType::U16,
				FieldType::U32 => CastType::U32,
				FieldType::U64 => CastType::U64,
				FieldType::U128 => CastType::U128,
				FieldType::F32 => CastType::F32,
				FieldType::F64 => CastType::F64,
				FieldType::Date => CastType::Date,
				FieldType::Boolean => CastType::Boolean,
				FieldType::Uuid => CastType::Id,
				FieldType::Array(_) => CastType::Array,
				FieldType::Object(_) => CastType::Object,
				_ => CastType::Empty,
			}
		}
	}
}

pub trait IntoPrimitive<T> {
	fn into_primitive(&self) -> &T;
}

impl IntoPrimitive<String> for Value {
	fn into_primitive(&self) -> &String {
		match self {
			Value::String(s) => s,
			_ => panic!("Value is not a string"),
		}
	}
}

impl IntoPrimitive<i8> for Value {
	fn into_primitive(&self) -> &i8 {
		match self {
			Value::I8(i) => i,
			_ => panic!("Value is not an i8"),
		}
	}
}

impl IntoPrimitive<i16> for Value {
	fn into_primitive(&self) -> &i16 {
		match self {
			Value::I16(i) => i,
			_ => panic!("Value is not an i16"),
		}
	}
}

impl IntoPrimitive<i32> for Value {
	fn into_primitive(&self) -> &i32 {
		match self {
			Value::I32(i) => i,
			_ => panic!("Value is not an i32"),
		}
	}
}

impl IntoPrimitive<i64> for Value {
	fn into_primitive(&self) -> &i64 {
		match self {
			Value::I64(i) => i,
			_ => panic!("Value is not an i64"),
		}
	}
}

impl IntoPrimitive<u8> for Value {
	fn into_primitive(&self) -> &u8 {
		match self {
			Value::U8(i) => i,
			_ => panic!("Value is not an u8"),
		}
	}
}

impl IntoPrimitive<u16> for Value {
	fn into_primitive(&self) -> &u16 {
		match self {
			Value::U16(i) => i,
			_ => panic!("Value is not an u16"),
		}
	}
}

impl IntoPrimitive<u32> for Value {
	fn into_primitive(&self) -> &u32 {
		match self {
			Value::U32(i) => i,
			_ => panic!("Value is not an u32"),
		}
	}
}

impl IntoPrimitive<u64> for Value {
	fn into_primitive(&self) -> &u64 {
		match self {
			Value::U64(i) => i,
			_ => panic!("Value is not an u64"),
		}
	}
}

impl IntoPrimitive<u128> for Value {
	fn into_primitive(&self) -> &u128 {
		match self {
			Value::U128(i) => i,
			_ => panic!("Value is not an u128"),
		}
	}
}

impl IntoPrimitive<f32> for Value {
	fn into_primitive(&self) -> &f32 {
		match self {
			Value::F32(i) => i,
			_ => panic!("Value is not an f32"),
		}
	}
}

impl IntoPrimitive<f64> for Value {
	fn into_primitive(&self) -> &f64 {
		match self {
			Value::F64(i) => i,
			_ => panic!("Value is not an f64"),
		}
	}
}

impl IntoPrimitive<bool> for Value {
	fn into_primitive(&self) -> &bool {
		match self {
			Value::Boolean(i) => i,
			_ => panic!("Value is not a boolean"),
		}
	}
}

impl IntoPrimitive<ID> for Value {
	fn into_primitive(&self) -> &ID {
		match self {
			Value::Id(i) => i,
			_ => panic!("Value is not an id"),
		}
	}
}

impl IntoPrimitive<Vec<Value>> for Value {
	fn into_primitive(&self) -> &Vec<Value> {
		match self {
			Value::Array(i) => i,
			_ => panic!("Value is not an array"),
		}
	}
}

impl IntoPrimitive<HashMap<String, Value>> for Value {
	fn into_primitive(&self) -> &HashMap<String, Value> {
		match self {
			Value::Object(i) => i,
			_ => panic!("Value is not an object"),
		}
	}
}

impl IntoPrimitive<Date> for Value {
	fn into_primitive(&self) -> &Date {
		match self {
			Value::Date(i) => i,
			_ => panic!("Value is not a date"),
		}
	}
}
