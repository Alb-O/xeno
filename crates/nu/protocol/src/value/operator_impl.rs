impl Value {
	pub fn add(&self, op: Span, rhs: &Value, span: Span) -> Result<Value, ShellError> {
		match (self, rhs) {
			(Value::Int { val: lhs, .. }, Value::Int { val: rhs, .. }) => {
				if let Some(val) = lhs.checked_add(*rhs) {
					Ok(Value::int(val, span))
				} else {
					Err(ShellError::OperatorOverflow {
                        msg: "add operation overflowed".into(),
                        span,
                        help: Some("Consider using floating point values for increased range by promoting operand with 'into float'. Note: float has reduced precision!".into()),
                     })
				}
			}
			(Value::Int { val: lhs, .. }, Value::Float { val: rhs, .. }) => Ok(Value::float(*lhs as f64 + *rhs, span)),
			(Value::Float { val: lhs, .. }, Value::Int { val: rhs, .. }) => Ok(Value::float(*lhs + *rhs as f64, span)),
			(Value::Float { val: lhs, .. }, Value::Float { val: rhs, .. }) => Ok(Value::float(lhs + rhs, span)),
			(Value::String { val: lhs, .. }, Value::String { val: rhs, .. }) => Ok(Value::string(lhs.to_string() + rhs, span)),
			(Value::Duration { val: lhs, .. }, Value::Date { val: rhs, .. }) => {
				if let Some(val) = rhs.checked_add_signed(chrono::Duration::nanoseconds(*lhs)) {
					Ok(Value::date(val, span))
				} else {
					Err(ShellError::OperatorOverflow {
						msg: "addition operation overflowed".into(),
						span,
						help: None,
					})
				}
			}
			(Value::Date { val: lhs, .. }, Value::Duration { val: rhs, .. }) => {
				if let Some(val) = lhs.checked_add_signed(chrono::Duration::nanoseconds(*rhs)) {
					Ok(Value::date(val, span))
				} else {
					Err(ShellError::OperatorOverflow {
						msg: "addition operation overflowed".into(),
						span,
						help: None,
					})
				}
			}
			(Value::Duration { val: lhs, .. }, Value::Duration { val: rhs, .. }) => checked_duration_operation(*lhs, *rhs, i64::checked_add, span),
			(Value::Filesize { val: lhs, .. }, Value::Filesize { val: rhs, .. }) => {
				if let Some(val) = *lhs + *rhs {
					Ok(Value::filesize(val, span))
				} else {
					Err(ShellError::OperatorOverflow {
						msg: "add operation overflowed".into(),
						span,
						help: None,
					})
				}
			}
			(Value::Custom { val: lhs, .. }, rhs) => lhs.operation(self.span(), Operator::Math(Math::Add), op, rhs),
			_ => Err(operator_type_error(Operator::Math(Math::Add), op, self, rhs, |val| {
				matches!(
					val,
					Value::Int { .. } | Value::Float { .. } | Value::String { .. } | Value::Date { .. } | Value::Duration { .. } | Value::Filesize { .. },
				)
			})),
		}
	}

	pub fn sub(&self, op: Span, rhs: &Value, span: Span) -> Result<Value, ShellError> {
		match (self, rhs) {
			(Value::Int { val: lhs, .. }, Value::Int { val: rhs, .. }) => {
				if let Some(val) = lhs.checked_sub(*rhs) {
					Ok(Value::int(val, span))
				} else {
					Err(ShellError::OperatorOverflow {
                        msg: "subtraction operation overflowed".into(),
                        span,
                        help: Some("Consider using floating point values for increased range by promoting operand with 'into float'. Note: float has reduced precision!".into()),
                    })
				}
			}
			(Value::Int { val: lhs, .. }, Value::Float { val: rhs, .. }) => Ok(Value::float(*lhs as f64 - *rhs, span)),
			(Value::Float { val: lhs, .. }, Value::Int { val: rhs, .. }) => Ok(Value::float(*lhs - *rhs as f64, span)),
			(Value::Float { val: lhs, .. }, Value::Float { val: rhs, .. }) => Ok(Value::float(lhs - rhs, span)),
			(Value::Date { val: lhs, .. }, Value::Date { val: rhs, .. }) => {
				let result = lhs.signed_duration_since(*rhs);
				if let Some(v) = result.num_nanoseconds() {
					Ok(Value::duration(v, span))
				} else {
					Err(ShellError::OperatorOverflow {
						msg: "subtraction operation overflowed".into(),
						span,
						help: None,
					})
				}
			}
			(Value::Date { val: lhs, .. }, Value::Duration { val: rhs, .. }) => match lhs.checked_sub_signed(chrono::Duration::nanoseconds(*rhs)) {
				Some(val) => Ok(Value::date(val, span)),
				_ => Err(ShellError::OperatorOverflow {
					msg: "subtraction operation overflowed".into(),
					span,
					help: None,
				}),
			},
			(Value::Duration { val: lhs, .. }, Value::Duration { val: rhs, .. }) => checked_duration_operation(*lhs, *rhs, i64::checked_sub, span),
			(Value::Filesize { val: lhs, .. }, Value::Filesize { val: rhs, .. }) => {
				if let Some(val) = *lhs - *rhs {
					Ok(Value::filesize(val, span))
				} else {
					Err(ShellError::OperatorOverflow {
						msg: "add operation overflowed".into(),
						span,
						help: None,
					})
				}
			}
			(Value::Custom { val: lhs, .. }, rhs) => lhs.operation(self.span(), Operator::Math(Math::Subtract), op, rhs),
			_ => Err(operator_type_error(Operator::Math(Math::Subtract), op, self, rhs, |val| {
				matches!(
					val,
					Value::Int { .. } | Value::Float { .. } | Value::Date { .. } | Value::Duration { .. } | Value::Filesize { .. },
				)
			})),
		}
	}

	pub fn mul(&self, op: Span, rhs: &Value, span: Span) -> Result<Value, ShellError> {
		match (self, rhs) {
			(Value::Int { val: lhs, .. }, Value::Int { val: rhs, .. }) => {
				if let Some(val) = lhs.checked_mul(*rhs) {
					Ok(Value::int(val, span))
				} else {
					Err(ShellError::OperatorOverflow {
                        msg: "multiply operation overflowed".into(),
                        span,
                        help: Some("Consider using floating point values for increased range by promoting operand with 'into float'. Note: float has reduced precision!".into()),
                    })
				}
			}
			(Value::Int { val: lhs, .. }, Value::Float { val: rhs, .. }) => Ok(Value::float(*lhs as f64 * *rhs, span)),
			(Value::Float { val: lhs, .. }, Value::Int { val: rhs, .. }) => Ok(Value::float(*lhs * *rhs as f64, span)),
			(Value::Float { val: lhs, .. }, Value::Float { val: rhs, .. }) => Ok(Value::float(lhs * rhs, span)),
			(Value::Int { val: lhs, .. }, Value::Filesize { val: rhs, .. }) => {
				if let Some(val) = *lhs * *rhs {
					Ok(Value::filesize(val, span))
				} else {
					Err(ShellError::OperatorOverflow {
						msg: "multiply operation overflowed".into(),
						span,
						help: None,
					})
				}
			}
			(Value::Filesize { val: lhs, .. }, Value::Int { val: rhs, .. }) => {
				if let Some(val) = *lhs * *rhs {
					Ok(Value::filesize(val, span))
				} else {
					Err(ShellError::OperatorOverflow {
						msg: "multiply operation overflowed".into(),
						span,
						help: None,
					})
				}
			}
			(Value::Float { val: lhs, .. }, Value::Filesize { val: rhs, .. }) => {
				if let Some(val) = *lhs * *rhs {
					Ok(Value::filesize(val, span))
				} else {
					Err(ShellError::OperatorOverflow {
						msg: "multiply operation overflowed".into(),
						span,
						help: None,
					})
				}
			}
			(Value::Filesize { val: lhs, .. }, Value::Float { val: rhs, .. }) => {
				if let Some(val) = *lhs * *rhs {
					Ok(Value::filesize(val, span))
				} else {
					Err(ShellError::OperatorOverflow {
						msg: "multiply operation overflowed".into(),
						span,
						help: None,
					})
				}
			}
			(Value::Int { val: lhs, .. }, Value::Duration { val: rhs, .. }) => checked_duration_operation(*lhs, *rhs, i64::checked_mul, span),
			(Value::Duration { val: lhs, .. }, Value::Int { val: rhs, .. }) => checked_duration_operation(*lhs, *rhs, i64::checked_mul, span),
			(Value::Duration { val: lhs, .. }, Value::Float { val: rhs, .. }) => Ok(Value::duration((*lhs as f64 * *rhs) as i64, span)),
			(Value::Float { val: lhs, .. }, Value::Duration { val: rhs, .. }) => Ok(Value::duration((*lhs * *rhs as f64) as i64, span)),
			(Value::Custom { val: lhs, .. }, rhs) => lhs.operation(self.span(), Operator::Math(Math::Multiply), op, rhs),
			_ => Err(operator_type_error(Operator::Math(Math::Multiply), op, self, rhs, |val| {
				matches!(val, Value::Int { .. } | Value::Float { .. } | Value::Duration { .. } | Value::Filesize { .. },)
			})),
		}
	}

	pub fn div(&self, op: Span, rhs: &Value, span: Span) -> Result<Value, ShellError> {
		match (self, rhs) {
			(Value::Int { val: lhs, .. }, Value::Int { val: rhs, .. }) => {
				if *rhs == 0 {
					Err(ShellError::DivisionByZero { span: op })
				} else {
					Ok(Value::float(*lhs as f64 / *rhs as f64, span))
				}
			}
			(Value::Int { val: lhs, .. }, Value::Float { val: rhs, .. }) => {
				if *rhs != 0.0 {
					Ok(Value::float(*lhs as f64 / *rhs, span))
				} else {
					Err(ShellError::DivisionByZero { span: op })
				}
			}
			(Value::Float { val: lhs, .. }, Value::Int { val: rhs, .. }) => {
				if *rhs != 0 {
					Ok(Value::float(*lhs / *rhs as f64, span))
				} else {
					Err(ShellError::DivisionByZero { span: op })
				}
			}
			(Value::Float { val: lhs, .. }, Value::Float { val: rhs, .. }) => {
				if *rhs != 0.0 {
					Ok(Value::float(lhs / rhs, span))
				} else {
					Err(ShellError::DivisionByZero { span: op })
				}
			}
			(Value::Filesize { val: lhs, .. }, Value::Filesize { val: rhs, .. }) => {
				if *rhs == Filesize::ZERO {
					Err(ShellError::DivisionByZero { span: op })
				} else {
					Ok(Value::float(lhs.get() as f64 / rhs.get() as f64, span))
				}
			}
			(Value::Filesize { val: lhs, .. }, Value::Int { val: rhs, .. }) => {
				if let Some(val) = lhs.get().checked_div(*rhs) {
					Ok(Value::filesize(val, span))
				} else if *rhs == 0 {
					Err(ShellError::DivisionByZero { span: op })
				} else {
					Err(ShellError::OperatorOverflow {
						msg: "division operation overflowed".into(),
						span,
						help: None,
					})
				}
			}
			(Value::Filesize { val: lhs, .. }, Value::Float { val: rhs, .. }) => {
				if *rhs != 0.0 {
					if let Ok(val) = Filesize::try_from(lhs.get() as f64 / rhs) {
						Ok(Value::filesize(val, span))
					} else {
						Err(ShellError::OperatorOverflow {
							msg: "division operation overflowed".into(),
							span,
							help: None,
						})
					}
				} else {
					Err(ShellError::DivisionByZero { span: op })
				}
			}
			(Value::Duration { val: lhs, .. }, Value::Duration { val: rhs, .. }) => {
				if *rhs == 0 {
					Err(ShellError::DivisionByZero { span: op })
				} else {
					Ok(Value::float(*lhs as f64 / *rhs as f64, span))
				}
			}
			(Value::Duration { val: lhs, .. }, Value::Int { val: rhs, .. }) => {
				if let Some(val) = lhs.checked_div(*rhs) {
					Ok(Value::duration(val, span))
				} else if *rhs == 0 {
					Err(ShellError::DivisionByZero { span: op })
				} else {
					Err(ShellError::OperatorOverflow {
						msg: "division operation overflowed".into(),
						span,
						help: None,
					})
				}
			}
			(Value::Duration { val: lhs, .. }, Value::Float { val: rhs, .. }) => {
				if *rhs != 0.0 {
					let val = *lhs as f64 / rhs;
					if i64::MIN as f64 <= val && val <= i64::MAX as f64 {
						Ok(Value::duration(val as i64, span))
					} else {
						Err(ShellError::OperatorOverflow {
							msg: "division operation overflowed".into(),
							span,
							help: None,
						})
					}
				} else {
					Err(ShellError::DivisionByZero { span: op })
				}
			}
			(Value::Custom { val: lhs, .. }, rhs) => lhs.operation(self.span(), Operator::Math(Math::Divide), op, rhs),
			_ => Err(operator_type_error(Operator::Math(Math::Divide), op, self, rhs, |val| {
				matches!(val, Value::Int { .. } | Value::Float { .. } | Value::Duration { .. } | Value::Filesize { .. },)
			})),
		}
	}

	pub fn floor_div(&self, op: Span, rhs: &Value, span: Span) -> Result<Value, ShellError> {
		// Taken from the unstable `div_floor` function in the std library.
		fn checked_div_floor_i64(dividend: i64, divisor: i64) -> Option<i64> {
			let quotient = dividend.checked_div(divisor)?;
			let remainder = dividend.checked_rem(divisor)?;
			if (remainder > 0 && divisor < 0) || (remainder < 0 && divisor > 0) {
				// Note that `quotient - 1` cannot overflow, because:
				//     `quotient` would have to be `i64::MIN`
				//     => `divisor` would have to be `1`
				//     => `remainder` would have to be `0`
				// But `remainder == 0` is excluded from the check above.
				Some(quotient - 1)
			} else {
				Some(quotient)
			}
		}

		fn checked_div_floor_f64(dividend: f64, divisor: f64) -> Option<f64> {
			if divisor == 0.0 { None } else { Some((dividend / divisor).floor()) }
		}

		match (self, rhs) {
			(Value::Int { val: lhs, .. }, Value::Int { val: rhs, .. }) => {
				if let Some(val) = checked_div_floor_i64(*lhs, *rhs) {
					Ok(Value::int(val, span))
				} else if *rhs == 0 {
					Err(ShellError::DivisionByZero { span: op })
				} else {
					Err(ShellError::OperatorOverflow {
						msg: "division operation overflowed".into(),
						span,
						help: None,
					})
				}
			}
			(Value::Int { val: lhs, .. }, Value::Float { val: rhs, .. }) => {
				if let Some(val) = checked_div_floor_f64(*lhs as f64, *rhs) {
					Ok(Value::float(val, span))
				} else {
					Err(ShellError::DivisionByZero { span: op })
				}
			}
			(Value::Float { val: lhs, .. }, Value::Int { val: rhs, .. }) => {
				if let Some(val) = checked_div_floor_f64(*lhs, *rhs as f64) {
					Ok(Value::float(val, span))
				} else {
					Err(ShellError::DivisionByZero { span: op })
				}
			}
			(Value::Float { val: lhs, .. }, Value::Float { val: rhs, .. }) => {
				if let Some(val) = checked_div_floor_f64(*lhs, *rhs) {
					Ok(Value::float(val, span))
				} else {
					Err(ShellError::DivisionByZero { span: op })
				}
			}
			(Value::Filesize { val: lhs, .. }, Value::Filesize { val: rhs, .. }) => {
				if let Some(val) = checked_div_floor_i64(lhs.get(), rhs.get()) {
					Ok(Value::int(val, span))
				} else if *rhs == Filesize::ZERO {
					Err(ShellError::DivisionByZero { span: op })
				} else {
					Err(ShellError::OperatorOverflow {
						msg: "division operation overflowed".into(),
						span,
						help: None,
					})
				}
			}
			(Value::Filesize { val: lhs, .. }, Value::Int { val: rhs, .. }) => {
				if let Some(val) = checked_div_floor_i64(lhs.get(), *rhs) {
					Ok(Value::filesize(val, span))
				} else if *rhs == 0 {
					Err(ShellError::DivisionByZero { span: op })
				} else {
					Err(ShellError::OperatorOverflow {
						msg: "division operation overflowed".into(),
						span,
						help: None,
					})
				}
			}
			(Value::Filesize { val: lhs, .. }, Value::Float { val: rhs, .. }) => {
				if let Some(val) = checked_div_floor_f64(lhs.get() as f64, *rhs) {
					if let Ok(val) = Filesize::try_from(val) {
						Ok(Value::filesize(val, span))
					} else {
						Err(ShellError::OperatorOverflow {
							msg: "division operation overflowed".into(),
							span,
							help: None,
						})
					}
				} else {
					Err(ShellError::DivisionByZero { span: op })
				}
			}
			(Value::Duration { val: lhs, .. }, Value::Duration { val: rhs, .. }) => {
				if let Some(val) = checked_div_floor_i64(*lhs, *rhs) {
					Ok(Value::int(val, span))
				} else if *rhs == 0 {
					Err(ShellError::DivisionByZero { span: op })
				} else {
					Err(ShellError::OperatorOverflow {
						msg: "division operation overflowed".into(),
						span,
						help: None,
					})
				}
			}
			(Value::Duration { val: lhs, .. }, Value::Int { val: rhs, .. }) => {
				if let Some(val) = checked_div_floor_i64(*lhs, *rhs) {
					Ok(Value::duration(val, span))
				} else if *rhs == 0 {
					Err(ShellError::DivisionByZero { span: op })
				} else {
					Err(ShellError::OperatorOverflow {
						msg: "division operation overflowed".into(),
						span,
						help: None,
					})
				}
			}
			(Value::Duration { val: lhs, .. }, Value::Float { val: rhs, .. }) => {
				if let Some(val) = checked_div_floor_f64(*lhs as f64, *rhs) {
					if i64::MIN as f64 <= val && val <= i64::MAX as f64 {
						Ok(Value::duration(val as i64, span))
					} else {
						Err(ShellError::OperatorOverflow {
							msg: "division operation overflowed".into(),
							span,
							help: None,
						})
					}
				} else {
					Err(ShellError::DivisionByZero { span: op })
				}
			}
			(Value::Custom { val: lhs, .. }, rhs) => lhs.operation(self.span(), Operator::Math(Math::FloorDivide), op, rhs),
			_ => Err(operator_type_error(Operator::Math(Math::FloorDivide), op, self, rhs, |val| {
				matches!(val, Value::Int { .. } | Value::Float { .. } | Value::Duration { .. } | Value::Filesize { .. },)
			})),
		}
	}

	pub fn modulo(&self, op: Span, rhs: &Value, span: Span) -> Result<Value, ShellError> {
		// Based off the unstable `div_floor` function in the std library.
		fn checked_mod_i64(dividend: i64, divisor: i64) -> Option<i64> {
			let remainder = dividend.checked_rem(divisor)?;
			if (remainder > 0 && divisor < 0) || (remainder < 0 && divisor > 0) {
				// Note that `remainder + divisor` cannot overflow, because `remainder` and
				// `divisor` have opposite signs.
				Some(remainder + divisor)
			} else {
				Some(remainder)
			}
		}

		fn checked_mod_f64(dividend: f64, divisor: f64) -> Option<f64> {
			if divisor == 0.0 {
				None
			} else {
				let remainder = dividend % divisor;
				if (remainder > 0.0 && divisor < 0.0) || (remainder < 0.0 && divisor > 0.0) {
					Some(remainder + divisor)
				} else {
					Some(remainder)
				}
			}
		}

		match (self, rhs) {
			(Value::Int { val: lhs, .. }, Value::Int { val: rhs, .. }) => {
				if let Some(val) = checked_mod_i64(*lhs, *rhs) {
					Ok(Value::int(val, span))
				} else if *rhs == 0 {
					Err(ShellError::DivisionByZero { span: op })
				} else {
					Err(ShellError::OperatorOverflow {
						msg: "modulo operation overflowed".into(),
						span,
						help: None,
					})
				}
			}
			(Value::Int { val: lhs, .. }, Value::Float { val: rhs, .. }) => {
				if let Some(val) = checked_mod_f64(*lhs as f64, *rhs) {
					Ok(Value::float(val, span))
				} else {
					Err(ShellError::DivisionByZero { span: op })
				}
			}
			(Value::Float { val: lhs, .. }, Value::Int { val: rhs, .. }) => {
				if let Some(val) = checked_mod_f64(*lhs, *rhs as f64) {
					Ok(Value::float(val, span))
				} else {
					Err(ShellError::DivisionByZero { span: op })
				}
			}
			(Value::Float { val: lhs, .. }, Value::Float { val: rhs, .. }) => {
				if let Some(val) = checked_mod_f64(*lhs, *rhs) {
					Ok(Value::float(val, span))
				} else {
					Err(ShellError::DivisionByZero { span: op })
				}
			}
			(Value::Filesize { val: lhs, .. }, Value::Filesize { val: rhs, .. }) => {
				if let Some(val) = checked_mod_i64(lhs.get(), rhs.get()) {
					Ok(Value::filesize(val, span))
				} else if *rhs == Filesize::ZERO {
					Err(ShellError::DivisionByZero { span: op })
				} else {
					Err(ShellError::OperatorOverflow {
						msg: "modulo operation overflowed".into(),
						span,
						help: None,
					})
				}
			}
			(Value::Filesize { val: lhs, .. }, Value::Int { val: rhs, .. }) => {
				if let Some(val) = checked_mod_i64(lhs.get(), *rhs) {
					Ok(Value::filesize(val, span))
				} else if *rhs == 0 {
					Err(ShellError::DivisionByZero { span: op })
				} else {
					Err(ShellError::OperatorOverflow {
						msg: "modulo operation overflowed".into(),
						span,
						help: None,
					})
				}
			}
			(Value::Filesize { val: lhs, .. }, Value::Float { val: rhs, .. }) => {
				if let Some(val) = checked_mod_f64(lhs.get() as f64, *rhs) {
					if let Ok(val) = Filesize::try_from(val) {
						Ok(Value::filesize(val, span))
					} else {
						Err(ShellError::OperatorOverflow {
							msg: "modulo operation overflowed".into(),
							span,
							help: None,
						})
					}
				} else {
					Err(ShellError::DivisionByZero { span: op })
				}
			}
			(Value::Duration { val: lhs, .. }, Value::Duration { val: rhs, .. }) => {
				if let Some(val) = checked_mod_i64(*lhs, *rhs) {
					Ok(Value::duration(val, span))
				} else if *rhs == 0 {
					Err(ShellError::DivisionByZero { span: op })
				} else {
					Err(ShellError::OperatorOverflow {
						msg: "division operation overflowed".into(),
						span,
						help: None,
					})
				}
			}
			(Value::Duration { val: lhs, .. }, Value::Int { val: rhs, .. }) => {
				if let Some(val) = checked_mod_i64(*lhs, *rhs) {
					Ok(Value::duration(val, span))
				} else if *rhs == 0 {
					Err(ShellError::DivisionByZero { span: op })
				} else {
					Err(ShellError::OperatorOverflow {
						msg: "division operation overflowed".into(),
						span,
						help: None,
					})
				}
			}
			(Value::Duration { val: lhs, .. }, Value::Float { val: rhs, .. }) => {
				if let Some(val) = checked_mod_f64(*lhs as f64, *rhs) {
					if i64::MIN as f64 <= val && val <= i64::MAX as f64 {
						Ok(Value::duration(val as i64, span))
					} else {
						Err(ShellError::OperatorOverflow {
							msg: "division operation overflowed".into(),
							span,
							help: None,
						})
					}
				} else {
					Err(ShellError::DivisionByZero { span: op })
				}
			}
			(Value::Custom { val: lhs, .. }, rhs) => lhs.operation(span, Operator::Math(Math::Modulo), op, rhs),
			_ => Err(operator_type_error(Operator::Math(Math::Modulo), op, self, rhs, |val| {
				matches!(val, Value::Int { .. } | Value::Float { .. } | Value::Duration { .. } | Value::Filesize { .. },)
			})),
		}
	}

	pub fn pow(&self, op: Span, rhs: &Value, span: Span) -> Result<Value, ShellError> {
		match (self, rhs) {
			(Value::Int { val: lhs, .. }, Value::Int { val: rhsv, .. }) => {
				if *rhsv < 0 {
					return Err(ShellError::IncorrectValue {
						msg: "Negative exponent for integer power is unsupported; use floats instead.".into(),
						val_span: rhs.span(),
						call_span: op,
					});
				}

				if let Some(val) = lhs.checked_pow(*rhsv as u32) {
					Ok(Value::int(val, span))
				} else {
					Err(ShellError::OperatorOverflow {
                        msg: "pow operation overflowed".into(),
                        span,
                        help: Some("Consider using floating point values for increased range by promoting operand with 'into float'. Note: float has reduced precision!".into()),
                    })
				}
			}
			(Value::Int { val: lhs, .. }, Value::Float { val: rhs, .. }) => Ok(Value::float((*lhs as f64).powf(*rhs), span)),
			(Value::Float { val: lhs, .. }, Value::Int { val: rhs, .. }) => Ok(Value::float(lhs.powf(*rhs as f64), span)),
			(Value::Float { val: lhs, .. }, Value::Float { val: rhs, .. }) => Ok(Value::float(lhs.powf(*rhs), span)),
			(Value::Custom { val: lhs, .. }, rhs) => lhs.operation(span, Operator::Math(Math::Pow), op, rhs),
			_ => Err(operator_type_error(Operator::Math(Math::Pow), op, self, rhs, |val| {
				matches!(val, Value::Int { .. } | Value::Float { .. })
			})),
		}
	}

	pub fn concat(&self, op: Span, rhs: &Value, span: Span) -> Result<Value, ShellError> {
		match (self, rhs) {
			(Value::List { vals: lhs, .. }, Value::List { vals: rhs, .. }) => Ok(Value::list([lhs.as_slice(), rhs.as_slice()].concat(), span)),
			(Value::String { val: lhs, .. }, Value::String { val: rhs, .. }) => Ok(Value::string([lhs.as_str(), rhs.as_str()].join(""), span)),
			(Value::Binary { val: lhs, .. }, Value::Binary { val: rhs, .. }) => Ok(Value::binary([lhs.as_slice(), rhs.as_slice()].concat(), span)),
			(Value::Custom { val: lhs, .. }, rhs) => lhs.operation(self.span(), Operator::Math(Math::Concatenate), op, rhs),
			_ => {
				let help = if matches!(self, Value::List { .. }) || matches!(rhs, Value::List { .. }) {
					Some(
						"if you meant to append a value to a list or a record to a table, use the `append` command or wrap the value in a list. For example: `$list ++ $value` should be `$list ++ [$value]` or `$list | append $value`.",
					)
				} else {
					None
				};
				let is_supported = |val: &Value| matches!(val, Value::List { .. } | Value::String { .. } | Value::Binary { .. } | Value::Custom { .. });
				Err(match (is_supported(self), is_supported(rhs)) {
					(true, true) => ShellError::OperatorIncompatibleTypes {
						op: Operator::Math(Math::Concatenate),
						lhs: self.get_type(),
						rhs: rhs.get_type(),
						op_span: op,
						lhs_span: self.span(),
						rhs_span: rhs.span(),
						help,
					},
					(true, false) => ShellError::OperatorUnsupportedType {
						op: Operator::Math(Math::Concatenate),
						unsupported: rhs.get_type(),
						op_span: op,
						unsupported_span: rhs.span(),
						help,
					},
					(false, _) => ShellError::OperatorUnsupportedType {
						op: Operator::Math(Math::Concatenate),
						unsupported: self.get_type(),
						op_span: op,
						unsupported_span: self.span(),
						help,
					},
				})
			}
		}
	}

	pub fn lt(&self, op: Span, rhs: &Value, span: Span) -> Result<Value, ShellError> {
		if let (Value::Custom { val: lhs, .. }, rhs) = (self, rhs) {
			return lhs.operation(self.span(), Operator::Comparison(Comparison::LessThan), op, rhs);
		}

		if matches!(self, Value::Nothing { .. }) || matches!(rhs, Value::Nothing { .. }) {
			return Ok(Value::nothing(span));
		}

		if !type_compatible(self.get_type(), rhs.get_type()) {
			return Err(operator_type_error(Operator::Comparison(Comparison::LessThan), op, self, rhs, |val| {
				matches!(
					val,
					Value::Int { .. }
						| Value::Float { .. }
						| Value::String { .. }
						| Value::Filesize { .. }
						| Value::Duration { .. }
						| Value::Date { .. }
						| Value::Bool { .. }
						| Value::Nothing { .. }
				)
			}));
		}

		Ok(Value::bool(matches!(self.partial_cmp(rhs), Some(Ordering::Less)), span))
	}

	pub fn lte(&self, op: Span, rhs: &Value, span: Span) -> Result<Value, ShellError> {
		if let (Value::Custom { val: lhs, .. }, rhs) = (self, rhs) {
			return lhs.operation(self.span(), Operator::Comparison(Comparison::LessThanOrEqual), op, rhs);
		}

		if matches!(self, Value::Nothing { .. }) || matches!(rhs, Value::Nothing { .. }) {
			return Ok(Value::nothing(span));
		}

		if !type_compatible(self.get_type(), rhs.get_type()) {
			return Err(operator_type_error(Operator::Comparison(Comparison::LessThanOrEqual), op, self, rhs, |val| {
				matches!(
					val,
					Value::Int { .. }
						| Value::Float { .. }
						| Value::String { .. }
						| Value::Filesize { .. }
						| Value::Duration { .. }
						| Value::Date { .. }
						| Value::Bool { .. }
						| Value::Nothing { .. }
				)
			}));
		}

		Ok(Value::bool(matches!(self.partial_cmp(rhs), Some(Ordering::Less | Ordering::Equal)), span))
	}

	pub fn gt(&self, op: Span, rhs: &Value, span: Span) -> Result<Value, ShellError> {
		if let (Value::Custom { val: lhs, .. }, rhs) = (self, rhs) {
			return lhs.operation(self.span(), Operator::Comparison(Comparison::GreaterThan), op, rhs);
		}

		if matches!(self, Value::Nothing { .. }) || matches!(rhs, Value::Nothing { .. }) {
			return Ok(Value::nothing(span));
		}

		if !type_compatible(self.get_type(), rhs.get_type()) {
			return Err(operator_type_error(Operator::Comparison(Comparison::GreaterThan), op, self, rhs, |val| {
				matches!(
					val,
					Value::Int { .. }
						| Value::Float { .. }
						| Value::String { .. }
						| Value::Filesize { .. }
						| Value::Duration { .. }
						| Value::Date { .. }
						| Value::Bool { .. }
						| Value::Nothing { .. }
				)
			}));
		}

		Ok(Value::bool(matches!(self.partial_cmp(rhs), Some(Ordering::Greater)), span))
	}

	pub fn gte(&self, op: Span, rhs: &Value, span: Span) -> Result<Value, ShellError> {
		if let (Value::Custom { val: lhs, .. }, rhs) = (self, rhs) {
			return lhs.operation(self.span(), Operator::Comparison(Comparison::GreaterThanOrEqual), op, rhs);
		}

		if matches!(self, Value::Nothing { .. }) || matches!(rhs, Value::Nothing { .. }) {
			return Ok(Value::nothing(span));
		}

		if !type_compatible(self.get_type(), rhs.get_type()) {
			return Err(operator_type_error(
				Operator::Comparison(Comparison::GreaterThanOrEqual),
				op,
				self,
				rhs,
				|val| {
					matches!(
						val,
						Value::Int { .. }
							| Value::Float { .. } | Value::String { .. }
							| Value::Filesize { .. }
							| Value::Duration { .. }
							| Value::Date { .. } | Value::Bool { .. }
							| Value::Nothing { .. }
					)
				},
			));
		}

		Ok(Value::bool(matches!(self.partial_cmp(rhs), Some(Ordering::Greater | Ordering::Equal)), span))
	}

	pub fn eq(&self, op: Span, rhs: &Value, span: Span) -> Result<Value, ShellError> {
		if let (Value::Custom { val: lhs, .. }, rhs) = (self, rhs) {
			return lhs.operation(self.span(), Operator::Comparison(Comparison::Equal), op, rhs);
		}

		Ok(Value::bool(matches!(self.partial_cmp(rhs), Some(Ordering::Equal)), span))
	}

	pub fn ne(&self, op: Span, rhs: &Value, span: Span) -> Result<Value, ShellError> {
		if let (Value::Custom { val: lhs, .. }, rhs) = (self, rhs) {
			return lhs.operation(self.span(), Operator::Comparison(Comparison::NotEqual), op, rhs);
		}

		Ok(Value::bool(!matches!(self.partial_cmp(rhs), Some(Ordering::Equal)), span))
	}

	pub fn r#in(&self, op: Span, rhs: &Value, span: Span) -> Result<Value, ShellError> {
		match (self, rhs) {
			(lhs, Value::Range { val: rhs, .. }) => Ok(Value::bool(rhs.contains(lhs), span)),
			(Value::String { val: lhs, .. }, Value::String { val: rhs, .. }) => Ok(Value::bool(rhs.contains(lhs), span)),
			(lhs, Value::List { vals: rhs, .. }) => Ok(Value::bool(rhs.contains(lhs), span)),
			(Value::String { val: lhs, .. }, Value::Record { val: rhs, .. }) => Ok(Value::bool(rhs.contains(lhs), span)),
			(Value::String { .. } | Value::Int { .. }, Value::CellPath { val: rhs, .. }) => {
				let val = rhs.members.iter().any(|member| match (self, member) {
					(Value::Int { val: lhs, .. }, PathMember::Int { val: rhs, .. }) => *lhs == *rhs as i64,
					(Value::String { val: lhs, .. }, PathMember::String { val: rhs, .. }) => lhs == rhs,
					(Value::String { .. }, PathMember::Int { .. }) | (Value::Int { .. }, PathMember::String { .. }) => false,
					_ => unreachable!("outer match arm ensures `self` is either a `String` or `Int` variant"),
				});

				Ok(Value::bool(val, span))
			}
			(Value::CellPath { val: lhs, .. }, Value::CellPath { val: rhs, .. }) => Ok(Value::bool(
				rhs.members.windows(lhs.members.len()).any(|member_window| member_window == rhs.members),
				span,
			)),
			(Value::Custom { val: lhs, .. }, rhs) => lhs.operation(self.span(), Operator::Comparison(Comparison::In), op, rhs),
			(lhs, rhs) => Err(
				if let Value::List { .. } | Value::Range { .. } | Value::String { .. } | Value::Record { .. } | Value::Custom { .. } = rhs {
					ShellError::OperatorIncompatibleTypes {
						op: Operator::Comparison(Comparison::In),
						lhs: lhs.get_type(),
						rhs: rhs.get_type(),
						op_span: op,
						lhs_span: lhs.span(),
						rhs_span: rhs.span(),
						help: None,
					}
				} else {
					ShellError::OperatorUnsupportedType {
						op: Operator::Comparison(Comparison::In),
						unsupported: rhs.get_type(),
						op_span: op,
						unsupported_span: rhs.span(),
						help: None,
					}
				},
			),
		}
	}

	pub fn not_in(&self, op: Span, rhs: &Value, span: Span) -> Result<Value, ShellError> {
		match (self, rhs) {
			(lhs, Value::Range { val: rhs, .. }) => Ok(Value::bool(!rhs.contains(lhs), span)),
			(Value::String { val: lhs, .. }, Value::String { val: rhs, .. }) => Ok(Value::bool(!rhs.contains(lhs), span)),
			(lhs, Value::List { vals: rhs, .. }) => Ok(Value::bool(!rhs.contains(lhs), span)),
			(Value::String { val: lhs, .. }, Value::Record { val: rhs, .. }) => Ok(Value::bool(!rhs.contains(lhs), span)),
			(Value::String { .. } | Value::Int { .. }, Value::CellPath { val: rhs, .. }) => {
				let val = rhs.members.iter().any(|member| match (self, member) {
					(Value::Int { val: lhs, .. }, PathMember::Int { val: rhs, .. }) => *lhs != *rhs as i64,
					(Value::String { val: lhs, .. }, PathMember::String { val: rhs, .. }) => lhs != rhs,
					(Value::String { .. }, PathMember::Int { .. }) | (Value::Int { .. }, PathMember::String { .. }) => true,
					_ => unreachable!("outer match arm ensures `self` is either a `String` or `Int` variant"),
				});

				Ok(Value::bool(val, span))
			}
			(Value::CellPath { val: lhs, .. }, Value::CellPath { val: rhs, .. }) => Ok(Value::bool(
				rhs.members.windows(lhs.members.len()).all(|member_window| member_window != rhs.members),
				span,
			)),
			(Value::Custom { val: lhs, .. }, rhs) => lhs.operation(self.span(), Operator::Comparison(Comparison::NotIn), op, rhs),
			(lhs, rhs) => Err(
				if let Value::List { .. } | Value::Range { .. } | Value::String { .. } | Value::Record { .. } | Value::Custom { .. } = rhs {
					ShellError::OperatorIncompatibleTypes {
						op: Operator::Comparison(Comparison::NotIn),
						lhs: lhs.get_type(),
						rhs: rhs.get_type(),
						op_span: op,
						lhs_span: lhs.span(),
						rhs_span: rhs.span(),
						help: None,
					}
				} else {
					ShellError::OperatorUnsupportedType {
						op: Operator::Comparison(Comparison::NotIn),
						unsupported: rhs.get_type(),
						op_span: op,
						unsupported_span: rhs.span(),
						help: None,
					}
				},
			),
		}
	}

	pub fn has(&self, op: Span, rhs: &Value, span: Span) -> Result<Value, ShellError> {
		rhs.r#in(op, self, span)
	}

	pub fn not_has(&self, op: Span, rhs: &Value, span: Span) -> Result<Value, ShellError> {
		rhs.r#not_in(op, self, span)
	}

	pub fn regex_match(&self, engine_state: &EngineState, op: Span, rhs: &Value, invert: bool, span: Span) -> Result<Value, ShellError> {
		let rhs_span = rhs.span();
		match (self, rhs) {
			(Value::String { val: lhs, .. }, Value::String { val: rhs, .. }) => {
				let is_match = match engine_state.regex_cache.try_lock() {
					Ok(mut cache) => {
						if let Some(regex) = cache.get(rhs) {
							regex.is_match(lhs)
						} else {
							let regex = Regex::new(rhs).map_err(|e| ShellError::UnsupportedInput {
								msg: format!("{e}"),
								input: "value originated from here".into(),
								msg_span: span,
								input_span: rhs_span,
							})?;
							let ret = regex.is_match(lhs);
							cache.put(rhs.clone(), regex);
							ret
						}
					}
					Err(_) => {
						let regex = Regex::new(rhs).map_err(|e| ShellError::UnsupportedInput {
							msg: format!("{e}"),
							input: "value originated from here".into(),
							msg_span: span,
							input_span: rhs_span,
						})?;
						regex.is_match(lhs)
					}
				};

				Ok(Value::bool(if invert { !is_match.unwrap_or(false) } else { is_match.unwrap_or(true) }, span))
			}
			(Value::Custom { val: lhs, .. }, rhs) => lhs.operation(
				span,
				if invert {
					Operator::Comparison(Comparison::NotRegexMatch)
				} else {
					Operator::Comparison(Comparison::RegexMatch)
				},
				op,
				rhs,
			),
			_ => Err(operator_type_error(
				if invert {
					Operator::Comparison(Comparison::NotRegexMatch)
				} else {
					Operator::Comparison(Comparison::RegexMatch)
				},
				op,
				self,
				rhs,
				|val| matches!(val, Value::String { .. }),
			)),
		}
	}

	pub fn starts_with(&self, op: Span, rhs: &Value, span: Span) -> Result<Value, ShellError> {
		match (self, rhs) {
			(Value::String { val: lhs, .. }, Value::String { val: rhs, .. }) => Ok(Value::bool(lhs.starts_with(rhs), span)),
			(Value::Custom { val: lhs, .. }, rhs) => lhs.operation(self.span(), Operator::Comparison(Comparison::StartsWith), op, rhs),
			_ => Err(operator_type_error(Operator::Comparison(Comparison::StartsWith), op, self, rhs, |val| {
				matches!(val, Value::String { .. })
			})),
		}
	}

	pub fn not_starts_with(&self, op: Span, rhs: &Value, span: Span) -> Result<Value, ShellError> {
		match (self, rhs) {
			(Value::String { val: lhs, .. }, Value::String { val: rhs, .. }) => Ok(Value::bool(!lhs.starts_with(rhs), span)),
			(Value::Custom { val: lhs, .. }, rhs) => lhs.operation(self.span(), Operator::Comparison(Comparison::NotStartsWith), op, rhs),
			_ => Err(operator_type_error(Operator::Comparison(Comparison::NotStartsWith), op, self, rhs, |val| {
				matches!(val, Value::String { .. })
			})),
		}
	}

	pub fn ends_with(&self, op: Span, rhs: &Value, span: Span) -> Result<Value, ShellError> {
		match (self, rhs) {
			(Value::String { val: lhs, .. }, Value::String { val: rhs, .. }) => Ok(Value::bool(lhs.ends_with(rhs), span)),
			(Value::Custom { val: lhs, .. }, rhs) => lhs.operation(self.span(), Operator::Comparison(Comparison::EndsWith), op, rhs),
			_ => Err(operator_type_error(Operator::Comparison(Comparison::EndsWith), op, self, rhs, |val| {
				matches!(val, Value::String { .. })
			})),
		}
	}

	pub fn not_ends_with(&self, op: Span, rhs: &Value, span: Span) -> Result<Value, ShellError> {
		match (self, rhs) {
			(Value::String { val: lhs, .. }, Value::String { val: rhs, .. }) => Ok(Value::bool(!lhs.ends_with(rhs), span)),
			(Value::Custom { val: lhs, .. }, rhs) => lhs.operation(self.span(), Operator::Comparison(Comparison::NotEndsWith), op, rhs),
			_ => Err(operator_type_error(Operator::Comparison(Comparison::NotEndsWith), op, self, rhs, |val| {
				matches!(val, Value::String { .. })
			})),
		}
	}

	pub fn bit_or(&self, op: Span, rhs: &Value, span: Span) -> Result<Value, ShellError> {
		match (self, rhs) {
			(Value::Int { val: lhs, .. }, Value::Int { val: rhs, .. }) => Ok(Value::int(*lhs | rhs, span)),
			(Value::Custom { val: lhs, .. }, rhs) => lhs.operation(span, Operator::Bits(Bits::BitOr), op, rhs),
			_ => Err(operator_type_error(Operator::Bits(Bits::BitOr), op, self, rhs, |val| {
				matches!(val, Value::Int { .. })
			})),
		}
	}

	pub fn bit_xor(&self, op: Span, rhs: &Value, span: Span) -> Result<Value, ShellError> {
		match (self, rhs) {
			(Value::Int { val: lhs, .. }, Value::Int { val: rhs, .. }) => Ok(Value::int(*lhs ^ rhs, span)),
			(Value::Custom { val: lhs, .. }, rhs) => lhs.operation(span, Operator::Bits(Bits::BitXor), op, rhs),
			_ => Err(operator_type_error(Operator::Bits(Bits::BitXor), op, self, rhs, |val| {
				matches!(val, Value::Int { .. })
			})),
		}
	}

	pub fn bit_and(&self, op: Span, rhs: &Value, span: Span) -> Result<Value, ShellError> {
		match (self, rhs) {
			(Value::Int { val: lhs, .. }, Value::Int { val: rhs, .. }) => Ok(Value::int(*lhs & rhs, span)),
			(Value::Custom { val: lhs, .. }, rhs) => lhs.operation(span, Operator::Bits(Bits::BitAnd), op, rhs),
			_ => Err(operator_type_error(Operator::Bits(Bits::BitAnd), op, self, rhs, |val| {
				matches!(val, Value::Int { .. })
			})),
		}
	}

	pub fn bit_shl(&self, op: Span, rhs: &Value, span: Span) -> Result<Value, ShellError> {
		match (self, rhs) {
			(Value::Int { val: lhs, .. }, Value::Int { val: rhs, .. }) => {
				// Currently we disallow negative operands like Rust's `Shl`
				// Cheap guarding with TryInto<u32>
				if let Some(val) = (*rhs).try_into().ok().and_then(|rhs| lhs.checked_shl(rhs)) {
					Ok(Value::int(val, span))
				} else {
					Err(ShellError::OperatorOverflow {
						msg: "right operand to bit-shl exceeds available bits in underlying data".into(),
						span,
						help: Some(format!("Limit operand to 0 <= rhs < {}", i64::BITS)),
					})
				}
			}
			(Value::Custom { val: lhs, .. }, rhs) => lhs.operation(span, Operator::Bits(Bits::ShiftLeft), op, rhs),
			_ => Err(operator_type_error(Operator::Bits(Bits::ShiftLeft), op, self, rhs, |val| {
				matches!(val, Value::Int { .. })
			})),
		}
	}

	pub fn bit_shr(&self, op: Span, rhs: &Value, span: Span) -> Result<Value, ShellError> {
		match (self, rhs) {
			(Value::Int { val: lhs, .. }, Value::Int { val: rhs, .. }) => {
				// Currently we disallow negative operands like Rust's `Shr`
				// Cheap guarding with TryInto<u32>
				if let Some(val) = (*rhs).try_into().ok().and_then(|rhs| lhs.checked_shr(rhs)) {
					Ok(Value::int(val, span))
				} else {
					Err(ShellError::OperatorOverflow {
						msg: "right operand to bit-shr exceeds available bits in underlying data".into(),
						span,
						help: Some(format!("Limit operand to 0 <= rhs < {}", i64::BITS)),
					})
				}
			}
			(Value::Custom { val: lhs, .. }, rhs) => lhs.operation(span, Operator::Bits(Bits::ShiftRight), op, rhs),
			_ => Err(operator_type_error(Operator::Bits(Bits::ShiftRight), op, self, rhs, |val| {
				matches!(val, Value::Int { .. })
			})),
		}
	}

	pub fn or(&self, op: Span, rhs: &Value, span: Span) -> Result<Value, ShellError> {
		match (self, rhs) {
			(Value::Bool { val: lhs, .. }, Value::Bool { val: rhs, .. }) => Ok(Value::bool(*lhs || *rhs, span)),
			(Value::Custom { val: lhs, .. }, rhs) => lhs.operation(span, Operator::Boolean(Boolean::Or), op, rhs),
			_ => Err(operator_type_error(Operator::Boolean(Boolean::Or), op, self, rhs, |val| {
				matches!(val, Value::Bool { .. })
			})),
		}
	}

	pub fn xor(&self, op: Span, rhs: &Value, span: Span) -> Result<Value, ShellError> {
		match (self, rhs) {
			(Value::Bool { val: lhs, .. }, Value::Bool { val: rhs, .. }) => Ok(Value::bool((*lhs && !*rhs) || (!*lhs && *rhs), span)),
			(Value::Custom { val: lhs, .. }, rhs) => lhs.operation(span, Operator::Boolean(Boolean::Xor), op, rhs),
			_ => Err(operator_type_error(Operator::Boolean(Boolean::Xor), op, self, rhs, |val| {
				matches!(val, Value::Bool { .. })
			})),
		}
	}

	pub fn and(&self, op: Span, rhs: &Value, span: Span) -> Result<Value, ShellError> {
		match (self, rhs) {
			(Value::Bool { val: lhs, .. }, Value::Bool { val: rhs, .. }) => Ok(Value::bool(*lhs && *rhs, span)),
			(Value::Custom { val: lhs, .. }, rhs) => lhs.operation(span, Operator::Boolean(Boolean::And), op, rhs),
			_ => Err(operator_type_error(Operator::Boolean(Boolean::And), op, self, rhs, |val| {
				matches!(val, Value::Bool { .. })
			})),
		}
	}
}

