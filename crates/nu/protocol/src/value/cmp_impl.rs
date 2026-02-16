impl Default for Value {
	fn default() -> Self {
		Value::Nothing {
			internal_span: Span::unknown(),
		}
	}
}

impl PartialOrd for Value {
	fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
		// Compare two floating point numbers. The decision interval for equality is dynamically
		// scaled as the value being compared increases in magnitude (using relative epsilon-based
		// tolerance). Implementation is similar to python's `math.isclose()` function:
		// https://docs.python.org/3/library/math.html#math.isclose. Fallback to the default strict
		// float comparison if the difference exceeds the error epsilon.
		fn compare_floats(val: f64, other: f64) -> Option<Ordering> {
			let prec = f64::EPSILON.max(val.abs().max(other.abs()) * f64::EPSILON);

			if (other - val).abs() <= prec {
				return Some(Ordering::Equal);
			}

			val.partial_cmp(&other)
		}

		match (self, other) {
			(Value::Bool { val: lhs, .. }, rhs) => match rhs {
				Value::Bool { val: rhs, .. } => lhs.partial_cmp(rhs),
				Value::Int { .. } => Some(Ordering::Less),
				Value::Float { .. } => Some(Ordering::Less),
				Value::String { .. } => Some(Ordering::Less),
				Value::Glob { .. } => Some(Ordering::Less),
				Value::Filesize { .. } => Some(Ordering::Less),
				Value::Duration { .. } => Some(Ordering::Less),
				Value::Date { .. } => Some(Ordering::Less),
				Value::Range { .. } => Some(Ordering::Less),
				Value::Record { .. } => Some(Ordering::Less),
				Value::List { .. } => Some(Ordering::Less),
				Value::Closure { .. } => Some(Ordering::Less),
				Value::Error { .. } => Some(Ordering::Less),
				Value::Binary { .. } => Some(Ordering::Less),
				Value::CellPath { .. } => Some(Ordering::Less),
				Value::Custom { .. } => Some(Ordering::Less),
				Value::Nothing { .. } => Some(Ordering::Less),
			},
			(Value::Int { val: lhs, .. }, rhs) => match rhs {
				Value::Bool { .. } => Some(Ordering::Greater),
				Value::Int { val: rhs, .. } => lhs.partial_cmp(rhs),
				Value::Float { val: rhs, .. } => compare_floats(*lhs as f64, *rhs),
				Value::String { .. } => Some(Ordering::Less),
				Value::Glob { .. } => Some(Ordering::Less),
				Value::Filesize { .. } => Some(Ordering::Less),
				Value::Duration { .. } => Some(Ordering::Less),
				Value::Date { .. } => Some(Ordering::Less),
				Value::Range { .. } => Some(Ordering::Less),
				Value::Record { .. } => Some(Ordering::Less),
				Value::List { .. } => Some(Ordering::Less),
				Value::Closure { .. } => Some(Ordering::Less),
				Value::Error { .. } => Some(Ordering::Less),
				Value::Binary { .. } => Some(Ordering::Less),
				Value::CellPath { .. } => Some(Ordering::Less),
				Value::Custom { .. } => Some(Ordering::Less),
				Value::Nothing { .. } => Some(Ordering::Less),
			},
			(Value::Float { val: lhs, .. }, rhs) => match rhs {
				Value::Bool { .. } => Some(Ordering::Greater),
				Value::Int { val: rhs, .. } => compare_floats(*lhs, *rhs as f64),
				Value::Float { val: rhs, .. } => compare_floats(*lhs, *rhs),
				Value::String { .. } => Some(Ordering::Less),
				Value::Glob { .. } => Some(Ordering::Less),
				Value::Filesize { .. } => Some(Ordering::Less),
				Value::Duration { .. } => Some(Ordering::Less),
				Value::Date { .. } => Some(Ordering::Less),
				Value::Range { .. } => Some(Ordering::Less),
				Value::Record { .. } => Some(Ordering::Less),
				Value::List { .. } => Some(Ordering::Less),
				Value::Closure { .. } => Some(Ordering::Less),
				Value::Error { .. } => Some(Ordering::Less),
				Value::Binary { .. } => Some(Ordering::Less),
				Value::CellPath { .. } => Some(Ordering::Less),
				Value::Custom { .. } => Some(Ordering::Less),
				Value::Nothing { .. } => Some(Ordering::Less),
			},
			(Value::String { val: lhs, .. }, rhs) => match rhs {
				Value::Bool { .. } => Some(Ordering::Greater),
				Value::Int { .. } => Some(Ordering::Greater),
				Value::Float { .. } => Some(Ordering::Greater),
				Value::String { val: rhs, .. } => lhs.partial_cmp(rhs),
				Value::Glob { val: rhs, .. } => lhs.partial_cmp(rhs),
				Value::Filesize { .. } => Some(Ordering::Less),
				Value::Duration { .. } => Some(Ordering::Less),
				Value::Date { .. } => Some(Ordering::Less),
				Value::Range { .. } => Some(Ordering::Less),
				Value::Record { .. } => Some(Ordering::Less),
				Value::List { .. } => Some(Ordering::Less),
				Value::Closure { .. } => Some(Ordering::Less),
				Value::Error { .. } => Some(Ordering::Less),
				Value::Binary { .. } => Some(Ordering::Less),
				Value::CellPath { .. } => Some(Ordering::Less),
				Value::Custom { .. } => Some(Ordering::Less),
				Value::Nothing { .. } => Some(Ordering::Less),
			},
			(Value::Glob { val: lhs, .. }, rhs) => match rhs {
				Value::Bool { .. } => Some(Ordering::Greater),
				Value::Int { .. } => Some(Ordering::Greater),
				Value::Float { .. } => Some(Ordering::Greater),
				Value::String { val: rhs, .. } => lhs.partial_cmp(rhs),
				Value::Glob { val: rhs, .. } => lhs.partial_cmp(rhs),
				Value::Filesize { .. } => Some(Ordering::Less),
				Value::Duration { .. } => Some(Ordering::Less),
				Value::Date { .. } => Some(Ordering::Less),
				Value::Range { .. } => Some(Ordering::Less),
				Value::Record { .. } => Some(Ordering::Less),
				Value::List { .. } => Some(Ordering::Less),
				Value::Closure { .. } => Some(Ordering::Less),
				Value::Error { .. } => Some(Ordering::Less),
				Value::Binary { .. } => Some(Ordering::Less),
				Value::CellPath { .. } => Some(Ordering::Less),
				Value::Custom { .. } => Some(Ordering::Less),
				Value::Nothing { .. } => Some(Ordering::Less),
			},
			(Value::Filesize { val: lhs, .. }, rhs) => match rhs {
				Value::Bool { .. } => Some(Ordering::Greater),
				Value::Int { .. } => Some(Ordering::Greater),
				Value::Float { .. } => Some(Ordering::Greater),
				Value::String { .. } => Some(Ordering::Greater),
				Value::Glob { .. } => Some(Ordering::Greater),
				Value::Filesize { val: rhs, .. } => lhs.partial_cmp(rhs),
				Value::Duration { .. } => Some(Ordering::Less),
				Value::Date { .. } => Some(Ordering::Less),
				Value::Range { .. } => Some(Ordering::Less),
				Value::Record { .. } => Some(Ordering::Less),
				Value::List { .. } => Some(Ordering::Less),
				Value::Closure { .. } => Some(Ordering::Less),
				Value::Error { .. } => Some(Ordering::Less),
				Value::Binary { .. } => Some(Ordering::Less),
				Value::CellPath { .. } => Some(Ordering::Less),
				Value::Custom { .. } => Some(Ordering::Less),
				Value::Nothing { .. } => Some(Ordering::Less),
			},
			(Value::Duration { val: lhs, .. }, rhs) => match rhs {
				Value::Bool { .. } => Some(Ordering::Greater),
				Value::Int { .. } => Some(Ordering::Greater),
				Value::Float { .. } => Some(Ordering::Greater),
				Value::String { .. } => Some(Ordering::Greater),
				Value::Glob { .. } => Some(Ordering::Greater),
				Value::Filesize { .. } => Some(Ordering::Greater),
				Value::Duration { val: rhs, .. } => lhs.partial_cmp(rhs),
				Value::Date { .. } => Some(Ordering::Less),
				Value::Range { .. } => Some(Ordering::Less),
				Value::Record { .. } => Some(Ordering::Less),
				Value::List { .. } => Some(Ordering::Less),
				Value::Closure { .. } => Some(Ordering::Less),
				Value::Error { .. } => Some(Ordering::Less),
				Value::Binary { .. } => Some(Ordering::Less),
				Value::CellPath { .. } => Some(Ordering::Less),
				Value::Custom { .. } => Some(Ordering::Less),
				Value::Nothing { .. } => Some(Ordering::Less),
			},
			(Value::Date { val: lhs, .. }, rhs) => match rhs {
				Value::Bool { .. } => Some(Ordering::Greater),
				Value::Int { .. } => Some(Ordering::Greater),
				Value::Float { .. } => Some(Ordering::Greater),
				Value::String { .. } => Some(Ordering::Greater),
				Value::Glob { .. } => Some(Ordering::Greater),
				Value::Filesize { .. } => Some(Ordering::Greater),
				Value::Duration { .. } => Some(Ordering::Greater),
				Value::Date { val: rhs, .. } => lhs.partial_cmp(rhs),
				Value::Range { .. } => Some(Ordering::Less),
				Value::Record { .. } => Some(Ordering::Less),
				Value::List { .. } => Some(Ordering::Less),
				Value::Closure { .. } => Some(Ordering::Less),
				Value::Error { .. } => Some(Ordering::Less),
				Value::Binary { .. } => Some(Ordering::Less),
				Value::CellPath { .. } => Some(Ordering::Less),
				Value::Custom { .. } => Some(Ordering::Less),
				Value::Nothing { .. } => Some(Ordering::Less),
			},
			(Value::Range { val: lhs, .. }, rhs) => match rhs {
				Value::Bool { .. } => Some(Ordering::Greater),
				Value::Int { .. } => Some(Ordering::Greater),
				Value::Float { .. } => Some(Ordering::Greater),
				Value::String { .. } => Some(Ordering::Greater),
				Value::Glob { .. } => Some(Ordering::Greater),
				Value::Filesize { .. } => Some(Ordering::Greater),
				Value::Duration { .. } => Some(Ordering::Greater),
				Value::Date { .. } => Some(Ordering::Greater),
				Value::Range { val: rhs, .. } => lhs.partial_cmp(rhs),
				Value::Record { .. } => Some(Ordering::Less),
				Value::List { .. } => Some(Ordering::Less),
				Value::Closure { .. } => Some(Ordering::Less),
				Value::Error { .. } => Some(Ordering::Less),
				Value::Binary { .. } => Some(Ordering::Less),
				Value::CellPath { .. } => Some(Ordering::Less),
				Value::Custom { .. } => Some(Ordering::Less),
				Value::Nothing { .. } => Some(Ordering::Less),
			},
			(Value::Record { val: lhs, .. }, rhs) => match rhs {
				Value::Bool { .. } => Some(Ordering::Greater),
				Value::Int { .. } => Some(Ordering::Greater),
				Value::Float { .. } => Some(Ordering::Greater),
				Value::String { .. } => Some(Ordering::Greater),
				Value::Glob { .. } => Some(Ordering::Greater),
				Value::Filesize { .. } => Some(Ordering::Greater),
				Value::Duration { .. } => Some(Ordering::Greater),
				Value::Date { .. } => Some(Ordering::Greater),
				Value::Range { .. } => Some(Ordering::Greater),
				Value::Record { val: rhs, .. } => {
					// reorder cols and vals to make more logically compare.
					// more general, if two record have same col and values,
					// the order of cols shouldn't affect the equal property.
					let mut lhs = lhs.clone().into_owned();
					let mut rhs = rhs.clone().into_owned();
					lhs.sort_cols();
					rhs.sort_cols();

					// Check columns first
					for (a, b) in lhs.columns().zip(rhs.columns()) {
						let result = a.partial_cmp(b);
						if result != Some(Ordering::Equal) {
							return result;
						}
					}
					// Then check the values
					for (a, b) in lhs.values().zip(rhs.values()) {
						let result = a.partial_cmp(b);
						if result != Some(Ordering::Equal) {
							return result;
						}
					}
					// If all of the comparisons were equal, then lexicographical order dictates
					// that the shorter sequence is less than the longer one
					lhs.len().partial_cmp(&rhs.len())
				}
				Value::List { .. } => Some(Ordering::Less),
				Value::Closure { .. } => Some(Ordering::Less),
				Value::Error { .. } => Some(Ordering::Less),
				Value::Binary { .. } => Some(Ordering::Less),
				Value::CellPath { .. } => Some(Ordering::Less),
				Value::Custom { .. } => Some(Ordering::Less),
				Value::Nothing { .. } => Some(Ordering::Less),
			},
			(Value::List { vals: lhs, .. }, rhs) => match rhs {
				Value::Bool { .. } => Some(Ordering::Greater),
				Value::Int { .. } => Some(Ordering::Greater),
				Value::Float { .. } => Some(Ordering::Greater),
				Value::String { .. } => Some(Ordering::Greater),
				Value::Glob { .. } => Some(Ordering::Greater),
				Value::Filesize { .. } => Some(Ordering::Greater),
				Value::Duration { .. } => Some(Ordering::Greater),
				Value::Date { .. } => Some(Ordering::Greater),
				Value::Range { .. } => Some(Ordering::Greater),
				Value::Record { .. } => Some(Ordering::Greater),
				Value::List { vals: rhs, .. } => lhs.partial_cmp(rhs),
				Value::Closure { .. } => Some(Ordering::Less),
				Value::Error { .. } => Some(Ordering::Less),
				Value::Binary { .. } => Some(Ordering::Less),
				Value::CellPath { .. } => Some(Ordering::Less),
				Value::Custom { .. } => Some(Ordering::Less),
				Value::Nothing { .. } => Some(Ordering::Less),
			},
			(Value::Closure { val: lhs, .. }, rhs) => match rhs {
				Value::Bool { .. } => Some(Ordering::Greater),
				Value::Int { .. } => Some(Ordering::Greater),
				Value::Float { .. } => Some(Ordering::Greater),
				Value::String { .. } => Some(Ordering::Greater),
				Value::Glob { .. } => Some(Ordering::Greater),
				Value::Filesize { .. } => Some(Ordering::Greater),
				Value::Duration { .. } => Some(Ordering::Greater),
				Value::Date { .. } => Some(Ordering::Greater),
				Value::Range { .. } => Some(Ordering::Greater),
				Value::Record { .. } => Some(Ordering::Greater),
				Value::List { .. } => Some(Ordering::Greater),
				Value::Closure { val: rhs, .. } => lhs.block_id.partial_cmp(&rhs.block_id),
				Value::Error { .. } => Some(Ordering::Less),
				Value::Binary { .. } => Some(Ordering::Less),
				Value::CellPath { .. } => Some(Ordering::Less),
				Value::Custom { .. } => Some(Ordering::Less),
				Value::Nothing { .. } => Some(Ordering::Less),
			},
			(Value::Error { .. }, rhs) => match rhs {
				Value::Bool { .. } => Some(Ordering::Greater),
				Value::Int { .. } => Some(Ordering::Greater),
				Value::Float { .. } => Some(Ordering::Greater),
				Value::String { .. } => Some(Ordering::Greater),
				Value::Glob { .. } => Some(Ordering::Greater),
				Value::Filesize { .. } => Some(Ordering::Greater),
				Value::Duration { .. } => Some(Ordering::Greater),
				Value::Date { .. } => Some(Ordering::Greater),
				Value::Range { .. } => Some(Ordering::Greater),
				Value::Record { .. } => Some(Ordering::Greater),
				Value::List { .. } => Some(Ordering::Greater),
				Value::Closure { .. } => Some(Ordering::Greater),
				Value::Error { .. } => Some(Ordering::Equal),
				Value::Binary { .. } => Some(Ordering::Less),
				Value::CellPath { .. } => Some(Ordering::Less),
				Value::Custom { .. } => Some(Ordering::Less),
				Value::Nothing { .. } => Some(Ordering::Less),
			},
			(Value::Binary { val: lhs, .. }, rhs) => match rhs {
				Value::Bool { .. } => Some(Ordering::Greater),
				Value::Int { .. } => Some(Ordering::Greater),
				Value::Float { .. } => Some(Ordering::Greater),
				Value::String { .. } => Some(Ordering::Greater),
				Value::Glob { .. } => Some(Ordering::Greater),
				Value::Filesize { .. } => Some(Ordering::Greater),
				Value::Duration { .. } => Some(Ordering::Greater),
				Value::Date { .. } => Some(Ordering::Greater),
				Value::Range { .. } => Some(Ordering::Greater),
				Value::Record { .. } => Some(Ordering::Greater),
				Value::List { .. } => Some(Ordering::Greater),
				Value::Closure { .. } => Some(Ordering::Greater),
				Value::Error { .. } => Some(Ordering::Greater),
				Value::Binary { val: rhs, .. } => lhs.partial_cmp(rhs),
				Value::CellPath { .. } => Some(Ordering::Less),
				Value::Custom { .. } => Some(Ordering::Less),
				Value::Nothing { .. } => Some(Ordering::Less),
			},
			(Value::CellPath { val: lhs, .. }, rhs) => match rhs {
				Value::Bool { .. } => Some(Ordering::Greater),
				Value::Int { .. } => Some(Ordering::Greater),
				Value::Float { .. } => Some(Ordering::Greater),
				Value::String { .. } => Some(Ordering::Greater),
				Value::Glob { .. } => Some(Ordering::Greater),
				Value::Filesize { .. } => Some(Ordering::Greater),
				Value::Duration { .. } => Some(Ordering::Greater),
				Value::Date { .. } => Some(Ordering::Greater),
				Value::Range { .. } => Some(Ordering::Greater),
				Value::Record { .. } => Some(Ordering::Greater),
				Value::List { .. } => Some(Ordering::Greater),
				Value::Closure { .. } => Some(Ordering::Greater),
				Value::Error { .. } => Some(Ordering::Greater),
				Value::Binary { .. } => Some(Ordering::Greater),
				Value::CellPath { val: rhs, .. } => lhs.partial_cmp(rhs),
				Value::Custom { .. } => Some(Ordering::Less),
				Value::Nothing { .. } => Some(Ordering::Less),
			},
			(Value::Custom { val: lhs, .. }, rhs) => lhs.partial_cmp(rhs),
			(Value::Nothing { .. }, rhs) => match rhs {
				Value::Bool { .. } => Some(Ordering::Greater),
				Value::Int { .. } => Some(Ordering::Greater),
				Value::Float { .. } => Some(Ordering::Greater),
				Value::String { .. } => Some(Ordering::Greater),
				Value::Glob { .. } => Some(Ordering::Greater),
				Value::Filesize { .. } => Some(Ordering::Greater),
				Value::Duration { .. } => Some(Ordering::Greater),
				Value::Date { .. } => Some(Ordering::Greater),
				Value::Range { .. } => Some(Ordering::Greater),
				Value::Record { .. } => Some(Ordering::Greater),
				Value::List { .. } => Some(Ordering::Greater),
				Value::Closure { .. } => Some(Ordering::Greater),
				Value::Error { .. } => Some(Ordering::Greater),
				Value::Binary { .. } => Some(Ordering::Greater),
				Value::CellPath { .. } => Some(Ordering::Greater),
				Value::Custom { .. } => Some(Ordering::Greater),
				Value::Nothing { .. } => Some(Ordering::Equal),
			},
		}
	}
}

impl PartialEq for Value {
	fn eq(&self, other: &Self) -> bool {
		self.partial_cmp(other).is_some_and(Ordering::is_eq)
	}
}

fn checked_duration_operation<F>(a: i64, b: i64, op: F, span: Span) -> Result<Value, ShellError>
where
	F: Fn(i64, i64) -> Option<i64>,
{
	if let Some(val) = op(a, b) {
		Ok(Value::duration(val, span))
	} else {
		Err(ShellError::OperatorOverflow {
			msg: "operation overflowed".to_owned(),
			span,
			help: None,
		})
	}
}
