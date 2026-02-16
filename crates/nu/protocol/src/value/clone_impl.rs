impl Clone for Value {
	fn clone(&self) -> Self {
		match self {
			Value::Bool { val, internal_span } => Value::bool(*val, *internal_span),
			Value::Int { val, internal_span } => Value::int(*val, *internal_span),
			Value::Filesize { val, internal_span } => Value::Filesize {
				val: *val,
				internal_span: *internal_span,
			},
			Value::Duration { val, internal_span } => Value::Duration {
				val: *val,
				internal_span: *internal_span,
			},
			Value::Date { val, internal_span } => Value::Date {
				val: *val,
				internal_span: *internal_span,
			},
			Value::Range { val, signals, internal_span } => Value::Range {
				val: val.clone(),
				signals: signals.clone(),
				internal_span: *internal_span,
			},
			Value::Float { val, internal_span } => Value::float(*val, *internal_span),
			Value::String { val, internal_span } => Value::String {
				val: val.clone(),
				internal_span: *internal_span,
			},
			Value::Glob {
				val,
				no_expand: quoted,
				internal_span,
			} => Value::Glob {
				val: val.clone(),
				no_expand: *quoted,
				internal_span: *internal_span,
			},
			Value::Record { val, internal_span } => Value::Record {
				val: val.clone(),
				internal_span: *internal_span,
			},
			Value::List { vals, signals, internal_span } => Value::List {
				vals: vals.clone(),
				signals: signals.clone(),
				internal_span: *internal_span,
			},
			Value::Closure { val, internal_span } => Value::Closure {
				val: val.clone(),
				internal_span: *internal_span,
			},
			Value::Nothing { internal_span } => Value::Nothing { internal_span: *internal_span },
			Value::Error { error, internal_span } => Value::Error {
				error: error.clone(),
				internal_span: *internal_span,
			},
			Value::Binary { val, internal_span } => Value::Binary {
				val: val.clone(),
				internal_span: *internal_span,
			},
			Value::CellPath { val, internal_span } => Value::CellPath {
				val: val.clone(),
				internal_span: *internal_span,
			},
			Value::Custom { val, internal_span } => val.clone_value(*internal_span),
		}
	}
}