// TODO: The name of this function is overly broad with partial compatibility
// Should be replaced by an explicitly named helper on `Type` (take `Any` into account)
fn type_compatible(a: Type, b: Type) -> bool {
	if a == b {
		return true;
	}

	matches!((a, b), (Type::Int, Type::Float) | (Type::Float, Type::Int))
}

fn operator_type_error(op: Operator, op_span: Span, lhs: &Value, rhs: &Value, is_supported: fn(&Value) -> bool) -> ShellError {
	let is_supported = |val| is_supported(val) || matches!(val, Value::Custom { .. });
	match (is_supported(lhs), is_supported(rhs)) {
		(true, true) => ShellError::OperatorIncompatibleTypes {
			op,
			lhs: lhs.get_type(),
			rhs: rhs.get_type(),
			op_span,
			lhs_span: lhs.span(),
			rhs_span: rhs.span(),
			help: None,
		},
		(true, false) => ShellError::OperatorUnsupportedType {
			op,
			unsupported: rhs.get_type(),
			op_span,
			unsupported_span: rhs.span(),
			help: None,
		},
		(false, _) => ShellError::OperatorUnsupportedType {
			op,
			unsupported: lhs.get_type(),
			op_span,
			unsupported_span: lhs.span(),
			help: None,
		},
	}
}

pub fn human_time_from_now(val: &DateTime<FixedOffset>) -> HumanTime {
	let now = Local::now().with_timezone(val.offset());
	let delta = *val - now;
	match delta.num_nanoseconds() {
		Some(num_nanoseconds) => {
			let delta_seconds = num_nanoseconds as f64 / 1_000_000_000.0;
			let delta_seconds_rounded = delta_seconds.round() as i64;
			HumanTime::from(Duration::seconds(delta_seconds_rounded))
		}
		None => {
			// Happens if the total number of nanoseconds exceeds what fits in an i64
			// Note: not using delta.num_days() because it results is wrong for years before ~936: a extra year is added
			let delta_years = val.year() - now.year();
			HumanTime::from(Duration::days(delta_years as i64 * 365))
		}
	}
}
