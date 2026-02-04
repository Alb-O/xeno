use crate::protocol::value::Value;
use crate::protocol::value_error::{NumUnaryOp, ValueError};

impl Value {
	pub fn try_num_unary(&self, op: NumUnaryOp) -> Result<Value, ValueError> {
		match op {
			NumUnaryOp::Abs => match self {
				Value::I8(v) => Ok(Value::I8(v.wrapping_abs())),
				Value::I16(v) => Ok(Value::I16(v.wrapping_abs())),
				Value::I32(v) => Ok(Value::I32(v.wrapping_abs())),
				Value::I64(v) => Ok(Value::I64(v.wrapping_abs())),
				Value::U8(v) => Ok(Value::U8(*v)),
				Value::U16(v) => Ok(Value::U16(*v)),
				Value::U32(v) => Ok(Value::U32(*v)),
				Value::U64(v) => Ok(Value::U64(*v)),
				Value::U128(v) => Ok(Value::U128(*v)),
				Value::F32(v) => Ok(Value::F32(v.abs())),
				Value::F64(v) => Ok(Value::F64(v.abs())),
				_ => Err(ValueError::NonNumericUnary {
					op,
					got: self.kind(),
				}),
			},
			NumUnaryOp::Sqrt => {
				if !self.is_numeric() {
					return Err(ValueError::NonNumericUnary {
						op,
						got: self.kind(),
					});
				}
				match self {
					Value::I8(v) if *v < 0 => Err(ValueError::Domain { op: "sqrt" }),
					Value::I16(v) if *v < 0 => Err(ValueError::Domain { op: "sqrt" }),
					Value::I32(v) if *v < 0 => Err(ValueError::Domain { op: "sqrt" }),
					Value::I64(v) if *v < 0 => Err(ValueError::Domain { op: "sqrt" }),
					Value::F32(v) if *v < 0.0 => Err(ValueError::Domain { op: "sqrt" }),
					Value::F64(v) if *v < 0.0 => Err(ValueError::Domain { op: "sqrt" }),
					_ => {
						let val = self.to_f64().ok_or(ValueError::NonNumericUnary {
							op,
							got: self.kind(),
						})?;
						Ok(Value::F64(val.sqrt()))
					}
				}
			}
		}
	}

	pub fn try_abs(&self) -> Result<Value, ValueError> {
		self.try_num_unary(NumUnaryOp::Abs)
	}

	pub fn try_sqrt(&self) -> Result<Value, ValueError> {
		self.try_num_unary(NumUnaryOp::Sqrt)
	}

	/// Compute absolute value, preserving type for integers
	pub fn abs(&self) -> Value {
		self.try_abs()
			.unwrap_or_else(|err| panic!("Value::abs failed: {err}"))
	}

	/// Compute square root, returns F64
	pub fn sqrt(&self) -> Value {
		self.try_sqrt()
			.unwrap_or_else(|err| panic!("Value::sqrt failed: {err}"))
	}
}
