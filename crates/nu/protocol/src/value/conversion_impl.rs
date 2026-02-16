impl Value {
	fn cant_convert_to<T>(&self, typ: &str) -> Result<T, ShellError> {
		Err(ShellError::CantConvert {
			to_type: typ.into(),
			from_type: self.get_type().to_string(),
			span: self.span(),
			help: None,
		})
	}

	/// Returns the inner `bool` value or an error if this `Value` is not a bool
	pub fn as_bool(&self) -> Result<bool, ShellError> {
		if let Value::Bool { val, .. } = self {
			Ok(*val)
		} else {
			self.cant_convert_to("boolean")
		}
	}

	/// Returns the inner `i64` value or an error if this `Value` is not an int
	pub fn as_int(&self) -> Result<i64, ShellError> {
		if let Value::Int { val, .. } = self {
			Ok(*val)
		} else {
			self.cant_convert_to("int")
		}
	}

	/// Returns the inner `f64` value or an error if this `Value` is not a float
	pub fn as_float(&self) -> Result<f64, ShellError> {
		if let Value::Float { val, .. } = self {
			Ok(*val)
		} else {
			self.cant_convert_to("float")
		}
	}

	/// Returns this `Value` converted to a `f64` or an error if it cannot be converted
	///
	/// Only the following `Value` cases will return an `Ok` result:
	/// - `Int`
	/// - `Float`
	///
	/// ```
	/// # use xeno_nu_protocol::Value;
	/// for val in Value::test_values() {
	///     assert_eq!(
	///         matches!(val, Value::Float { .. } | Value::Int { .. }),
	///         val.coerce_float().is_ok(),
	///     );
	/// }
	/// ```
	pub fn coerce_float(&self) -> Result<f64, ShellError> {
		match self {
			Value::Float { val, .. } => Ok(*val),
			Value::Int { val, .. } => Ok(*val as f64),
			val => val.cant_convert_to("float"),
		}
	}

	/// Returns the inner `i64` filesize value or an error if this `Value` is not a filesize
	pub fn as_filesize(&self) -> Result<Filesize, ShellError> {
		if let Value::Filesize { val, .. } = self {
			Ok(*val)
		} else {
			self.cant_convert_to("filesize")
		}
	}

	/// Returns the inner `i64` duration value or an error if this `Value` is not a duration
	pub fn as_duration(&self) -> Result<i64, ShellError> {
		if let Value::Duration { val, .. } = self {
			Ok(*val)
		} else {
			self.cant_convert_to("duration")
		}
	}

	/// Returns the inner [`DateTime`] value or an error if this `Value` is not a date
	pub fn as_date(&self) -> Result<DateTime<FixedOffset>, ShellError> {
		if let Value::Date { val, .. } = self {
			Ok(*val)
		} else {
			self.cant_convert_to("datetime")
		}
	}

	/// Returns a reference to the inner [`Range`] value or an error if this `Value` is not a range
	pub fn as_range(&self) -> Result<Range, ShellError> {
		if let Value::Range { val, .. } = self {
			Ok(**val)
		} else {
			self.cant_convert_to("range")
		}
	}

	/// Unwraps the inner [`Range`] value or returns an error if this `Value` is not a range
	pub fn into_range(self) -> Result<Range, ShellError> {
		if let Value::Range { val, .. } = self {
			Ok(*val)
		} else {
			self.cant_convert_to("range")
		}
	}

	/// Returns a reference to the inner `str` value or an error if this `Value` is not a string
	pub fn as_str(&self) -> Result<&str, ShellError> {
		if let Value::String { val, .. } = self {
			Ok(val)
		} else {
			self.cant_convert_to("string")
		}
	}

	/// Unwraps the inner `String` value or returns an error if this `Value` is not a string
	pub fn into_string(self) -> Result<String, ShellError> {
		if let Value::String { val, .. } = self {
			Ok(val)
		} else {
			self.cant_convert_to("string")
		}
	}

	/// Returns this `Value` converted to a `str` or an error if it cannot be converted
	///
	/// Only the following `Value` cases will return an `Ok` result:
	/// - `Bool`
	/// - `Int`
	/// - `Float`
	/// - `String`
	/// - `Glob`
	/// - `Binary` (only if valid utf-8)
	/// - `Date`
	///
	/// ```
	/// # use xeno_nu_protocol::Value;
	/// for val in Value::test_values() {
	///     assert_eq!(
	///         matches!(
	///             val,
	///             Value::Bool { .. }
	///                 | Value::Int { .. }
	///                 | Value::Float { .. }
	///                 | Value::String { .. }
	///                 | Value::Glob { .. }
	///                 | Value::Binary { .. }
	///                 | Value::Date { .. }
	///         ),
	///         val.coerce_str().is_ok(),
	///     );
	/// }
	/// ```
	pub fn coerce_str(&self) -> Result<Cow<'_, str>, ShellError> {
		match self {
			Value::Bool { val, .. } => Ok(Cow::Owned(val.to_string())),
			Value::Int { val, .. } => Ok(Cow::Owned(val.to_string())),
			Value::Float { val, .. } => Ok(Cow::Owned(val.to_string())),
			Value::String { val, .. } => Ok(Cow::Borrowed(val)),
			Value::Glob { val, .. } => Ok(Cow::Borrowed(val)),
			Value::Binary { val, .. } => match std::str::from_utf8(val) {
				Ok(s) => Ok(Cow::Borrowed(s)),
				Err(_) => self.cant_convert_to("string"),
			},
			Value::Date { val, .. } => Ok(Cow::Owned(val.to_rfc3339_opts(chrono::SecondsFormat::Millis, true))),
			val => val.cant_convert_to("string"),
		}
	}

	/// Returns this `Value` converted to a `String` or an error if it cannot be converted
	///
	/// # Note
	/// This function is equivalent to `value.coerce_str().map(Cow::into_owned)`
	/// which might allocate a new `String`.
	///
	/// To avoid this allocation, prefer [`coerce_str`](Self::coerce_str)
	/// if you do not need an owned `String`,
	/// or [`coerce_into_string`](Self::coerce_into_string)
	/// if you do not need to keep the original `Value` around.
	///
	/// Only the following `Value` cases will return an `Ok` result:
	/// - `Bool`
	/// - `Int`
	/// - `Float`
	/// - `String`
	/// - `Glob`
	/// - `Binary` (only if valid utf-8)
	/// - `Date`
	///
	/// ```
	/// # use xeno_nu_protocol::Value;
	/// for val in Value::test_values() {
	///     assert_eq!(
	///         matches!(
	///             val,
	///             Value::Bool { .. }
	///                 | Value::Int { .. }
	///                 | Value::Float { .. }
	///                 | Value::String { .. }
	///                 | Value::Glob { .. }
	///                 | Value::Binary { .. }
	///                 | Value::Date { .. }
	///         ),
	///         val.coerce_string().is_ok(),
	///     );
	/// }
	/// ```
	pub fn coerce_string(&self) -> Result<String, ShellError> {
		self.coerce_str().map(Cow::into_owned)
	}

	/// Returns this `Value` converted to a `String` or an error if it cannot be converted
	///
	/// Only the following `Value` cases will return an `Ok` result:
	/// - `Bool`
	/// - `Int`
	/// - `Float`
	/// - `String`
	/// - `Glob`
	/// - `Binary` (only if valid utf-8)
	/// - `Date`
	///
	/// ```
	/// # use xeno_nu_protocol::Value;
	/// for val in Value::test_values() {
	///     assert_eq!(
	///         matches!(
	///             val,
	///             Value::Bool { .. }
	///                 | Value::Int { .. }
	///                 | Value::Float { .. }
	///                 | Value::String { .. }
	///                 | Value::Glob { .. }
	///                 | Value::Binary { .. }
	///                 | Value::Date { .. }
	///         ),
	///         val.coerce_into_string().is_ok(),
	///     );
	/// }
	/// ```
	pub fn coerce_into_string(self) -> Result<String, ShellError> {
		let span = self.span();
		match self {
			Value::Bool { val, .. } => Ok(val.to_string()),
			Value::Int { val, .. } => Ok(val.to_string()),
			Value::Float { val, .. } => Ok(val.to_string()),
			Value::String { val, .. } => Ok(val),
			Value::Glob { val, .. } => Ok(val),
			Value::Binary { val, .. } => match String::from_utf8(val) {
				Ok(s) => Ok(s),
				Err(err) => Value::binary(err.into_bytes(), span).cant_convert_to("string"),
			},
			Value::Date { val, .. } => Ok(val.to_rfc3339_opts(chrono::SecondsFormat::Millis, true)),
			val => val.cant_convert_to("string"),
		}
	}

	/// Returns this `Value` as a `char` or an error if it is not a single character string
	pub fn as_char(&self) -> Result<char, ShellError> {
		let span = self.span();
		if let Value::String { val, .. } = self {
			let mut chars = val.chars();
			match (chars.next(), chars.next()) {
				(Some(c), None) => Ok(c),
				_ => Err(ShellError::MissingParameter {
					param_name: "single character separator".into(),
					span,
				}),
			}
		} else {
			self.cant_convert_to("char")
		}
	}

	/// Converts this `Value` to a `PathBuf` or returns an error if it is not a string
	pub fn to_path(&self) -> Result<PathBuf, ShellError> {
		if let Value::String { val, .. } = self {
			Ok(PathBuf::from(val))
		} else {
			self.cant_convert_to("path")
		}
	}

	/// Returns a reference to the inner [`Record`] value or an error if this `Value` is not a record
	pub fn as_record(&self) -> Result<&Record, ShellError> {
		if let Value::Record { val, .. } = self {
			Ok(val)
		} else {
			self.cant_convert_to("record")
		}
	}

	/// Unwraps the inner [`Record`] value or returns an error if this `Value` is not a record
	pub fn into_record(self) -> Result<Record, ShellError> {
		if let Value::Record { val, .. } = self {
			Ok(val.into_owned())
		} else {
			self.cant_convert_to("record")
		}
	}

	/// Returns a reference to the inner list slice or an error if this `Value` is not a list
	pub fn as_list(&self) -> Result<&[Value], ShellError> {
		if let Value::List { vals, .. } = self {
			Ok(vals)
		} else {
			self.cant_convert_to("list")
		}
	}

	/// Unwraps the inner list `Vec` or returns an error if this `Value` is not a list
	pub fn into_list(self) -> Result<Vec<Value>, ShellError> {
		if let Value::List { vals, .. } = self {
			Ok(vals)
		} else {
			self.cant_convert_to("list")
		}
	}

	/// Returns a reference to the inner [`Closure`] value or an error if this `Value` is not a closure
	pub fn as_closure(&self) -> Result<&Closure, ShellError> {
		if let Value::Closure { val, .. } = self {
			Ok(val)
		} else {
			self.cant_convert_to("closure")
		}
	}

	/// Unwraps the inner [`Closure`] value or returns an error if this `Value` is not a closure
	pub fn into_closure(self) -> Result<Closure, ShellError> {
		if let Value::Closure { val, .. } = self {
			Ok(*val)
		} else {
			self.cant_convert_to("closure")
		}
	}

	/// Returns a reference to the inner binary slice or an error if this `Value` is not a binary value
	pub fn as_binary(&self) -> Result<&[u8], ShellError> {
		if let Value::Binary { val, .. } = self {
			Ok(val)
		} else {
			self.cant_convert_to("binary")
		}
	}

	/// Unwraps the inner binary `Vec` or returns an error if this `Value` is not a binary value
	pub fn into_binary(self) -> Result<Vec<u8>, ShellError> {
		if let Value::Binary { val, .. } = self {
			Ok(val)
		} else {
			self.cant_convert_to("binary")
		}
	}

	/// Returns this `Value` as a `u8` slice or an error if it cannot be converted
	///
	/// Prefer [`coerce_into_binary`](Self::coerce_into_binary)
	/// if you do not need to keep the original `Value` around.
	///
	/// Only the following `Value` cases will return an `Ok` result:
	/// - `Binary`
	/// - `String`
	///
	/// ```
	/// # use xeno_nu_protocol::Value;
	/// for val in Value::test_values() {
	///     assert_eq!(
	///         matches!(val, Value::Binary { .. } | Value::String { .. }),
	///         val.coerce_binary().is_ok(),
	///     );
	/// }
	/// ```
	pub fn coerce_binary(&self) -> Result<&[u8], ShellError> {
		match self {
			Value::Binary { val, .. } => Ok(val),
			Value::String { val, .. } => Ok(val.as_bytes()),
			val => val.cant_convert_to("binary"),
		}
	}

	/// Returns this `Value` as a `Vec<u8>` or an error if it cannot be converted
	///
	/// Only the following `Value` cases will return an `Ok` result:
	/// - `Binary`
	/// - `String`
	///
	/// ```
	/// # use xeno_nu_protocol::Value;
	/// for val in Value::test_values() {
	///     assert_eq!(
	///         matches!(val, Value::Binary { .. } | Value::String { .. }),
	///         val.coerce_into_binary().is_ok(),
	///     );
	/// }
	/// ```
	pub fn coerce_into_binary(self) -> Result<Vec<u8>, ShellError> {
		match self {
			Value::Binary { val, .. } => Ok(val),
			Value::String { val, .. } => Ok(val.into_bytes()),
			val => val.cant_convert_to("binary"),
		}
	}

	/// Returns a reference to the inner [`CellPath`] value or an error if this `Value` is not a cell path
	pub fn as_cell_path(&self) -> Result<&CellPath, ShellError> {
		if let Value::CellPath { val, .. } = self {
			Ok(val)
		} else {
			self.cant_convert_to("cell path")
		}
	}

	/// Unwraps the inner [`CellPath`] value or returns an error if this `Value` is not a cell path
	pub fn into_cell_path(self) -> Result<CellPath, ShellError> {
		if let Value::CellPath { val, .. } = self {
			Ok(val)
		} else {
			self.cant_convert_to("cell path")
		}
	}

	/// Interprets this `Value` as a boolean based on typical conventions for environment values.
	///
	/// The following rules are used:
	/// - Values representing `false`:
	///   - Empty strings or strings that equal to "false" in any case
	///   - The number `0` (as an integer, float or string)
	///   - `Nothing`
	///   - Explicit boolean `false`
	/// - Values representing `true`:
	///   - Non-zero numbers (integer or float)
	///   - Non-empty strings
	///   - Explicit boolean `true`
	///
	/// For all other, more complex variants of [`Value`], the function cannot determine a
	/// boolean representation and returns `Err`.
	pub fn coerce_bool(&self) -> Result<bool, ShellError> {
		match self {
			Value::Bool { val: false, .. } | Value::Int { val: 0, .. } | Value::Nothing { .. } => Ok(false),
			Value::Float { val, .. } if val <= &f64::EPSILON => Ok(false),
			Value::String { val, .. } => match val.trim().to_ascii_lowercase().as_str() {
				"" | "0" | "false" => Ok(false),
				_ => Ok(true),
			},
			Value::Bool { .. } | Value::Int { .. } | Value::Float { .. } => Ok(true),
			_ => self.cant_convert_to("bool"),
		}
	}

	/// Returns a reference to the inner [`CustomValue`] trait object or an error if this `Value` is not a custom value
	pub fn as_custom_value(&self) -> Result<&dyn CustomValue, ShellError> {
		if let Value::Custom { val, .. } = self {
			Ok(val.as_ref())
		} else {
			self.cant_convert_to("custom value")
		}
	}

	/// Unwraps the inner [`CustomValue`] trait object or returns an error if this `Value` is not a custom value
	pub fn into_custom_value(self) -> Result<Box<dyn CustomValue>, ShellError> {
		if let Value::Custom { val, .. } = self {
			Ok(val)
		} else {
			self.cant_convert_to("custom value")
		}
	}

	/// Get the span for the current value
	pub fn span(&self) -> Span {
		match self {
			Value::Bool { internal_span, .. }
			| Value::Int { internal_span, .. }
			| Value::Float { internal_span, .. }
			| Value::Filesize { internal_span, .. }
			| Value::Duration { internal_span, .. }
			| Value::Date { internal_span, .. }
			| Value::Range { internal_span, .. }
			| Value::String { internal_span, .. }
			| Value::Glob { internal_span, .. }
			| Value::Record { internal_span, .. }
			| Value::List { internal_span, .. }
			| Value::Closure { internal_span, .. }
			| Value::Nothing { internal_span, .. }
			| Value::Binary { internal_span, .. }
			| Value::CellPath { internal_span, .. }
			| Value::Custom { internal_span, .. }
			| Value::Error { internal_span, .. } => *internal_span,
		}
	}

	/// Set the value's span to a new span
	pub fn set_span(&mut self, new_span: Span) {
		match self {
			Value::Bool { internal_span, .. }
			| Value::Int { internal_span, .. }
			| Value::Float { internal_span, .. }
			| Value::Filesize { internal_span, .. }
			| Value::Duration { internal_span, .. }
			| Value::Date { internal_span, .. }
			| Value::Range { internal_span, .. }
			| Value::String { internal_span, .. }
			| Value::Glob { internal_span, .. }
			| Value::Record { internal_span, .. }
			| Value::List { internal_span, .. }
			| Value::Closure { internal_span, .. }
			| Value::Nothing { internal_span, .. }
			| Value::Binary { internal_span, .. }
			| Value::CellPath { internal_span, .. }
			| Value::Custom { internal_span, .. } => *internal_span = new_span,
			Value::Error { .. } => (),
		}
	}

	/// Update the value with a new span
	pub fn with_span(mut self, new_span: Span) -> Value {
		self.set_span(new_span);
		self
	}

	/// Get the type of the current Value
	pub fn get_type(&self) -> Type {
		match self {
			Value::Bool { .. } => Type::Bool,
			Value::Int { .. } => Type::Int,
			Value::Float { .. } => Type::Float,
			Value::Filesize { .. } => Type::Filesize,
			Value::Duration { .. } => Type::Duration,
			Value::Date { .. } => Type::Date,
			Value::Range { .. } => Type::Range,
			Value::String { .. } => Type::String,
			Value::Glob { .. } => Type::Glob,
			Value::Record { val, .. } => Type::Record(val.iter().map(|(x, y)| (x.clone(), y.get_type())).collect()),
			Value::List { vals, .. } => {
				let ty = Type::supertype_of(vals.iter().map(Value::get_type)).unwrap_or(Type::Any);

				match ty {
					Type::Record(columns) => Type::Table(columns),
					ty => Type::list(ty),
				}
			}
			Value::Nothing { .. } => Type::Nothing,
			Value::Closure { .. } => Type::Closure,
			Value::Error { .. } => Type::Error,
			Value::Binary { .. } => Type::Binary,
			Value::CellPath { .. } => Type::CellPath,
			Value::Custom { val, .. } => Type::Custom(val.type_name().into()),
		}
	}

	/// Determine of the [`Value`] is a [subtype](https://en.wikipedia.org/wiki/Subtyping) of `other`
	///
	/// If you have a [`Value`], this method should always be used over chaining [`Value::get_type`] with [`Type::is_subtype_of`](crate::Type::is_subtype_of).
	///
	/// This method is able to leverage that information encoded in a `Value` to provide more accurate
	/// type comparison than if one were to collect the type into [`Type`](crate::Type) value with [`Value::get_type`].
	///
	/// Empty lists are considered subtypes of all `list<T>` types.
	///
	/// Lists of mixed records where some column is present in all record is a subtype of `table<column>`.
	/// For example, `[{a: 1, b: 2}, {a: 1}]` is a subtype of `table<a: int>` (but not `table<a: int, b: int>`).
	///
	/// See also: [`PipelineData::is_subtype_of`](crate::PipelineData::is_subtype_of)
	pub fn is_subtype_of(&self, other: &Type) -> bool {
		// records are structurally typed
		let record_compatible = |val: &Value, other: &[(String, Type)]| match val {
			Value::Record { val, .. } => other.iter().all(|(key, ty)| val.get(key).is_some_and(|inner| inner.is_subtype_of(ty))),
			_ => false,
		};

		// All cases matched explicitly to ensure this does not accidentally allocate `Type` if any composite types are introduced in the future
		match (self, other) {
			(_, Type::Any) => true,
			(val, Type::OneOf(types)) => types.iter().any(|t| val.is_subtype_of(t)),

			// `Type` allocation for scalar types is trivial
			(
				Value::Bool { .. }
				| Value::Int { .. }
				| Value::Float { .. }
				| Value::String { .. }
				| Value::Glob { .. }
				| Value::Filesize { .. }
				| Value::Duration { .. }
				| Value::Date { .. }
				| Value::Range { .. }
				| Value::Closure { .. }
				| Value::Error { .. }
				| Value::Binary { .. }
				| Value::CellPath { .. }
				| Value::Nothing { .. },
				_,
			) => self.get_type().is_subtype_of(other),

			// matching composite types
			(val @ Value::Record { .. }, Type::Record(inner)) => record_compatible(val, inner),
			(Value::List { vals, .. }, Type::List(inner)) => vals.iter().all(|val| val.is_subtype_of(inner)),
			(Value::List { vals, .. }, Type::Table(inner)) => vals.iter().all(|val| record_compatible(val, inner)),
			(Value::Custom { val, .. }, Type::Custom(inner)) => val.type_name() == **inner,

			// non-matching composite types
			(Value::Record { .. } | Value::List { .. } | Value::Custom { .. }, _) => false,
		}
	}

	pub fn get_data_by_key(&self, name: &str) -> Option<Value> {
		let span = self.span();
		match self {
			Value::Record { val, .. } => val.get(name).cloned(),
			Value::List { vals, .. } => {
				let out = vals
					.iter()
					.map(|item| item.as_record().ok().and_then(|val| val.get(name).cloned()).unwrap_or(Value::nothing(span)))
					.collect::<Vec<_>>();

				if !out.is_empty() { Some(Value::list(out, span)) } else { None }
			}
			_ => None,
		}
	}

	fn format_datetime<Tz: TimeZone>(&self, date_time: &DateTime<Tz>, formatter: &str) -> String
	where
		Tz::Offset: Display,
	{
		let mut formatter_buf = String::new();
		let locale = if let Ok(l) = std::env::var(LOCALE_OVERRIDE_ENV_VAR).or_else(|_| std::env::var("LC_TIME")) {
			let locale_str = l.split('.').next().unwrap_or("en_US");
			locale_str.try_into().unwrap_or(Locale::en_US)
		} else {
			// LC_ALL > LC_CTYPE > LANG else en_US
			get_system_locale_string()
				.map(|l| l.replace('-', "_")) // `chrono::Locale` needs something like `xx_xx`, rather than `xx-xx`
				.unwrap_or_else(|| String::from("en_US"))
				.as_str()
				.try_into()
				.unwrap_or(Locale::en_US)
		};
		let format = date_time.format_localized(formatter, locale);

		match formatter_buf.write_fmt(format_args!("{format}")) {
			Ok(_) => (),
			Err(_) => formatter_buf = format!("Invalid format string {formatter}"),
		}
		formatter_buf
	}

	/// Converts this `Value` to a string according to the given [`Config`] and separator
	///
	/// This functions recurses into records and lists,
	/// returning a string that contains the stringified form of all nested `Value`s.
	pub fn to_expanded_string(&self, separator: &str, config: &Config) -> String {
		let span = self.span();
		match self {
			Value::Bool { val, .. } => val.to_string(),
			Value::Int { val, .. } => val.to_string(),
			Value::Float { val, .. } => ObviousFloat(*val).to_string(),
			Value::Filesize { val, .. } => config.filesize.format(*val).to_string(),
			Value::Duration { val, .. } => format_duration(*val),
			Value::Date { val, .. } => match &config.datetime_format.normal {
				Some(format) => self.format_datetime(val, format),
				None => {
					format!(
						"{} ({})",
						if val.year() >= 0 { val.to_rfc2822() } else { val.to_rfc3339() },
						human_time_from_now(val),
					)
				}
			},
			Value::Range { val, .. } => val.to_string(),
			Value::String { val, .. } => val.clone(),
			Value::Glob { val, .. } => val.clone(),
			Value::List { vals: val, .. } => format!(
				"[{}]",
				val.iter().map(|x| x.to_expanded_string(", ", config)).collect::<Vec<_>>().join(separator)
			),
			Value::Record { val, .. } => format!(
				"{{{}}}",
				val.iter()
					.map(|(x, y)| format!("{}: {}", x, y.to_expanded_string(", ", config)))
					.collect::<Vec<_>>()
					.join(separator)
			),
			Value::Closure { val, .. } => format!("closure_{}", val.block_id.get()),
			Value::Nothing { .. } => String::new(),
			Value::Error { error, .. } => format!("{error:?}"),
			Value::Binary { val, .. } => format!("{val:?}"),
			Value::CellPath { val, .. } => val.to_string(),
			// If we fail to collapse the custom value, just print <{type_name}> - failure is not
			// that critical here
			Value::Custom { val, .. } => val
				.to_base_value(span)
				.map(|val| val.to_expanded_string(separator, config))
				.unwrap_or_else(|_| format!("<{}>", val.type_name())),
		}
	}

	/// Converts this `Value` to a string according to the given [`Config`]
	///
	/// This functions does not recurse into records and lists.
	/// Instead, it will shorten the first list or record it finds like so:
	/// - "[table {n} rows]"
	/// - "[list {n} items]"
	/// - "[record {n} fields]"
	pub fn to_abbreviated_string(&self, config: &Config) -> String {
		match self {
			Value::Date { val, .. } => match &config.datetime_format.table {
				Some(format) => self.format_datetime(val, format),
				None => human_time_from_now(val).to_string(),
			},
			Value::List { vals, .. } => {
				if !vals.is_empty() && vals.iter().all(|x| matches!(x, Value::Record { .. })) {
					format!("[table {} row{}]", vals.len(), if vals.len() == 1 { "" } else { "s" })
				} else {
					format!("[list {} item{}]", vals.len(), if vals.len() == 1 { "" } else { "s" })
				}
			}
			Value::Record { val, .. } => format!("{{record {} field{}}}", val.len(), if val.len() == 1 { "" } else { "s" }),
			val => val.to_expanded_string(", ", config),
		}
	}

	/// Converts this `Value` to a string according to the given [`Config`] and separator
	///
	/// This function adds quotes around strings,
	/// so that the returned string can be parsed by nushell.
	/// The other `Value` cases are already parsable when converted strings
	/// or are not yet handled by this function.
	///
	/// This functions behaves like [`to_expanded_string`](Self::to_expanded_string)
	/// and will recurse into records and lists.
	pub fn to_parsable_string(&self, separator: &str, config: &Config) -> String {
		match self {
			// give special treatment to the simple types to make them parsable
			Value::String { val, .. } => format!("'{val}'"),
			// recurse back into this function for recursive formatting
			Value::List { vals: val, .. } => format!(
				"[{}]",
				val.iter().map(|x| x.to_parsable_string(", ", config)).collect::<Vec<_>>().join(separator)
			),
			Value::Record { val, .. } => format!(
				"{{{}}}",
				val.iter()
					.map(|(x, y)| format!("{}: {}", x, y.to_parsable_string(", ", config)))
					.collect::<Vec<_>>()
					.join(separator)
			),
			// defer to standard handling for types where standard representation is parsable
			_ => self.to_expanded_string(separator, config),
		}
	}

	/// Convert this `Value` to a debug string
	///
	/// In general, this function should only be used for debug purposes,
	/// and the resulting string should not be displayed to the user (not even in an error).
	pub fn to_debug_string(&self) -> String {
		match self {
			Value::String { val, .. } => {
				if contains_emoji(val) {
					// This has to be an emoji, so let's display the code points that make it up.
					format!("{:#?}", Value::string(val.escape_unicode().to_string(), self.span()))
				} else {
					format!("{self:#?}")
				}
			}
			_ => format!("{self:#?}"),
		}
	}

	/// Follow a given cell path into the value: for example accessing select elements in a stream or list
	pub fn follow_cell_path<'out>(&'out self, cell_path: &[PathMember]) -> Result<Cow<'out, Value>, ShellError> {
		// A dummy value is required, otherwise rust doesn't allow references, which we need for
		// the `std::ptr::eq` comparison
		let mut store: Value = Value::test_nothing();
		let mut current: MultiLife<'out, '_, Value> = MultiLife::Out(self);

		let reorder_cell_paths = xeno_nu_experimental::REORDER_CELL_PATHS.get();

		let mut members: Vec<_> = if reorder_cell_paths {
			cell_path.iter().map(Some).collect()
		} else {
			Vec::new()
		};
		let mut members = members.as_mut_slice();
		let mut cell_path = cell_path;

		loop {
			let member = if reorder_cell_paths {
				// Skip any None values at the start.
				while let Some(None) = members.first() {
					members = &mut members[1..];
				}

				if members.is_empty() {
					break;
				}

				// Reorder cell-path member access by prioritizing Int members to avoid cloning unless
				// necessary
				let member = if let Value::List { .. } = &*current {
					// If the value is a list, try to find an Int member
					members
						.iter_mut()
						.find(|x| matches!(x, Some(PathMember::Int { .. })))
						// And take it from the list of members
						.and_then(Option::take)
				} else {
					None
				};

				let Some(member) = member.or_else(|| members.first_mut().and_then(Option::take)) else {
					break;
				};
				member
			} else {
				match cell_path {
					[first, rest @ ..] => {
						cell_path = rest;
						first
					}
					_ => break,
				}
			};

			current = match current {
				MultiLife::Out(current) => match get_value_member(current, member)? {
					ControlFlow::Break(span) => return Ok(Cow::Owned(Value::nothing(span))),
					ControlFlow::Continue(x) => match x {
						Cow::Borrowed(x) => MultiLife::Out(x),
						Cow::Owned(x) => {
							store = x;
							MultiLife::Local(&store)
						}
					},
				},
				MultiLife::Local(current) => match get_value_member(current, member)? {
					ControlFlow::Break(span) => return Ok(Cow::Owned(Value::nothing(span))),
					ControlFlow::Continue(x) => match x {
						Cow::Borrowed(x) => MultiLife::Local(x),
						Cow::Owned(x) => {
							store = x;
							MultiLife::Local(&store)
						}
					},
				},
			};
		}

		// If a single Value::Error was produced by the above (which won't happen if nullify_errors is true), unwrap it now.
		// Note that Value::Errors inside Lists remain as they are, so that the rest of the list can still potentially be used.
		if let Value::Error { error, .. } = &*current {
			Err(error.as_ref().clone())
		} else {
			Ok(match current {
				MultiLife::Out(x) => Cow::Borrowed(x),
				MultiLife::Local(x) => {
					let x = if std::ptr::eq(x, &store) { store } else { x.clone() };
					Cow::Owned(x)
				}
			})
		}
	}

	/// Follow a given cell path into the value: for example accessing select elements in a stream or list
	pub fn upsert_cell_path(&mut self, cell_path: &[PathMember], callback: Box<dyn FnOnce(&Value) -> Value>) -> Result<(), ShellError> {
		let new_val = callback(self.follow_cell_path(cell_path)?.as_ref());

		match new_val {
			Value::Error { error, .. } => Err(*error),
			new_val => self.upsert_data_at_cell_path(cell_path, new_val),
		}
	}

	pub fn upsert_data_at_cell_path(&mut self, cell_path: &[PathMember], new_val: Value) -> Result<(), ShellError> {
		let v_span = self.span();
		if let Some((member, path)) = cell_path.split_first() {
			match member {
				PathMember::String {
					val: col_name, span, casing, ..
				} => match self {
					Value::List { vals, .. } => {
						for val in vals.iter_mut() {
							match val {
								Value::Record { val: record, .. } => {
									let record = record.to_mut();
									if let Some(val) = record.cased_mut(*casing).get_mut(col_name) {
										val.upsert_data_at_cell_path(path, new_val.clone())?;
									} else {
										let new_col = Value::with_data_at_cell_path(path, new_val.clone())?;
										record.push(col_name, new_col);
									}
								}
								Value::Error { error, .. } => return Err(*error.clone()),
								v => {
									return Err(ShellError::CantFindColumn {
										col_name: col_name.clone(),
										span: Some(*span),
										src_span: v.span(),
									});
								}
							}
						}
					}
					Value::Record { val: record, .. } => {
						let record = record.to_mut();
						if let Some(val) = record.cased_mut(*casing).get_mut(col_name) {
							val.upsert_data_at_cell_path(path, new_val)?;
						} else {
							let new_col = Value::with_data_at_cell_path(path, new_val.clone())?;
							record.push(col_name, new_col);
						}
					}
					Value::Error { error, .. } => return Err(*error.clone()),
					v => {
						return Err(ShellError::CantFindColumn {
							col_name: col_name.clone(),
							span: Some(*span),
							src_span: v.span(),
						});
					}
				},
				PathMember::Int { val: row_num, span, .. } => match self {
					Value::List { vals, .. } => {
						if let Some(v) = vals.get_mut(*row_num) {
							v.upsert_data_at_cell_path(path, new_val)?;
						} else if vals.len() != *row_num {
							return Err(ShellError::InsertAfterNextFreeIndex {
								available_idx: vals.len(),
								span: *span,
							});
						} else {
							// If the upsert is at 1 + the end of the list, it's OK.
							vals.push(Value::with_data_at_cell_path(path, new_val)?);
						}
					}
					Value::Error { error, .. } => return Err(*error.clone()),
					_ => {
						return Err(ShellError::NotAList {
							dst_span: *span,
							src_span: v_span,
						});
					}
				},
			}
		} else {
			*self = new_val;
		}
		Ok(())
	}

	/// Follow a given cell path into the value: for example accessing select elements in a stream or list
	pub fn update_cell_path<'a>(&mut self, cell_path: &[PathMember], callback: Box<dyn FnOnce(&Value) -> Value + 'a>) -> Result<(), ShellError> {
		let new_val = callback(self.follow_cell_path(cell_path)?.as_ref());

		match new_val {
			Value::Error { error, .. } => Err(*error),
			new_val => self.update_data_at_cell_path(cell_path, new_val),
		}
	}

	pub fn update_data_at_cell_path(&mut self, cell_path: &[PathMember], new_val: Value) -> Result<(), ShellError> {
		let v_span = self.span();
		if let Some((member, path)) = cell_path.split_first() {
			match member {
				PathMember::String {
					val: col_name,
					span,
					casing,
					optional,
				} => match self {
					Value::List { vals, .. } => {
						for val in vals.iter_mut() {
							let v_span = val.span();
							match val {
								Value::Record { val: record, .. } => {
									if let Some(val) = record.to_mut().cased_mut(*casing).get_mut(col_name) {
										val.update_data_at_cell_path(path, new_val.clone())?;
									} else if !*optional {
										return Err(ShellError::CantFindColumn {
											col_name: col_name.clone(),
											span: Some(*span),
											src_span: v_span,
										});
									}
								}
								Value::Error { error, .. } => return Err(*error.clone()),
								v => {
									if !*optional {
										return Err(ShellError::CantFindColumn {
											col_name: col_name.clone(),
											span: Some(*span),
											src_span: v.span(),
										});
									}
								}
							}
						}
					}
					Value::Record { val: record, .. } => {
						if let Some(val) = record.to_mut().cased_mut(*casing).get_mut(col_name) {
							val.update_data_at_cell_path(path, new_val)?;
						} else if !*optional {
							return Err(ShellError::CantFindColumn {
								col_name: col_name.clone(),
								span: Some(*span),
								src_span: v_span,
							});
						}
					}
					Value::Error { error, .. } => return Err(*error.clone()),
					v => {
						if !*optional {
							return Err(ShellError::CantFindColumn {
								col_name: col_name.clone(),
								span: Some(*span),
								src_span: v.span(),
							});
						}
					}
				},
				PathMember::Int { val: row_num, span, optional } => match self {
					Value::List { vals, .. } => {
						if let Some(v) = vals.get_mut(*row_num) {
							v.update_data_at_cell_path(path, new_val)?;
						} else if !*optional {
							if vals.is_empty() {
								return Err(ShellError::AccessEmptyContent { span: *span });
							} else {
								return Err(ShellError::AccessBeyondEnd {
									max_idx: vals.len() - 1,
									span: *span,
								});
							}
						}
					}
					Value::Error { error, .. } => return Err(*error.clone()),
					v => {
						return Err(ShellError::NotAList {
							dst_span: *span,
							src_span: v.span(),
						});
					}
				},
			}
		} else {
			*self = new_val;
		}
		Ok(())
	}

	pub fn remove_data_at_cell_path(&mut self, cell_path: &[PathMember]) -> Result<(), ShellError> {
		match cell_path {
			[] => Ok(()),
			[member] => {
				let v_span = self.span();
				match member {
					PathMember::String {
						val: col_name,
						span,
						optional,
						casing,
					} => match self {
						Value::List { vals, .. } => {
							for val in vals.iter_mut() {
								let v_span = val.span();
								match val {
									Value::Record { val: record, .. } => {
										let value = record.to_mut().cased_mut(*casing).remove(col_name);
										if value.is_none() && !optional {
											return Err(ShellError::CantFindColumn {
												col_name: col_name.clone(),
												span: Some(*span),
												src_span: v_span,
											});
										}
									}
									v => {
										return Err(ShellError::CantFindColumn {
											col_name: col_name.clone(),
											span: Some(*span),
											src_span: v.span(),
										});
									}
								}
							}
							Ok(())
						}
						Value::Record { val: record, .. } => {
							if record.to_mut().cased_mut(*casing).remove(col_name).is_none() && !optional {
								return Err(ShellError::CantFindColumn {
									col_name: col_name.clone(),
									span: Some(*span),
									src_span: v_span,
								});
							}
							Ok(())
						}
						v => Err(ShellError::CantFindColumn {
							col_name: col_name.clone(),
							span: Some(*span),
							src_span: v.span(),
						}),
					},
					PathMember::Int { val: row_num, span, optional } => match self {
						Value::List { vals, .. } => {
							if vals.get_mut(*row_num).is_some() {
								vals.remove(*row_num);
								Ok(())
							} else if *optional {
								Ok(())
							} else if vals.is_empty() {
								Err(ShellError::AccessEmptyContent { span: *span })
							} else {
								Err(ShellError::AccessBeyondEnd {
									max_idx: vals.len() - 1,
									span: *span,
								})
							}
						}
						v => Err(ShellError::NotAList {
							dst_span: *span,
							src_span: v.span(),
						}),
					},
				}
			}
			[member, path @ ..] => {
				let v_span = self.span();
				match member {
					PathMember::String {
						val: col_name,
						span,
						optional,
						casing,
					} => match self {
						Value::List { vals, .. } => {
							for val in vals.iter_mut() {
								let v_span = val.span();
								match val {
									Value::Record { val: record, .. } => {
										let val = record.to_mut().cased_mut(*casing).get_mut(col_name);
										if let Some(val) = val {
											val.remove_data_at_cell_path(path)?;
										} else if !optional {
											return Err(ShellError::CantFindColumn {
												col_name: col_name.clone(),
												span: Some(*span),
												src_span: v_span,
											});
										}
									}
									v => {
										return Err(ShellError::CantFindColumn {
											col_name: col_name.clone(),
											span: Some(*span),
											src_span: v.span(),
										});
									}
								}
							}
							Ok(())
						}
						Value::Record { val: record, .. } => {
							if let Some(val) = record.to_mut().cased_mut(*casing).get_mut(col_name) {
								val.remove_data_at_cell_path(path)?;
							} else if !optional {
								return Err(ShellError::CantFindColumn {
									col_name: col_name.clone(),
									span: Some(*span),
									src_span: v_span,
								});
							}
							Ok(())
						}
						v => Err(ShellError::CantFindColumn {
							col_name: col_name.clone(),
							span: Some(*span),
							src_span: v.span(),
						}),
					},
					PathMember::Int { val: row_num, span, optional } => match self {
						Value::List { vals, .. } => {
							if let Some(v) = vals.get_mut(*row_num) {
								v.remove_data_at_cell_path(path)
							} else if *optional {
								Ok(())
							} else if vals.is_empty() {
								Err(ShellError::AccessEmptyContent { span: *span })
							} else {
								Err(ShellError::AccessBeyondEnd {
									max_idx: vals.len() - 1,
									span: *span,
								})
							}
						}
						v => Err(ShellError::NotAList {
							dst_span: *span,
							src_span: v.span(),
						}),
					},
				}
			}
		}
	}
	pub fn insert_data_at_cell_path(&mut self, cell_path: &[PathMember], new_val: Value, head_span: Span) -> Result<(), ShellError> {
		let v_span = self.span();
		if let Some((member, path)) = cell_path.split_first() {
			match member {
				PathMember::String {
					val: col_name, span, casing, ..
				} => match self {
					Value::List { vals, .. } => {
						for val in vals.iter_mut() {
							let v_span = val.span();
							match val {
								Value::Record { val: record, .. } => {
									let record = record.to_mut();
									if let Some(val) = record.cased_mut(*casing).get_mut(col_name) {
										if path.is_empty() {
											return Err(ShellError::ColumnAlreadyExists {
												col_name: col_name.clone(),
												span: *span,
												src_span: v_span,
											});
										} else {
											val.insert_data_at_cell_path(path, new_val.clone(), head_span)?;
										}
									} else {
										let new_col = Value::with_data_at_cell_path(path, new_val.clone())?;
										record.push(col_name, new_col);
									}
								}
								Value::Error { error, .. } => return Err(*error.clone()),
								_ => {
									return Err(ShellError::UnsupportedInput {
										msg: "expected table or record".into(),
										input: format!("input type: {:?}", val.get_type()),
										msg_span: head_span,
										input_span: *span,
									});
								}
							}
						}
					}
					Value::Record { val: record, .. } => {
						let record = record.to_mut();
						if let Some(val) = record.cased_mut(*casing).get_mut(col_name) {
							if path.is_empty() {
								return Err(ShellError::ColumnAlreadyExists {
									col_name: col_name.clone(),
									span: *span,
									src_span: v_span,
								});
							} else {
								val.insert_data_at_cell_path(path, new_val, head_span)?;
							}
						} else {
							let new_col = Value::with_data_at_cell_path(path, new_val)?;
							record.push(col_name, new_col);
						}
					}
					other => {
						return Err(ShellError::UnsupportedInput {
							msg: "table or record".into(),
							input: format!("input type: {:?}", other.get_type()),
							msg_span: head_span,
							input_span: *span,
						});
					}
				},
				PathMember::Int { val: row_num, span, .. } => match self {
					Value::List { vals, .. } => {
						if let Some(v) = vals.get_mut(*row_num) {
							if path.is_empty() {
								vals.insert(*row_num, new_val);
							} else {
								v.insert_data_at_cell_path(path, new_val, head_span)?;
							}
						} else if vals.len() != *row_num {
							return Err(ShellError::InsertAfterNextFreeIndex {
								available_idx: vals.len(),
								span: *span,
							});
						} else {
							// If the insert is at 1 + the end of the list, it's OK.
							vals.push(Value::with_data_at_cell_path(path, new_val)?);
						}
					}
					_ => {
						return Err(ShellError::NotAList {
							dst_span: *span,
							src_span: v_span,
						});
					}
				},
			}
		} else {
			*self = new_val;
		}
		Ok(())
	}

	/// Creates a new [Value] with the specified member at the specified path.
	/// This is used by [Value::insert_data_at_cell_path] and [Value::upsert_data_at_cell_path] whenever they have the need to insert a non-existent element
	fn with_data_at_cell_path(cell_path: &[PathMember], value: Value) -> Result<Value, ShellError> {
		if let Some((member, path)) = cell_path.split_first() {
			let span = value.span();
			match member {
				PathMember::String { val, .. } => Ok(Value::record(
					std::iter::once((val.clone(), Value::with_data_at_cell_path(path, value)?)).collect(),
					span,
				)),
				PathMember::Int { val, .. } => {
					if *val == 0usize {
						Ok(Value::list(vec![Value::with_data_at_cell_path(path, value)?], span))
					} else {
						Err(ShellError::InsertAfterNextFreeIndex { available_idx: 0, span })
					}
				}
			}
		} else {
			Ok(value)
		}
	}

	/// Visits all values contained within the value (including this value) with a mutable reference
	/// given to the closure.
	///
	/// If the closure returns `Err`, the traversal will stop.
	///
	/// Captures of closure values are currently visited, as they are values owned by the closure.
	pub fn recurse_mut<E>(&mut self, f: &mut impl FnMut(&mut Value) -> Result<(), E>) -> Result<(), E> {
		// Visit this value
		f(self)?;
		// Check for contained values
		match self {
			Value::Record { val, .. } => val.to_mut().iter_mut().try_for_each(|(_, rec_value)| rec_value.recurse_mut(f)),
			Value::List { vals, .. } => vals.iter_mut().try_for_each(|list_value| list_value.recurse_mut(f)),
			// Closure captures are visited. Maybe these don't have to be if they are changed to
			// more opaque references.
			Value::Closure { val, .. } => val
				.captures
				.iter_mut()
				.map(|(_, captured_value)| captured_value)
				.try_for_each(|captured_value| captured_value.recurse_mut(f)),
			// All of these don't contain other values
			Value::Bool { .. }
			| Value::Int { .. }
			| Value::Float { .. }
			| Value::Filesize { .. }
			| Value::Duration { .. }
			| Value::Date { .. }
			| Value::Range { .. }
			| Value::String { .. }
			| Value::Glob { .. }
			| Value::Nothing { .. }
			| Value::Error { .. }
			| Value::Binary { .. }
			| Value::CellPath { .. } => Ok(()),
			// These could potentially contain values, but we expect the closure to handle them
			Value::Custom { .. } => Ok(()),
		}
	}

	/// Check if the content is empty
	pub fn is_empty(&self) -> bool {
		match self {
			Value::String { val, .. } => val.is_empty(),
			Value::List { vals, .. } => vals.is_empty(),
			Value::Record { val, .. } => val.is_empty(),
			Value::Binary { val, .. } => val.is_empty(),
			Value::Nothing { .. } => true,
			_ => false,
		}
	}

	pub fn is_nothing(&self) -> bool {
		matches!(self, Value::Nothing { .. })
	}

	pub fn is_error(&self) -> bool {
		matches!(self, Value::Error { .. })
	}

	/// Extract [ShellError] from [Value::Error]
	pub fn unwrap_error(self) -> Result<Self, ShellError> {
		match self {
			Self::Error { error, .. } => Err(*error),
			val => Ok(val),
		}
	}

	pub fn is_true(&self) -> bool {
		matches!(self, Value::Bool { val: true, .. })
	}

	pub fn is_false(&self) -> bool {
		matches!(self, Value::Bool { val: false, .. })
	}

	pub fn columns(&self) -> impl Iterator<Item = &String> {
		let opt = match self {
			Value::Record { val, .. } => Some(val.columns()),
			_ => None,
		};

		opt.into_iter().flatten()
	}

	/// Returns an estimate of the memory size used by this Value in bytes
	pub fn memory_size(&self) -> usize {
		match self {
			Value::Bool { .. } => std::mem::size_of::<Self>(),
			Value::Int { .. } => std::mem::size_of::<Self>(),
			Value::Float { .. } => std::mem::size_of::<Self>(),
			Value::Filesize { .. } => std::mem::size_of::<Self>(),
			Value::Duration { .. } => std::mem::size_of::<Self>(),
			Value::Date { .. } => std::mem::size_of::<Self>(),
			Value::Range { val, .. } => std::mem::size_of::<Self>() + val.memory_size(),
			Value::String { val, .. } => std::mem::size_of::<Self>() + val.capacity(),
			Value::Glob { val, .. } => std::mem::size_of::<Self>() + val.capacity(),
			Value::Record { val, .. } => std::mem::size_of::<Self>() + val.memory_size(),
			Value::List { vals, .. } => std::mem::size_of::<Self>() + vals.iter().map(|v| v.memory_size()).sum::<usize>(),
			Value::Closure { val, .. } => std::mem::size_of::<Self>() + val.memory_size(),
			Value::Nothing { .. } => std::mem::size_of::<Self>(),
			Value::Error { error, .. } => std::mem::size_of::<Self>() + std::mem::size_of_val(error),
			Value::Binary { val, .. } => std::mem::size_of::<Self>() + val.capacity(),
			Value::CellPath { val, .. } => std::mem::size_of::<Self>() + val.memory_size(),
			Value::Custom { val, .. } => std::mem::size_of::<Self>() + val.memory_size(),
		}
	}

	pub fn bool(val: bool, span: Span) -> Value {
		Value::Bool { val, internal_span: span }
	}

	pub fn int(val: i64, span: Span) -> Value {
		Value::Int { val, internal_span: span }
	}

	pub fn float(val: f64, span: Span) -> Value {
		Value::Float { val, internal_span: span }
	}

	pub fn filesize(val: impl Into<Filesize>, span: Span) -> Value {
		Value::Filesize {
			val: val.into(),
			internal_span: span,
		}
	}

	pub fn duration(val: i64, span: Span) -> Value {
		Value::Duration { val, internal_span: span }
	}

	pub fn date(val: DateTime<FixedOffset>, span: Span) -> Value {
		Value::Date { val, internal_span: span }
	}

	pub fn range(val: Range, span: Span) -> Value {
		Value::Range {
			val: val.into(),
			signals: None,
			internal_span: span,
		}
	}

	pub fn string(val: impl Into<String>, span: Span) -> Value {
		Value::String {
			val: val.into(),
			internal_span: span,
		}
	}

	pub fn glob(val: impl Into<String>, no_expand: bool, span: Span) -> Value {
		Value::Glob {
			val: val.into(),
			no_expand,
			internal_span: span,
		}
	}

	pub fn record(val: Record, span: Span) -> Value {
		Value::Record {
			val: SharedCow::new(val),
			internal_span: span,
		}
	}

	pub fn list(vals: Vec<Value>, span: Span) -> Value {
		Value::List {
			vals,
			signals: None,
			internal_span: span,
		}
	}

	pub fn closure(val: Closure, span: Span) -> Value {
		Value::Closure {
			val: val.into(),
			internal_span: span,
		}
	}

	/// Create a new `Nothing` value
	pub fn nothing(span: Span) -> Value {
		Value::Nothing { internal_span: span }
	}

	pub fn error(error: ShellError, span: Span) -> Value {
		Value::Error {
			error: Box::new(error),
			internal_span: span,
		}
	}

	pub fn binary(val: impl Into<Vec<u8>>, span: Span) -> Value {
		Value::Binary {
			val: val.into(),
			internal_span: span,
		}
	}

	pub fn cell_path(val: CellPath, span: Span) -> Value {
		Value::CellPath { val, internal_span: span }
	}

	pub fn custom(val: Box<dyn CustomValue>, span: Span) -> Value {
		Value::Custom { val, internal_span: span }
	}

	/// Note: Only use this for test data, *not* live data, as it will point into unknown source
	/// when used in errors.
	pub fn test_bool(val: bool) -> Value {
		Value::bool(val, Span::test_data())
	}

	/// Note: Only use this for test data, *not* live data, as it will point into unknown source
	/// when used in errors.
	pub fn test_int(val: i64) -> Value {
		Value::int(val, Span::test_data())
	}

	/// Note: Only use this for test data, *not* live data, as it will point into unknown source
	/// when used in errors.
	pub fn test_float(val: f64) -> Value {
		Value::float(val, Span::test_data())
	}

	/// Note: Only use this for test data, *not* live data, as it will point into unknown source
	/// when used in errors.
	pub fn test_filesize(val: impl Into<Filesize>) -> Value {
		Value::filesize(val, Span::test_data())
	}

	/// Note: Only use this for test data, *not* live data, as it will point into unknown source
	/// when used in errors.
	pub fn test_duration(val: i64) -> Value {
		Value::duration(val, Span::test_data())
	}

	/// Note: Only use this for test data, *not* live data, as it will point into unknown source
	/// when used in errors.
	pub fn test_date(val: DateTime<FixedOffset>) -> Value {
		Value::date(val, Span::test_data())
	}

	/// Note: Only use this for test data, *not* live data, as it will point into unknown source
	/// when used in errors.
	pub fn test_range(val: Range) -> Value {
		Value::range(val, Span::test_data())
	}

	/// Note: Only use this for test data, *not* live data, as it will point into unknown source
	/// when used in errors.
	pub fn test_string(val: impl Into<String>) -> Value {
		Value::string(val, Span::test_data())
	}

	/// Note: Only use this for test data, *not* live data, as it will point into unknown source
	/// when used in errors.
	pub fn test_glob(val: impl Into<String>) -> Value {
		Value::glob(val, false, Span::test_data())
	}

	/// Note: Only use this for test data, *not* live data, as it will point into unknown source
	/// when used in errors.
	pub fn test_record(val: Record) -> Value {
		Value::record(val, Span::test_data())
	}

	/// Note: Only use this for test data, *not* live data, as it will point into unknown source
	/// when used in errors.
	pub fn test_list(vals: Vec<Value>) -> Value {
		Value::list(vals, Span::test_data())
	}

	/// Note: Only use this for test data, *not* live data, as it will point into unknown source
	/// when used in errors.
	pub fn test_closure(val: Closure) -> Value {
		Value::closure(val, Span::test_data())
	}

	/// Note: Only use this for test data, *not* live data, as it will point into unknown source
	/// when used in errors.
	pub fn test_nothing() -> Value {
		Value::nothing(Span::test_data())
	}

	/// Note: Only use this for test data, *not* live data, as it will point into unknown source
	/// when used in errors.
	pub fn test_binary(val: impl Into<Vec<u8>>) -> Value {
		Value::binary(val, Span::test_data())
	}

	/// Note: Only use this for test data, *not* live data, as it will point into unknown source
	/// when used in errors.
	pub fn test_cell_path(val: CellPath) -> Value {
		Value::cell_path(val, Span::test_data())
	}

	/// Note: Only use this for test data, *not* live data, as it will point into unknown source
	/// when used in errors.
	pub fn test_custom_value(val: Box<dyn CustomValue>) -> Value {
		Value::custom(val, Span::test_data())
	}

	/// Note: Only use this for test data, *not* live data,
	/// as it will point into unknown source when used in errors.
	///
	/// Returns a `Vec` containing one of each value case (`Value::Int`, `Value::String`, etc.)
	/// except for `Value::Custom`.
	pub fn test_values() -> Vec<Value> {
		vec![
			Value::test_bool(false),
			Value::test_int(0),
			Value::test_filesize(0),
			Value::test_duration(0),
			Value::test_date(DateTime::UNIX_EPOCH.into()),
			Value::test_range(Range::IntRange(IntRange {
				start: 0,
				step: 1,
				end: Bound::Excluded(0),
			})),
			Value::test_float(0.0),
			Value::test_string(String::new()),
			Value::test_record(Record::new()),
			Value::test_list(Vec::new()),
			Value::test_closure(Closure {
				block_id: BlockId::new(0),
				captures: Vec::new(),
			}),
			Value::test_nothing(),
			Value::error(ShellError::NushellFailed { msg: String::new() }, Span::test_data()),
			Value::test_binary(Vec::new()),
			Value::test_cell_path(CellPath { members: Vec::new() }),
			// Value::test_custom_value(Box::new(todo!())),
		]
	}

	/// inject signals from engine_state so iterating the value
	/// itself can be interrupted.
	pub fn inject_signals(&mut self, engine_state: &EngineState) {
		match self {
			Value::List { signals: s, .. } | Value::Range { signals: s, .. } => {
				*s = Some(engine_state.signals().clone());
			}
			_ => (),
		}
	}
}

