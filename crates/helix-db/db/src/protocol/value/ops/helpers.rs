use crate::protocol::value::Value;

impl Value {
	/// Convert any numeric Value to f64 for type promotion
	pub(super) fn to_f64(&self) -> Option<f64> {
		match self {
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
	}

	/// Check if value is a float type
	pub(super) fn is_float(&self) -> bool {
		matches!(self, Value::F32(_) | Value::F64(_))
	}

	/// Convert any signed integer Value to i64
	pub(super) fn to_i64(&self) -> Option<i64> {
		match self {
			Value::I8(v) => Some(*v as i64),
			Value::I16(v) => Some(*v as i64),
			Value::I32(v) => Some(*v as i64),
			Value::I64(v) => Some(*v),
			_ => None,
		}
	}

	/// Check if value is a signed integer
	pub(super) fn is_signed_int(&self) -> bool {
		matches!(
			self,
			Value::I8(_) | Value::I16(_) | Value::I32(_) | Value::I64(_)
		)
	}

	/// Check if value is an unsigned integer
	pub(super) fn is_unsigned_int(&self) -> bool {
		matches!(
			self,
			Value::U8(_) | Value::U16(_) | Value::U32(_) | Value::U64(_) | Value::U128(_)
		)
	}

	/// Check if value is any numeric type.
	pub(super) fn is_numeric(&self) -> bool {
		self.is_float() || self.is_signed_int() || self.is_unsigned_int()
	}
}
