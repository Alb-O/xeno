use super::Value;
use crate::protocol::value_error::{NumBinaryOp, NumUnaryOp, ValueError};

impl Value {
	/// Convert any numeric Value to f64 for type promotion
	fn to_f64(&self) -> Option<f64> {
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
	fn is_float(&self) -> bool {
		matches!(self, Value::F32(_) | Value::F64(_))
	}

	/// Convert any signed integer Value to i64
	fn to_i64(&self) -> Option<i64> {
		match self {
			Value::I8(v) => Some(*v as i64),
			Value::I16(v) => Some(*v as i64),
			Value::I32(v) => Some(*v as i64),
			Value::I64(v) => Some(*v),
			_ => None,
		}
	}

	/// Check if value is a signed integer
	fn is_signed_int(&self) -> bool {
		matches!(
			self,
			Value::I8(_) | Value::I16(_) | Value::I32(_) | Value::I64(_)
		)
	}

	/// Check if value is an unsigned integer
	fn is_unsigned_int(&self) -> bool {
		matches!(
			self,
			Value::U8(_) | Value::U16(_) | Value::U32(_) | Value::U64(_) | Value::U128(_)
		)
	}

	/// Check if value is any numeric type.
	fn is_numeric(&self) -> bool {
		self.is_float() || self.is_signed_int() || self.is_unsigned_int()
	}

	fn num_binary_add(lhs: Value, rhs: Value) -> Result<Value, ValueError> {
		let lhs_kind = lhs.kind();
		let rhs_kind = rhs.kind();
		match (lhs, rhs) {
			// Float + Float cases
			(Value::F64(a), Value::F64(b)) => Ok(Value::F64(a + b)),
			(Value::F32(a), Value::F32(b)) => Ok(Value::F32(a + b)),
			(Value::F64(a), Value::F32(b)) => Ok(Value::F64(a + (b as f64))),
			(Value::F32(a), Value::F64(b)) => Ok(Value::F64((a as f64) + b)),

			// Same-type signed integer additions
			(Value::I8(a), Value::I8(b)) => Ok(Value::I8(a.wrapping_add(b))),
			(Value::I16(a), Value::I16(b)) => Ok(Value::I16(a.wrapping_add(b))),
			(Value::I32(a), Value::I32(b)) => Ok(Value::I32(a.wrapping_add(b))),
			(Value::I64(a), Value::I64(b)) => Ok(Value::I64(a.wrapping_add(b))),

			// Same-type unsigned integer additions
			(Value::U8(a), Value::U8(b)) => Ok(Value::U8(a.wrapping_add(b))),
			(Value::U16(a), Value::U16(b)) => Ok(Value::U16(a.wrapping_add(b))),
			(Value::U32(a), Value::U32(b)) => Ok(Value::U32(a.wrapping_add(b))),
			(Value::U64(a), Value::U64(b)) => Ok(Value::U64(a.wrapping_add(b))),
			(Value::U128(a), Value::U128(b)) => Ok(Value::U128(a.wrapping_add(b))),

			// Int + Float → F64 (any integer with any float promotes to F64)
			(a, b) if (a.is_signed_int() || a.is_unsigned_int()) && b.is_float() => {
				let a_f64 = a.to_f64().unwrap();
				let b_f64 = b.to_f64().unwrap();
				Ok(Value::F64(a_f64 + b_f64))
			}
			(a, b) if a.is_float() && (b.is_signed_int() || b.is_unsigned_int()) => {
				let a_f64 = a.to_f64().unwrap();
				let b_f64 = b.to_f64().unwrap();
				Ok(Value::F64(a_f64 + b_f64))
			}

			// Cross-type signed integer additions → I64
			(a, b) if a.is_signed_int() && b.is_signed_int() => {
				let a_i64 = a.to_i64().unwrap();
				let b_i64 = b.to_i64().unwrap();
				Ok(Value::I64(a_i64.wrapping_add(b_i64)))
			}

			// Cross-type unsigned integer additions → U128 (safest)
			(Value::U8(a), b) if b.is_unsigned_int() => {
				let b_val = match b {
					Value::U8(v) => v as u128,
					Value::U16(v) => v as u128,
					Value::U32(v) => v as u128,
					Value::U64(v) => v as u128,
					Value::U128(v) => v,
					_ => {
						return Err(ValueError::NonNumericBinary {
							op: NumBinaryOp::Add,
							lhs: lhs_kind,
							rhs: rhs_kind,
						});
					}
				};
				Ok(Value::U128((a as u128).wrapping_add(b_val)))
			}
			(Value::U16(a), b) if b.is_unsigned_int() => {
				let b_val = match b {
					Value::U8(v) => v as u128,
					Value::U16(v) => v as u128,
					Value::U32(v) => v as u128,
					Value::U64(v) => v as u128,
					Value::U128(v) => v,
					_ => {
						return Err(ValueError::NonNumericBinary {
							op: NumBinaryOp::Add,
							lhs: lhs_kind,
							rhs: rhs_kind,
						});
					}
				};
				Ok(Value::U128((a as u128).wrapping_add(b_val)))
			}
			(Value::U32(a), b) if b.is_unsigned_int() => {
				let b_val = match b {
					Value::U8(v) => v as u128,
					Value::U16(v) => v as u128,
					Value::U32(v) => v as u128,
					Value::U64(v) => v as u128,
					Value::U128(v) => v,
					_ => {
						return Err(ValueError::NonNumericBinary {
							op: NumBinaryOp::Add,
							lhs: lhs_kind,
							rhs: rhs_kind,
						});
					}
				};
				Ok(Value::U128((a as u128).wrapping_add(b_val)))
			}
			(Value::U64(a), b) if b.is_unsigned_int() => {
				let b_val = match b {
					Value::U8(v) => v as u128,
					Value::U16(v) => v as u128,
					Value::U32(v) => v as u128,
					Value::U64(v) => v as u128,
					Value::U128(v) => v,
					_ => {
						return Err(ValueError::NonNumericBinary {
							op: NumBinaryOp::Add,
							lhs: lhs_kind,
							rhs: rhs_kind,
						});
					}
				};
				Ok(Value::U128((a as u128).wrapping_add(b_val)))
			}
			(Value::U128(a), b) if b.is_unsigned_int() => {
				let b_val = match b {
					Value::U8(v) => v as u128,
					Value::U16(v) => v as u128,
					Value::U32(v) => v as u128,
					Value::U64(v) => v as u128,
					Value::U128(v) => v,
					_ => {
						return Err(ValueError::NonNumericBinary {
							op: NumBinaryOp::Add,
							lhs: lhs_kind,
							rhs: rhs_kind,
						});
					}
				};
				Ok(Value::U128(a.wrapping_add(b_val)))
			}

			// Signed + Unsigned → I64 (safe widening that can represent both)
			(a, b) if a.is_signed_int() && b.is_unsigned_int() => {
				let a_i64 = a.to_i64().unwrap();
				let b_i64 = match b {
					Value::U8(v) => v as i64,
					Value::U16(v) => v as i64,
					Value::U32(v) => v as i64,
					Value::U64(v) => v as i64,
					Value::U128(v) => v as i64,
					_ => {
						return Err(ValueError::NonNumericBinary {
							op: NumBinaryOp::Add,
							lhs: lhs_kind,
							rhs: rhs_kind,
						});
					}
				};
				Ok(Value::I64(a_i64.wrapping_add(b_i64)))
			}
			(a, b) if a.is_unsigned_int() && b.is_signed_int() => {
				let a_i64 = match a {
					Value::U8(v) => v as i64,
					Value::U16(v) => v as i64,
					Value::U32(v) => v as i64,
					Value::U64(v) => v as i64,
					Value::U128(v) => v as i64,
					_ => {
						return Err(ValueError::NonNumericBinary {
							op: NumBinaryOp::Add,
							lhs: lhs_kind,
							rhs: rhs_kind,
						});
					}
				};
				let b_i64 = b.to_i64().unwrap();
				Ok(Value::I64(a_i64.wrapping_add(b_i64)))
			}

			// String concatenation
			(Value::String(mut a), Value::String(b)) => {
				a.push_str(b.as_str());
				Ok(Value::String(a))
			}

			// Array concatenation
			(Value::Array(mut a), Value::Array(b)) => {
				a.extend(b);
				Ok(Value::Array(a))
			}

			// Invalid combinations
			_ => Err(ValueError::NonNumericBinary {
				op: NumBinaryOp::Add,
				lhs: lhs_kind,
				rhs: rhs_kind,
			}),
		}
	}

	fn num_binary_sub(lhs: Value, rhs: Value) -> Result<Value, ValueError> {
		let lhs_kind = lhs.kind();
		let rhs_kind = rhs.kind();
		match (lhs, rhs) {
			// Float - Float cases
			(Value::F64(a), Value::F64(b)) => Ok(Value::F64(a - b)),
			(Value::F32(a), Value::F32(b)) => Ok(Value::F32(a - b)),
			(Value::F64(a), Value::F32(b)) => Ok(Value::F64(a - (b as f64))),
			(Value::F32(a), Value::F64(b)) => Ok(Value::F64((a as f64) - b)),

			// Same-type signed integer subtractions
			(Value::I8(a), Value::I8(b)) => Ok(Value::I8(a.wrapping_sub(b))),
			(Value::I16(a), Value::I16(b)) => Ok(Value::I16(a.wrapping_sub(b))),
			(Value::I32(a), Value::I32(b)) => Ok(Value::I32(a.wrapping_sub(b))),
			(Value::I64(a), Value::I64(b)) => Ok(Value::I64(a.wrapping_sub(b))),

			// Same-type unsigned integer subtractions
			(Value::U8(a), Value::U8(b)) => Ok(Value::U8(a.wrapping_sub(b))),
			(Value::U16(a), Value::U16(b)) => Ok(Value::U16(a.wrapping_sub(b))),
			(Value::U32(a), Value::U32(b)) => Ok(Value::U32(a.wrapping_sub(b))),
			(Value::U64(a), Value::U64(b)) => Ok(Value::U64(a.wrapping_sub(b))),
			(Value::U128(a), Value::U128(b)) => Ok(Value::U128(a.wrapping_sub(b))),

			// Int - Float → F64
			(a, b) if (a.is_signed_int() || a.is_unsigned_int()) && b.is_float() => {
				let a_f64 = a.to_f64().unwrap();
				let b_f64 = b.to_f64().unwrap();
				Ok(Value::F64(a_f64 - b_f64))
			}
			(a, b) if a.is_float() && (b.is_signed_int() || b.is_unsigned_int()) => {
				let a_f64 = a.to_f64().unwrap();
				let b_f64 = b.to_f64().unwrap();
				Ok(Value::F64(a_f64 - b_f64))
			}

			// Cross-type signed integer subtractions → I64
			(a, b) if a.is_signed_int() && b.is_signed_int() => {
				let a_i64 = a.to_i64().unwrap();
				let b_i64 = b.to_i64().unwrap();
				Ok(Value::I64(a_i64.wrapping_sub(b_i64)))
			}

			// Cross-type unsigned integer subtractions → U128
			(Value::U8(a), b) if b.is_unsigned_int() => {
				let b_val = match b {
					Value::U8(v) => v as u128,
					Value::U16(v) => v as u128,
					Value::U32(v) => v as u128,
					Value::U64(v) => v as u128,
					Value::U128(v) => v,
					_ => {
						return Err(ValueError::NonNumericBinary {
							op: NumBinaryOp::Sub,
							lhs: lhs_kind,
							rhs: rhs_kind,
						});
					}
				};
				Ok(Value::U128((a as u128).wrapping_sub(b_val)))
			}
			(Value::U16(a), b) if b.is_unsigned_int() => {
				let b_val = match b {
					Value::U8(v) => v as u128,
					Value::U16(v) => v as u128,
					Value::U32(v) => v as u128,
					Value::U64(v) => v as u128,
					Value::U128(v) => v,
					_ => {
						return Err(ValueError::NonNumericBinary {
							op: NumBinaryOp::Sub,
							lhs: lhs_kind,
							rhs: rhs_kind,
						});
					}
				};
				Ok(Value::U128((a as u128).wrapping_sub(b_val)))
			}
			(Value::U32(a), b) if b.is_unsigned_int() => {
				let b_val = match b {
					Value::U8(v) => v as u128,
					Value::U16(v) => v as u128,
					Value::U32(v) => v as u128,
					Value::U64(v) => v as u128,
					Value::U128(v) => v,
					_ => {
						return Err(ValueError::NonNumericBinary {
							op: NumBinaryOp::Sub,
							lhs: lhs_kind,
							rhs: rhs_kind,
						});
					}
				};
				Ok(Value::U128((a as u128).wrapping_sub(b_val)))
			}
			(Value::U64(a), b) if b.is_unsigned_int() => {
				let b_val = match b {
					Value::U8(v) => v as u128,
					Value::U16(v) => v as u128,
					Value::U32(v) => v as u128,
					Value::U64(v) => v as u128,
					Value::U128(v) => v,
					_ => {
						return Err(ValueError::NonNumericBinary {
							op: NumBinaryOp::Sub,
							lhs: lhs_kind,
							rhs: rhs_kind,
						});
					}
				};
				Ok(Value::U128((a as u128).wrapping_sub(b_val)))
			}
			(Value::U128(a), b) if b.is_unsigned_int() => {
				let b_val = match b {
					Value::U8(v) => v as u128,
					Value::U16(v) => v as u128,
					Value::U32(v) => v as u128,
					Value::U64(v) => v as u128,
					Value::U128(v) => v,
					_ => {
						return Err(ValueError::NonNumericBinary {
							op: NumBinaryOp::Sub,
							lhs: lhs_kind,
							rhs: rhs_kind,
						});
					}
				};
				Ok(Value::U128(a.wrapping_sub(b_val)))
			}

			// Signed - Unsigned → I64
			(a, b) if a.is_signed_int() && b.is_unsigned_int() => {
				let a_i64 = a.to_i64().unwrap();
				let b_i64 = match b {
					Value::U8(v) => v as i64,
					Value::U16(v) => v as i64,
					Value::U32(v) => v as i64,
					Value::U64(v) => v as i64,
					Value::U128(v) => v as i64,
					_ => {
						return Err(ValueError::NonNumericBinary {
							op: NumBinaryOp::Sub,
							lhs: lhs_kind,
							rhs: rhs_kind,
						});
					}
				};
				Ok(Value::I64(a_i64.wrapping_sub(b_i64)))
			}
			(a, b) if a.is_unsigned_int() && b.is_signed_int() => {
				let a_i64 = match a {
					Value::U8(v) => v as i64,
					Value::U16(v) => v as i64,
					Value::U32(v) => v as i64,
					Value::U64(v) => v as i64,
					Value::U128(v) => v as i64,
					_ => {
						return Err(ValueError::NonNumericBinary {
							op: NumBinaryOp::Sub,
							lhs: lhs_kind,
							rhs: rhs_kind,
						});
					}
				};
				let b_i64 = b.to_i64().unwrap();
				Ok(Value::I64(a_i64.wrapping_sub(b_i64)))
			}

			// Invalid combinations
			_ => Err(ValueError::NonNumericBinary {
				op: NumBinaryOp::Sub,
				lhs: lhs_kind,
				rhs: rhs_kind,
			}),
		}
	}

	fn num_binary_mul(lhs: Value, rhs: Value) -> Result<Value, ValueError> {
		let lhs_kind = lhs.kind();
		let rhs_kind = rhs.kind();
		match (lhs, rhs) {
			// Float * Float cases
			(Value::F64(a), Value::F64(b)) => Ok(Value::F64(a * b)),
			(Value::F32(a), Value::F32(b)) => Ok(Value::F32(a * b)),
			(Value::F64(a), Value::F32(b)) => Ok(Value::F64(a * (b as f64))),
			(Value::F32(a), Value::F64(b)) => Ok(Value::F64((a as f64) * b)),

			// Same-type signed integer multiplications
			(Value::I8(a), Value::I8(b)) => Ok(Value::I8(a.wrapping_mul(b))),
			(Value::I16(a), Value::I16(b)) => Ok(Value::I16(a.wrapping_mul(b))),
			(Value::I32(a), Value::I32(b)) => Ok(Value::I32(a.wrapping_mul(b))),
			(Value::I64(a), Value::I64(b)) => Ok(Value::I64(a.wrapping_mul(b))),

			// Same-type unsigned integer multiplications
			(Value::U8(a), Value::U8(b)) => Ok(Value::U8(a.wrapping_mul(b))),
			(Value::U16(a), Value::U16(b)) => Ok(Value::U16(a.wrapping_mul(b))),
			(Value::U32(a), Value::U32(b)) => Ok(Value::U32(a.wrapping_mul(b))),
			(Value::U64(a), Value::U64(b)) => Ok(Value::U64(a.wrapping_mul(b))),
			(Value::U128(a), Value::U128(b)) => Ok(Value::U128(a.wrapping_mul(b))),

			// Int * Float → F64
			(a, b) if (a.is_signed_int() || a.is_unsigned_int()) && b.is_float() => {
				let a_f64 = a.to_f64().unwrap();
				let b_f64 = b.to_f64().unwrap();
				Ok(Value::F64(a_f64 * b_f64))
			}
			(a, b) if a.is_float() && (b.is_signed_int() || b.is_unsigned_int()) => {
				let a_f64 = a.to_f64().unwrap();
				let b_f64 = b.to_f64().unwrap();
				Ok(Value::F64(a_f64 * b_f64))
			}

			// Cross-type signed integer multiplications → I64
			(a, b) if a.is_signed_int() && b.is_signed_int() => {
				let a_i64 = a.to_i64().unwrap();
				let b_i64 = b.to_i64().unwrap();
				Ok(Value::I64(a_i64.wrapping_mul(b_i64)))
			}

			// Cross-type unsigned integer multiplications → U128
			(Value::U8(a), b) if b.is_unsigned_int() => {
				let b_val = match b {
					Value::U8(v) => v as u128,
					Value::U16(v) => v as u128,
					Value::U32(v) => v as u128,
					Value::U64(v) => v as u128,
					Value::U128(v) => v,
					_ => {
						return Err(ValueError::NonNumericBinary {
							op: NumBinaryOp::Mul,
							lhs: lhs_kind,
							rhs: rhs_kind,
						});
					}
				};
				Ok(Value::U128((a as u128).wrapping_mul(b_val)))
			}
			(Value::U16(a), b) if b.is_unsigned_int() => {
				let b_val = match b {
					Value::U8(v) => v as u128,
					Value::U16(v) => v as u128,
					Value::U32(v) => v as u128,
					Value::U64(v) => v as u128,
					Value::U128(v) => v,
					_ => {
						return Err(ValueError::NonNumericBinary {
							op: NumBinaryOp::Mul,
							lhs: lhs_kind,
							rhs: rhs_kind,
						});
					}
				};
				Ok(Value::U128((a as u128).wrapping_mul(b_val)))
			}
			(Value::U32(a), b) if b.is_unsigned_int() => {
				let b_val = match b {
					Value::U8(v) => v as u128,
					Value::U16(v) => v as u128,
					Value::U32(v) => v as u128,
					Value::U64(v) => v as u128,
					Value::U128(v) => v,
					_ => {
						return Err(ValueError::NonNumericBinary {
							op: NumBinaryOp::Mul,
							lhs: lhs_kind,
							rhs: rhs_kind,
						});
					}
				};
				Ok(Value::U128((a as u128).wrapping_mul(b_val)))
			}
			(Value::U64(a), b) if b.is_unsigned_int() => {
				let b_val = match b {
					Value::U8(v) => v as u128,
					Value::U16(v) => v as u128,
					Value::U32(v) => v as u128,
					Value::U64(v) => v as u128,
					Value::U128(v) => v,
					_ => {
						return Err(ValueError::NonNumericBinary {
							op: NumBinaryOp::Mul,
							lhs: lhs_kind,
							rhs: rhs_kind,
						});
					}
				};
				Ok(Value::U128((a as u128).wrapping_mul(b_val)))
			}
			(Value::U128(a), b) if b.is_unsigned_int() => {
				let b_val = match b {
					Value::U8(v) => v as u128,
					Value::U16(v) => v as u128,
					Value::U32(v) => v as u128,
					Value::U64(v) => v as u128,
					Value::U128(v) => v,
					_ => {
						return Err(ValueError::NonNumericBinary {
							op: NumBinaryOp::Mul,
							lhs: lhs_kind,
							rhs: rhs_kind,
						});
					}
				};
				Ok(Value::U128(a.wrapping_mul(b_val)))
			}

			// Signed * Unsigned → I64
			(a, b) if a.is_signed_int() && b.is_unsigned_int() => {
				let a_i64 = a.to_i64().unwrap();
				let b_i64 = match b {
					Value::U8(v) => v as i64,
					Value::U16(v) => v as i64,
					Value::U32(v) => v as i64,
					Value::U64(v) => v as i64,
					Value::U128(v) => v as i64,
					_ => {
						return Err(ValueError::NonNumericBinary {
							op: NumBinaryOp::Mul,
							lhs: lhs_kind,
							rhs: rhs_kind,
						});
					}
				};
				Ok(Value::I64(a_i64.wrapping_mul(b_i64)))
			}
			(a, b) if a.is_unsigned_int() && b.is_signed_int() => {
				let a_i64 = match a {
					Value::U8(v) => v as i64,
					Value::U16(v) => v as i64,
					Value::U32(v) => v as i64,
					Value::U64(v) => v as i64,
					Value::U128(v) => v as i64,
					_ => {
						return Err(ValueError::NonNumericBinary {
							op: NumBinaryOp::Mul,
							lhs: lhs_kind,
							rhs: rhs_kind,
						});
					}
				};
				let b_i64 = b.to_i64().unwrap();
				Ok(Value::I64(a_i64.wrapping_mul(b_i64)))
			}

			// Invalid combinations
			_ => Err(ValueError::NonNumericBinary {
				op: NumBinaryOp::Mul,
				lhs: lhs_kind,
				rhs: rhs_kind,
			}),
		}
	}

	fn num_binary_div(lhs: Value, rhs: Value) -> Result<Value, ValueError> {
		let lhs_kind = lhs.kind();
		let rhs_kind = rhs.kind();

		let is_zero = |v: &Value| -> bool {
			match v {
				Value::I8(n) => *n == 0,
				Value::I16(n) => *n == 0,
				Value::I32(n) => *n == 0,
				Value::I64(n) => *n == 0,
				Value::U8(n) => *n == 0,
				Value::U16(n) => *n == 0,
				Value::U32(n) => *n == 0,
				Value::U64(n) => *n == 0,
				Value::U128(n) => *n == 0,
				Value::F32(n) => *n == 0.0,
				Value::F64(n) => *n == 0.0,
				_ => false,
			}
		};

		if rhs.is_numeric() && is_zero(&rhs) {
			return Err(ValueError::DivisionByZero);
		}

		match (lhs, rhs) {
			// Float / Float cases
			(Value::F64(a), Value::F64(b)) => Ok(Value::F64(a / b)),
			(Value::F32(a), Value::F32(b)) => Ok(Value::F32(a / b)),
			(Value::F64(a), Value::F32(b)) => Ok(Value::F64(a / (b as f64))),
			(Value::F32(a), Value::F64(b)) => Ok(Value::F64((a as f64) / b)),

			// Same-type signed integer divisions
			(Value::I8(a), Value::I8(b)) => Ok(Value::I8(a.wrapping_div(b))),
			(Value::I16(a), Value::I16(b)) => Ok(Value::I16(a.wrapping_div(b))),
			(Value::I32(a), Value::I32(b)) => Ok(Value::I32(a.wrapping_div(b))),
			(Value::I64(a), Value::I64(b)) => Ok(Value::I64(a.wrapping_div(b))),

			// Same-type unsigned integer divisions
			(Value::U8(a), Value::U8(b)) => Ok(Value::U8(a.wrapping_div(b))),
			(Value::U16(a), Value::U16(b)) => Ok(Value::U16(a.wrapping_div(b))),
			(Value::U32(a), Value::U32(b)) => Ok(Value::U32(a.wrapping_div(b))),
			(Value::U64(a), Value::U64(b)) => Ok(Value::U64(a.wrapping_div(b))),
			(Value::U128(a), Value::U128(b)) => Ok(Value::U128(a.wrapping_div(b))),

			// Int / Float → F64
			(a, b) if (a.is_signed_int() || a.is_unsigned_int()) && b.is_float() => {
				let a_f64 = a.to_f64().unwrap();
				let b_f64 = b.to_f64().unwrap();
				Ok(Value::F64(a_f64 / b_f64))
			}
			(a, b) if a.is_float() && (b.is_signed_int() || b.is_unsigned_int()) => {
				let a_f64 = a.to_f64().unwrap();
				let b_f64 = b.to_f64().unwrap();
				Ok(Value::F64(a_f64 / b_f64))
			}

			// Cross-type signed integer divisions → I64
			(a, b) if a.is_signed_int() && b.is_signed_int() => {
				let a_i64 = a.to_i64().unwrap();
				let b_i64 = b.to_i64().unwrap();
				Ok(Value::I64(a_i64.wrapping_div(b_i64)))
			}

			// Cross-type unsigned integer divisions → U128
			(Value::U8(a), b) if b.is_unsigned_int() => {
				let b_val = match b {
					Value::U8(v) => v as u128,
					Value::U16(v) => v as u128,
					Value::U32(v) => v as u128,
					Value::U64(v) => v as u128,
					Value::U128(v) => v,
					_ => {
						return Err(ValueError::NonNumericBinary {
							op: NumBinaryOp::Div,
							lhs: lhs_kind,
							rhs: rhs_kind,
						});
					}
				};
				Ok(Value::U128((a as u128).wrapping_div(b_val)))
			}
			(Value::U16(a), b) if b.is_unsigned_int() => {
				let b_val = match b {
					Value::U8(v) => v as u128,
					Value::U16(v) => v as u128,
					Value::U32(v) => v as u128,
					Value::U64(v) => v as u128,
					Value::U128(v) => v,
					_ => {
						return Err(ValueError::NonNumericBinary {
							op: NumBinaryOp::Div,
							lhs: lhs_kind,
							rhs: rhs_kind,
						});
					}
				};
				Ok(Value::U128((a as u128).wrapping_div(b_val)))
			}
			(Value::U32(a), b) if b.is_unsigned_int() => {
				let b_val = match b {
					Value::U8(v) => v as u128,
					Value::U16(v) => v as u128,
					Value::U32(v) => v as u128,
					Value::U64(v) => v as u128,
					Value::U128(v) => v,
					_ => {
						return Err(ValueError::NonNumericBinary {
							op: NumBinaryOp::Div,
							lhs: lhs_kind,
							rhs: rhs_kind,
						});
					}
				};
				Ok(Value::U128((a as u128).wrapping_div(b_val)))
			}
			(Value::U64(a), b) if b.is_unsigned_int() => {
				let b_val = match b {
					Value::U8(v) => v as u128,
					Value::U16(v) => v as u128,
					Value::U32(v) => v as u128,
					Value::U64(v) => v as u128,
					Value::U128(v) => v,
					_ => {
						return Err(ValueError::NonNumericBinary {
							op: NumBinaryOp::Div,
							lhs: lhs_kind,
							rhs: rhs_kind,
						});
					}
				};
				Ok(Value::U128((a as u128).wrapping_div(b_val)))
			}
			(Value::U128(a), b) if b.is_unsigned_int() => {
				let b_val = match b {
					Value::U8(v) => v as u128,
					Value::U16(v) => v as u128,
					Value::U32(v) => v as u128,
					Value::U64(v) => v as u128,
					Value::U128(v) => v,
					_ => {
						return Err(ValueError::NonNumericBinary {
							op: NumBinaryOp::Div,
							lhs: lhs_kind,
							rhs: rhs_kind,
						});
					}
				};
				Ok(Value::U128(a.wrapping_div(b_val)))
			}

			// Signed / Unsigned → I64
			(a, b) if a.is_signed_int() && b.is_unsigned_int() => {
				let a_i64 = a.to_i64().unwrap();
				let b_i64 = match b {
					Value::U8(v) => v as i64,
					Value::U16(v) => v as i64,
					Value::U32(v) => v as i64,
					Value::U64(v) => v as i64,
					Value::U128(v) => v as i64,
					_ => {
						return Err(ValueError::NonNumericBinary {
							op: NumBinaryOp::Div,
							lhs: lhs_kind,
							rhs: rhs_kind,
						});
					}
				};
				Ok(Value::I64(a_i64.wrapping_div(b_i64)))
			}
			(a, b) if a.is_unsigned_int() && b.is_signed_int() => {
				let a_i64 = match a {
					Value::U8(v) => v as i64,
					Value::U16(v) => v as i64,
					Value::U32(v) => v as i64,
					Value::U64(v) => v as i64,
					Value::U128(v) => v as i64,
					_ => {
						return Err(ValueError::NonNumericBinary {
							op: NumBinaryOp::Div,
							lhs: lhs_kind,
							rhs: rhs_kind,
						});
					}
				};
				let b_i64 = b.to_i64().unwrap();
				Ok(Value::I64(a_i64.wrapping_div(b_i64)))
			}

			// Invalid combinations
			_ => Err(ValueError::NonNumericBinary {
				op: NumBinaryOp::Div,
				lhs: lhs_kind,
				rhs: rhs_kind,
			}),
		}
	}

	fn num_binary_rem(lhs: Value, rhs: Value) -> Result<Value, ValueError> {
		let lhs_kind = lhs.kind();
		let rhs_kind = rhs.kind();

		let is_zero = |v: &Value| -> bool {
			match v {
				Value::I8(n) => *n == 0,
				Value::I16(n) => *n == 0,
				Value::I32(n) => *n == 0,
				Value::I64(n) => *n == 0,
				Value::U8(n) => *n == 0,
				Value::U16(n) => *n == 0,
				Value::U32(n) => *n == 0,
				Value::U64(n) => *n == 0,
				Value::U128(n) => *n == 0,
				Value::F32(n) => *n == 0.0,
				Value::F64(n) => *n == 0.0,
				_ => false,
			}
		};

		if rhs.is_numeric() && is_zero(&rhs) {
			return Err(ValueError::DivisionByZero);
		}

		match (lhs, rhs) {
			// Float % Float cases
			(Value::F64(a), Value::F64(b)) => Ok(Value::F64(a % b)),
			(Value::F32(a), Value::F32(b)) => Ok(Value::F32(a % b)),
			(Value::F64(a), Value::F32(b)) => Ok(Value::F64(a % (b as f64))),
			(Value::F32(a), Value::F64(b)) => Ok(Value::F64((a as f64) % b)),

			// Same-type signed integer modulo
			(Value::I8(a), Value::I8(b)) => Ok(Value::I8(a.wrapping_rem(b))),
			(Value::I16(a), Value::I16(b)) => Ok(Value::I16(a.wrapping_rem(b))),
			(Value::I32(a), Value::I32(b)) => Ok(Value::I32(a.wrapping_rem(b))),
			(Value::I64(a), Value::I64(b)) => Ok(Value::I64(a.wrapping_rem(b))),

			// Same-type unsigned integer modulo
			(Value::U8(a), Value::U8(b)) => Ok(Value::U8(a.wrapping_rem(b))),
			(Value::U16(a), Value::U16(b)) => Ok(Value::U16(a.wrapping_rem(b))),
			(Value::U32(a), Value::U32(b)) => Ok(Value::U32(a.wrapping_rem(b))),
			(Value::U64(a), Value::U64(b)) => Ok(Value::U64(a.wrapping_rem(b))),
			(Value::U128(a), Value::U128(b)) => Ok(Value::U128(a.wrapping_rem(b))),

			// Int % Float → F64
			(a, b) if (a.is_signed_int() || a.is_unsigned_int()) && b.is_float() => {
				let a_f64 = a.to_f64().unwrap();
				let b_f64 = b.to_f64().unwrap();
				Ok(Value::F64(a_f64 % b_f64))
			}
			(a, b) if a.is_float() && (b.is_signed_int() || b.is_unsigned_int()) => {
				let a_f64 = a.to_f64().unwrap();
				let b_f64 = b.to_f64().unwrap();
				Ok(Value::F64(a_f64 % b_f64))
			}

			// Cross-type signed integer modulo → I64
			(a, b) if a.is_signed_int() && b.is_signed_int() => {
				let a_i64 = a.to_i64().unwrap();
				let b_i64 = b.to_i64().unwrap();
				Ok(Value::I64(a_i64.wrapping_rem(b_i64)))
			}

			// Cross-type unsigned integer modulo → U128
			(Value::U8(a), b) if b.is_unsigned_int() => {
				let b_val = match b {
					Value::U8(v) => v as u128,
					Value::U16(v) => v as u128,
					Value::U32(v) => v as u128,
					Value::U64(v) => v as u128,
					Value::U128(v) => v,
					_ => {
						return Err(ValueError::NonNumericBinary {
							op: NumBinaryOp::Rem,
							lhs: lhs_kind,
							rhs: rhs_kind,
						});
					}
				};
				Ok(Value::U128((a as u128).wrapping_rem(b_val)))
			}
			(Value::U16(a), b) if b.is_unsigned_int() => {
				let b_val = match b {
					Value::U8(v) => v as u128,
					Value::U16(v) => v as u128,
					Value::U32(v) => v as u128,
					Value::U64(v) => v as u128,
					Value::U128(v) => v,
					_ => {
						return Err(ValueError::NonNumericBinary {
							op: NumBinaryOp::Rem,
							lhs: lhs_kind,
							rhs: rhs_kind,
						});
					}
				};
				Ok(Value::U128((a as u128).wrapping_rem(b_val)))
			}
			(Value::U32(a), b) if b.is_unsigned_int() => {
				let b_val = match b {
					Value::U8(v) => v as u128,
					Value::U16(v) => v as u128,
					Value::U32(v) => v as u128,
					Value::U64(v) => v as u128,
					Value::U128(v) => v,
					_ => {
						return Err(ValueError::NonNumericBinary {
							op: NumBinaryOp::Rem,
							lhs: lhs_kind,
							rhs: rhs_kind,
						});
					}
				};
				Ok(Value::U128((a as u128).wrapping_rem(b_val)))
			}
			(Value::U64(a), b) if b.is_unsigned_int() => {
				let b_val = match b {
					Value::U8(v) => v as u128,
					Value::U16(v) => v as u128,
					Value::U32(v) => v as u128,
					Value::U64(v) => v as u128,
					Value::U128(v) => v,
					_ => {
						return Err(ValueError::NonNumericBinary {
							op: NumBinaryOp::Rem,
							lhs: lhs_kind,
							rhs: rhs_kind,
						});
					}
				};
				Ok(Value::U128((a as u128).wrapping_rem(b_val)))
			}
			(Value::U128(a), b) if b.is_unsigned_int() => {
				let b_val = match b {
					Value::U8(v) => v as u128,
					Value::U16(v) => v as u128,
					Value::U32(v) => v as u128,
					Value::U64(v) => v as u128,
					Value::U128(v) => v,
					_ => {
						return Err(ValueError::NonNumericBinary {
							op: NumBinaryOp::Rem,
							lhs: lhs_kind,
							rhs: rhs_kind,
						});
					}
				};
				Ok(Value::U128(a.wrapping_rem(b_val)))
			}

			// Signed % Unsigned → I64
			(a, b) if a.is_signed_int() && b.is_unsigned_int() => {
				let a_i64 = a.to_i64().unwrap();
				let b_i64 = match b {
					Value::U8(v) => v as i64,
					Value::U16(v) => v as i64,
					Value::U32(v) => v as i64,
					Value::U64(v) => v as i64,
					Value::U128(v) => v as i64,
					_ => {
						return Err(ValueError::NonNumericBinary {
							op: NumBinaryOp::Rem,
							lhs: lhs_kind,
							rhs: rhs_kind,
						});
					}
				};
				Ok(Value::I64(a_i64.wrapping_rem(b_i64)))
			}
			(a, b) if a.is_unsigned_int() && b.is_signed_int() => {
				let a_i64 = match a {
					Value::U8(v) => v as i64,
					Value::U16(v) => v as i64,
					Value::U32(v) => v as i64,
					Value::U64(v) => v as i64,
					Value::U128(v) => v as i64,
					_ => {
						return Err(ValueError::NonNumericBinary {
							op: NumBinaryOp::Rem,
							lhs: lhs_kind,
							rhs: rhs_kind,
						});
					}
				};
				let b_i64 = b.to_i64().unwrap();
				Ok(Value::I64(a_i64.wrapping_rem(b_i64)))
			}

			// Invalid combinations
			_ => Err(ValueError::NonNumericBinary {
				op: NumBinaryOp::Rem,
				lhs: lhs_kind,
				rhs: rhs_kind,
			}),
		}
	}

	fn num_binary_pow(lhs: &Value, rhs: &Value) -> Result<Value, ValueError> {
		if !lhs.is_numeric() || !rhs.is_numeric() {
			return Err(ValueError::NonNumericBinary {
				op: NumBinaryOp::Pow,
				lhs: lhs.kind(),
				rhs: rhs.kind(),
			});
		}
		let base = lhs.to_f64().ok_or(ValueError::NonNumericBinary {
			op: NumBinaryOp::Pow,
			lhs: lhs.kind(),
			rhs: rhs.kind(),
		})?;
		let exp = rhs.to_f64().ok_or(ValueError::NonNumericBinary {
			op: NumBinaryOp::Pow,
			lhs: lhs.kind(),
			rhs: rhs.kind(),
		})?;
		Ok(Value::F64(base.powf(exp)))
	}

	fn num_binary_min(lhs: &Value, rhs: &Value) -> Result<Value, ValueError> {
		if !lhs.is_numeric() || !rhs.is_numeric() {
			return Err(ValueError::NonNumericBinary {
				op: NumBinaryOp::Min,
				lhs: lhs.kind(),
				rhs: rhs.kind(),
			});
		}

		match (lhs, rhs) {
			(Value::I8(a), Value::I8(b)) => Ok(Value::I8(*a.min(b))),
			(Value::I16(a), Value::I16(b)) => Ok(Value::I16(*a.min(b))),
			(Value::I32(a), Value::I32(b)) => Ok(Value::I32(*a.min(b))),
			(Value::I64(a), Value::I64(b)) => Ok(Value::I64(*a.min(b))),
			(Value::U8(a), Value::U8(b)) => Ok(Value::U8(*a.min(b))),
			(Value::U16(a), Value::U16(b)) => Ok(Value::U16(*a.min(b))),
			(Value::U32(a), Value::U32(b)) => Ok(Value::U32(*a.min(b))),
			(Value::U64(a), Value::U64(b)) => Ok(Value::U64(*a.min(b))),
			(Value::U128(a), Value::U128(b)) => Ok(Value::U128(*a.min(b))),
			(Value::F32(a), Value::F32(b)) => Ok(Value::F32(a.min(*b))),
			(Value::F64(a), Value::F64(b)) => Ok(Value::F64(a.min(*b))),
			_ => {
				let a_f64 = lhs.to_f64().ok_or(ValueError::NonNumericBinary {
					op: NumBinaryOp::Min,
					lhs: lhs.kind(),
					rhs: rhs.kind(),
				})?;
				let b_f64 = rhs.to_f64().ok_or(ValueError::NonNumericBinary {
					op: NumBinaryOp::Min,
					lhs: lhs.kind(),
					rhs: rhs.kind(),
				})?;
				Ok(Value::F64(a_f64.min(b_f64)))
			}
		}
	}

	fn num_binary_max(lhs: &Value, rhs: &Value) -> Result<Value, ValueError> {
		if !lhs.is_numeric() || !rhs.is_numeric() {
			return Err(ValueError::NonNumericBinary {
				op: NumBinaryOp::Max,
				lhs: lhs.kind(),
				rhs: rhs.kind(),
			});
		}

		match (lhs, rhs) {
			(Value::I8(a), Value::I8(b)) => Ok(Value::I8(*a.max(b))),
			(Value::I16(a), Value::I16(b)) => Ok(Value::I16(*a.max(b))),
			(Value::I32(a), Value::I32(b)) => Ok(Value::I32(*a.max(b))),
			(Value::I64(a), Value::I64(b)) => Ok(Value::I64(*a.max(b))),
			(Value::U8(a), Value::U8(b)) => Ok(Value::U8(*a.max(b))),
			(Value::U16(a), Value::U16(b)) => Ok(Value::U16(*a.max(b))),
			(Value::U32(a), Value::U32(b)) => Ok(Value::U32(*a.max(b))),
			(Value::U64(a), Value::U64(b)) => Ok(Value::U64(*a.max(b))),
			(Value::U128(a), Value::U128(b)) => Ok(Value::U128(*a.max(b))),
			(Value::F32(a), Value::F32(b)) => Ok(Value::F32(a.max(*b))),
			(Value::F64(a), Value::F64(b)) => Ok(Value::F64(a.max(*b))),
			_ => {
				let a_f64 = lhs.to_f64().ok_or(ValueError::NonNumericBinary {
					op: NumBinaryOp::Max,
					lhs: lhs.kind(),
					rhs: rhs.kind(),
				})?;
				let b_f64 = rhs.to_f64().ok_or(ValueError::NonNumericBinary {
					op: NumBinaryOp::Max,
					lhs: lhs.kind(),
					rhs: rhs.kind(),
				})?;
				Ok(Value::F64(a_f64.max(b_f64)))
			}
		}
	}

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

	pub fn try_num_binary(&self, op: NumBinaryOp, rhs: &Value) -> Result<Value, ValueError> {
		match op {
			NumBinaryOp::Add => Self::num_binary_add(self.clone(), rhs.clone()),
			NumBinaryOp::Sub => Self::num_binary_sub(self.clone(), rhs.clone()),
			NumBinaryOp::Mul => Self::num_binary_mul(self.clone(), rhs.clone()),
			NumBinaryOp::Div => Self::num_binary_div(self.clone(), rhs.clone()),
			NumBinaryOp::Rem => Self::num_binary_rem(self.clone(), rhs.clone()),
			NumBinaryOp::Pow => Self::num_binary_pow(self, rhs),
			NumBinaryOp::Min => Self::num_binary_min(self, rhs),
			NumBinaryOp::Max => Self::num_binary_max(self, rhs),
		}
	}

	pub fn try_add(&self, rhs: &Value) -> Result<Value, ValueError> {
		self.try_num_binary(NumBinaryOp::Add, rhs)
	}

	pub fn try_sub(&self, rhs: &Value) -> Result<Value, ValueError> {
		self.try_num_binary(NumBinaryOp::Sub, rhs)
	}

	pub fn try_mul(&self, rhs: &Value) -> Result<Value, ValueError> {
		self.try_num_binary(NumBinaryOp::Mul, rhs)
	}

	pub fn try_div(&self, rhs: &Value) -> Result<Value, ValueError> {
		self.try_num_binary(NumBinaryOp::Div, rhs)
	}

	pub fn try_rem(&self, rhs: &Value) -> Result<Value, ValueError> {
		self.try_num_binary(NumBinaryOp::Rem, rhs)
	}

	pub fn try_pow(&self, rhs: &Value) -> Result<Value, ValueError> {
		self.try_num_binary(NumBinaryOp::Pow, rhs)
	}

	pub fn try_abs(&self) -> Result<Value, ValueError> {
		self.try_num_unary(NumUnaryOp::Abs)
	}

	pub fn try_sqrt(&self) -> Result<Value, ValueError> {
		self.try_num_unary(NumUnaryOp::Sqrt)
	}

	pub fn try_min(&self, rhs: &Value) -> Result<Value, ValueError> {
		self.try_num_binary(NumBinaryOp::Min, rhs)
	}

	pub fn try_max(&self, rhs: &Value) -> Result<Value, ValueError> {
		self.try_num_binary(NumBinaryOp::Max, rhs)
	}

	/// Compute power: self^other, returns F64
	pub fn pow(&self, other: &Value) -> Value {
		self.try_pow(other)
			.unwrap_or_else(|err| panic!("Value::pow failed: {err}"))
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

	/// Return the minimum of self and other
	pub fn min(&self, other: &Value) -> Value {
		self.try_min(other)
			.unwrap_or_else(|err| panic!("Value::min failed: {err}"))
	}

	/// Return the maximum of self and other
	pub fn max(&self, other: &Value) -> Value {
		self.try_max(other)
			.unwrap_or_else(|err| panic!("Value::max failed: {err}"))
	}
}

impl std::ops::Add for &Value {
	type Output = Value;
	fn add(self, other: Self) -> Self::Output {
		self.try_num_binary(NumBinaryOp::Add, other)
			.unwrap_or_else(|err| panic!("Value::add failed: {err}"))
	}
}
impl std::ops::Mul for &Value {
	type Output = Value;
	fn mul(self, other: Self) -> Self::Output {
		self.try_num_binary(NumBinaryOp::Mul, other)
			.unwrap_or_else(|err| panic!("Value::mul failed: {err}"))
	}
}
impl std::ops::Div for &Value {
	type Output = Value;
	fn div(self, other: Self) -> Self::Output {
		self.try_num_binary(NumBinaryOp::Div, other)
			.unwrap_or_else(|err| panic!("Value::div failed: {err}"))
	}
}
impl std::ops::Sub for &Value {
	type Output = Value;
	fn sub(self, other: Self) -> Self::Output {
		self.try_num_binary(NumBinaryOp::Sub, other)
			.unwrap_or_else(|err| panic!("Value::sub failed: {err}"))
	}
}

impl std::ops::Add for Value {
	type Output = Value;
	fn add(self, other: Self) -> Self::Output {
		Self::num_binary_add(self, other).unwrap_or_else(|err| panic!("Value::add failed: {err}"))
	}
}

impl std::ops::Sub for Value {
	type Output = Value;

	fn sub(self, other: Self) -> Self::Output {
		Self::num_binary_sub(self, other).unwrap_or_else(|err| panic!("Value::sub failed: {err}"))
	}
}

impl std::ops::Mul for Value {
	type Output = Value;

	fn mul(self, other: Self) -> Self::Output {
		Self::num_binary_mul(self, other).unwrap_or_else(|err| panic!("Value::mul failed: {err}"))
	}
}

impl std::ops::Div for Value {
	type Output = Value;

	fn div(self, other: Self) -> Self::Output {
		Self::num_binary_div(self, other).unwrap_or_else(|err| panic!("Value::div failed: {err}"))
	}
}

impl std::ops::Rem for &Value {
	type Output = Value;
	fn rem(self, other: Self) -> Self::Output {
		self.try_num_binary(NumBinaryOp::Rem, other)
			.unwrap_or_else(|err| panic!("Value::rem failed: {err}"))
	}
}

impl std::ops::Rem for Value {
	type Output = Value;

	fn rem(self, other: Self) -> Self::Output {
		Self::num_binary_rem(self, other).unwrap_or_else(|err| panic!("Value::rem failed: {err}"))
	}
}