fn get_value_member<'a>(current: &'a Value, member: &PathMember) -> Result<ControlFlow<Span, Cow<'a, Value>>, ShellError> {
	match member {
		PathMember::Int {
			val: count,
			span: origin_span,
			optional,
		} => {
			// Treat a numeric path member as `select <val>`
			match current {
				Value::List { vals, .. } => {
					if *count < vals.len() {
						Ok(ControlFlow::Continue(Cow::Borrowed(&vals[*count])))
					} else if *optional {
						Ok(ControlFlow::Break(*origin_span))
						// short-circuit
					} else if vals.is_empty() {
						Err(ShellError::AccessEmptyContent { span: *origin_span })
					} else {
						Err(ShellError::AccessBeyondEnd {
							max_idx: vals.len() - 1,
							span: *origin_span,
						})
					}
				}
				Value::Binary { val, .. } => {
					if let Some(item) = val.get(*count) {
						Ok(ControlFlow::Continue(Cow::Owned(Value::int(*item as i64, *origin_span))))
					} else if *optional {
						Ok(ControlFlow::Break(*origin_span))
						// short-circuit
					} else if val.is_empty() {
						Err(ShellError::AccessEmptyContent { span: *origin_span })
					} else {
						Err(ShellError::AccessBeyondEnd {
							max_idx: val.len() - 1,
							span: *origin_span,
						})
					}
				}
				Value::Range { val, .. } => {
					if let Some(item) = val.into_range_iter(current.span(), Signals::empty()).nth(*count) {
						Ok(ControlFlow::Continue(Cow::Owned(item)))
					} else if *optional {
						Ok(ControlFlow::Break(*origin_span))
						// short-circuit
					} else {
						Err(ShellError::AccessBeyondEndOfStream { span: *origin_span })
					}
				}
				Value::Custom { val, .. } => {
					match val.follow_path_int(current.span(), *count, *origin_span, *optional) {
						Ok(val) => Ok(ControlFlow::Continue(Cow::Owned(val))),
						Err(err) => {
							if *optional {
								Ok(ControlFlow::Break(*origin_span))
								// short-circuit
							} else {
								Err(err)
							}
						}
					}
				}
				Value::Nothing { .. } if *optional => Ok(ControlFlow::Break(*origin_span)),
				// Records (and tables) are the only built-in which support column names,
				// so only use this message for them.
				Value::Record { .. } => Err(ShellError::TypeMismatch {
					err_message: "Can't access record values with a row index. Try specifying a column name instead".into(),
					span: *origin_span,
				}),
				Value::Error { error, .. } => Err(*error.clone()),
				x => Err(ShellError::IncompatiblePathAccess {
					type_name: format!("{}", x.get_type()),
					span: *origin_span,
				}),
			}
		}
		PathMember::String {
			val: column_name,
			span: origin_span,
			optional,
			casing,
		} => {
			let span = current.span();
			match current {
				Value::Record { val, .. } => {
					let found = val.cased(*casing).get(column_name);
					if let Some(found) = found {
						Ok(ControlFlow::Continue(Cow::Borrowed(found)))
					} else if *optional {
						Ok(ControlFlow::Break(*origin_span))
						// short-circuit
					} else if let Some(suggestion) = did_you_mean(val.columns(), column_name) {
						Err(ShellError::DidYouMean {
							suggestion,
							span: *origin_span,
						})
					} else {
						Err(ShellError::CantFindColumn {
							col_name: column_name.clone(),
							span: Some(*origin_span),
							src_span: span,
						})
					}
				}
				// String access of Lists always means Table access.
				// Create a List which contains each matching value for contained
				// records in the source list.
				Value::List { vals, .. } => {
					let list = vals
						.iter()
						.map(|val| {
							let val_span = val.span();
							match val {
								Value::Record { val, .. } => {
									let found = val.cased(*casing).get(column_name);
									if let Some(found) = found {
										Ok(found.clone())
									} else if *optional {
										Ok(Value::nothing(*origin_span))
									} else if let Some(suggestion) = did_you_mean(val.columns(), column_name) {
										Err(ShellError::DidYouMean {
											suggestion,
											span: *origin_span,
										})
									} else {
										Err(ShellError::CantFindColumn {
											col_name: column_name.clone(),
											span: Some(*origin_span),
											src_span: val_span,
										})
									}
								}
								Value::Nothing { .. } if *optional => Ok(Value::nothing(*origin_span)),
								_ => Err(ShellError::CantFindColumn {
									col_name: column_name.clone(),
									span: Some(*origin_span),
									src_span: val_span,
								}),
							}
						})
						.collect::<Result<_, _>>()?;

					Ok(ControlFlow::Continue(Cow::Owned(Value::list(list, span))))
				}
				Value::Custom { val, .. } => {
					match val.follow_path_string(current.span(), column_name.clone(), *origin_span, *optional, *casing) {
						Ok(val) => Ok(ControlFlow::Continue(Cow::Owned(val))),
						Err(err) => {
							if *optional {
								Ok(ControlFlow::Break(*origin_span))
								// short-circuit
							} else {
								Err(err)
							}
						}
					}
				}
				Value::Nothing { .. } if *optional => Ok(ControlFlow::Break(*origin_span)),
				Value::Error { error, .. } => Err(error.as_ref().clone()),
				x => Err(ShellError::IncompatiblePathAccess {
					type_name: format!("{}", x.get_type()),
					span: *origin_span,
				}),
			}
		}
	}
}
