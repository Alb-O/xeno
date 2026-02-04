use crate::protocol::value::Value;
use crate::protocol::value_error::NumBinaryOp;

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
