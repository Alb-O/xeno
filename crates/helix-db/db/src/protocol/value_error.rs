use thiserror::Error;

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum ValueKind {
	String,
	F32,
	F64,
	I8,
	I16,
	I32,
	I64,
	U8,
	U16,
	U32,
	U64,
	U128,
	Date,
	Bool,
	Id,
	Array,
	Object,
	Empty,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum NumUnaryOp {
	Abs,
	Sqrt,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum NumBinaryOp {
	Add,
	Sub,
	Mul,
	Div,
	Rem,
	Pow,
	Min,
	Max,
}

#[derive(Debug, Error)]
pub enum ValueError {
	#[error("expected string, got {got:?}")]
	NotString { got: ValueKind },

	#[error("expected primitive, got {got:?}")]
	NotPrimitive { got: ValueKind },

	#[error("numeric op {op:?} requires numeric operand, got {got:?}")]
	NonNumericUnary { op: NumUnaryOp, got: ValueKind },

	#[error("numeric op {op:?} type mismatch: {lhs:?} vs {rhs:?}")]
	NonNumericBinary {
		op: NumBinaryOp,
		lhs: ValueKind,
		rhs: ValueKind,
	},

	#[error("division by zero")]
	DivisionByZero,

	#[error("domain error for {op}")]
	Domain { op: &'static str },

	#[error("numeric overflow")]
	Overflow,
}

#[cfg(test)]
mod tests {
	use super::ValueKind;
	use crate::protocol::date::Date;
	use crate::protocol::value::Value;
	use crate::utils::id::ID;

	#[test]
	fn test_value_kind_is_total() {
		let value_cases = vec![
			(Value::String("s".to_string()), ValueKind::String),
			(Value::F32(1.0), ValueKind::F32),
			(Value::F64(1.0), ValueKind::F64),
			(Value::I8(1), ValueKind::I8),
			(Value::I16(1), ValueKind::I16),
			(Value::I32(1), ValueKind::I32),
			(Value::I64(1), ValueKind::I64),
			(Value::U8(1), ValueKind::U8),
			(Value::U16(1), ValueKind::U16),
			(Value::U32(1), ValueKind::U32),
			(Value::U64(1), ValueKind::U64),
			(Value::U128(1), ValueKind::U128),
			(
				Value::Date(Date::new(&Value::I64(0)).expect("valid epoch date")),
				ValueKind::Date,
			),
			(Value::Boolean(true), ValueKind::Bool),
			(
				Value::Id(ID::from("00000000-0000-0000-0000-000000000000")),
				ValueKind::Id,
			),
			(Value::Array(vec![]), ValueKind::Array),
			(Value::Object(Default::default()), ValueKind::Object),
			(Value::Empty, ValueKind::Empty),
		];

		for (value, expected) in value_cases {
			assert_eq!(value.kind(), expected);
		}
	}

	#[test]
	fn test_stringify_primitive_contract() {
		let val = Value::String("hello".to_string());
		let cow = val.try_stringify_primitive().unwrap();
		assert!(matches!(cow, std::borrow::Cow::Borrowed(_)));

		let val = Value::I32(42);
		let cow = val.try_stringify_primitive().unwrap();
		assert!(matches!(cow, std::borrow::Cow::Owned(_)));

		let val = Value::Boolean(true);
		let cow = val.try_stringify_primitive().unwrap();
		assert!(matches!(cow, std::borrow::Cow::Owned(_)));

		let date = Date::new(&Value::I64(0)).expect("valid epoch date");
		let val = Value::Date(date);
		let cow = val.try_stringify_primitive().unwrap();
		assert!(matches!(cow, std::borrow::Cow::Owned(_)));

		let id = ID::from("00000000-0000-0000-0000-000000000000");
		let val = Value::Id(id);
		let cow = val.try_stringify_primitive().unwrap();
		assert!(matches!(cow, std::borrow::Cow::Owned(_)));

		assert!(Value::Array(vec![]).try_stringify_primitive().is_err());
		assert!(
			Value::Object(Default::default())
				.try_stringify_primitive()
				.is_err()
		);
		assert!(Value::Empty.try_stringify_primitive().is_err());
	}
}
