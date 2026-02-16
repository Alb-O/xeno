//! Xeno-owned Nu data boundary.
//!
//! This crate defines a compact value model used at Xeno integration
//! boundaries. It intentionally supports only the subset used by runtime
//! effects/config parsing and provides explicit conversions to/from the
//! vendored Nu value types.

use std::fmt;

/// Span attached to a value for diagnostics.
#[derive(Clone, Copy, Default, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Span {
	pub start: usize,
	pub end: usize,
}

impl Span {
	pub const fn new(start: usize, end: usize) -> Self {
		Self { start, end }
	}

	pub const fn unknown() -> Self {
		Self { start: 0, end: 0 }
	}

	pub const fn test_data() -> Self {
		Self {
			start: usize::MAX / 2,
			end: usize::MAX / 2,
		}
	}
}

/// Insertion-ordered record used by [`Value::Record`].
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Record {
	inner: Vec<(String, Value)>,
}

impl Record {
	pub fn new() -> Self {
		Self::default()
	}

	pub fn with_capacity(capacity: usize) -> Self {
		Self {
			inner: Vec::with_capacity(capacity),
		}
	}

	pub fn is_empty(&self) -> bool {
		self.inner.is_empty()
	}

	pub fn len(&self) -> usize {
		self.inner.len()
	}

	pub fn push<K>(&mut self, key: K, value: Value)
	where
		K: Into<String>,
	{
		self.inner.push((key.into(), value));
	}

	pub fn contains(&self, key: impl AsRef<str>) -> bool {
		self.get(key).is_some()
	}

	pub fn get(&self, key: impl AsRef<str>) -> Option<&Value> {
		let key = key.as_ref();
		self.inner.iter().rfind(|(k, _)| k == key).map(|(_, v)| v)
	}

	pub fn get_mut(&mut self, key: impl AsRef<str>) -> Option<&mut Value> {
		let key = key.as_ref();
		self.inner.iter_mut().rfind(|(k, _)| k == key).map(|(_, v)| v)
	}

	pub fn iter(&self) -> RecordIter<'_> {
		RecordIter { inner: self.inner.iter() }
	}
}

pub struct RecordIter<'a> {
	inner: std::slice::Iter<'a, (String, Value)>,
}

impl<'a> Iterator for RecordIter<'a> {
	type Item = (&'a String, &'a Value);

	fn next(&mut self) -> Option<Self::Item> {
		self.inner.next().map(|(key, value)| (key, value))
	}
}

impl<'a> ExactSizeIterator for RecordIter<'a> {}

impl<'a> IntoIterator for &'a Record {
	type Item = (&'a String, &'a Value);
	type IntoIter = RecordIter<'a>;

	fn into_iter(self) -> Self::IntoIter {
		self.iter()
	}
}

/// Runtime value type used by Xeno-facing Nu APIs.
#[derive(Debug, Clone, PartialEq)]
pub enum Value {
	Bool { val: bool, internal_span: Span },
	Int { val: i64, internal_span: Span },
	Float { val: f64, internal_span: Span },
	String { val: String, internal_span: Span },
	Record { val: Record, internal_span: Span },
	List { vals: Vec<Value>, internal_span: Span },
	Nothing { internal_span: Span },
}

impl Value {
	pub fn bool(val: bool, span: Span) -> Self {
		Self::Bool { val, internal_span: span }
	}

	pub fn int(val: i64, span: Span) -> Self {
		Self::Int { val, internal_span: span }
	}

	pub fn float(val: f64, span: Span) -> Self {
		Self::Float { val, internal_span: span }
	}

	pub fn string(val: impl Into<String>, span: Span) -> Self {
		Self::String {
			val: val.into(),
			internal_span: span,
		}
	}

	pub fn record(val: Record, span: Span) -> Self {
		Self::Record { val, internal_span: span }
	}

	pub fn list(vals: Vec<Value>, span: Span) -> Self {
		Self::List { vals, internal_span: span }
	}

	pub fn nothing(span: Span) -> Self {
		Self::Nothing { internal_span: span }
	}

	pub fn test_bool(val: bool) -> Self {
		Self::bool(val, Span::test_data())
	}

	pub fn test_int(val: i64) -> Self {
		Self::int(val, Span::test_data())
	}

	pub fn test_string(val: impl Into<String>) -> Self {
		Self::string(val, Span::test_data())
	}

	pub fn test_nothing() -> Self {
		Self::nothing(Span::test_data())
	}

	pub fn test_list(vals: Vec<Value>) -> Self {
		Self::list(vals, Span::test_data())
	}

	pub fn test_record(val: Record) -> Self {
		Self::record(val, Span::test_data())
	}

	pub fn span(&self) -> Span {
		match self {
			Self::Bool { internal_span, .. }
			| Self::Int { internal_span, .. }
			| Self::Float { internal_span, .. }
			| Self::String { internal_span, .. }
			| Self::Record { internal_span, .. }
			| Self::List { internal_span, .. }
			| Self::Nothing { internal_span } => *internal_span,
		}
	}

	pub fn get_type(&self) -> NuType {
		match self {
			Self::Bool { .. } => NuType::Bool,
			Self::Int { .. } => NuType::Int,
			Self::Float { .. } => NuType::Float,
			Self::String { .. } => NuType::String,
			Self::Record { .. } => NuType::Record,
			Self::List { .. } => NuType::List,
			Self::Nothing { .. } => NuType::Nothing,
		}
	}

	pub fn is_nothing(&self) -> bool {
		matches!(self, Self::Nothing { .. })
	}

	pub fn as_bool(&self) -> Result<bool, ValueTypeError> {
		match self {
			Self::Bool { val, .. } => Ok(*val),
			other => Err(ValueTypeError::new("bool", other.get_type())),
		}
	}

	pub fn as_int(&self) -> Result<i64, ValueTypeError> {
		match self {
			Self::Int { val, .. } => Ok(*val),
			other => Err(ValueTypeError::new("int", other.get_type())),
		}
	}

	pub fn as_float(&self) -> Result<f64, ValueTypeError> {
		match self {
			Self::Float { val, .. } => Ok(*val),
			other => Err(ValueTypeError::new("float", other.get_type())),
		}
	}

	pub fn as_str(&self) -> Result<&str, ValueTypeError> {
		match self {
			Self::String { val, .. } => Ok(val),
			other => Err(ValueTypeError::new("string", other.get_type())),
		}
	}

	pub fn as_list(&self) -> Result<&[Value], ValueTypeError> {
		match self {
			Self::List { vals, .. } => Ok(vals),
			other => Err(ValueTypeError::new("list", other.get_type())),
		}
	}

	pub fn as_record(&self) -> Result<&Record, ValueTypeError> {
		match self {
			Self::Record { val, .. } => Ok(val),
			other => Err(ValueTypeError::new("record", other.get_type())),
		}
	}

	pub fn into_record(self) -> Result<Record, Self> {
		match self {
			Self::Record { val, .. } => Ok(val),
			other => Err(other),
		}
	}
}

/// Coarse value type used for diagnostics.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum NuType {
	Bool,
	Int,
	Float,
	String,
	Record,
	List,
	Nothing,
}

impl fmt::Display for NuType {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		let name = match self {
			Self::Bool => "bool",
			Self::Int => "int",
			Self::Float => "float",
			Self::String => "string",
			Self::Record => "record",
			Self::List => "list",
			Self::Nothing => "nothing",
		};
		f.write_str(name)
	}
}

/// Error returned by typed accessors like [`Value::as_record`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValueTypeError {
	expected: &'static str,
	got: NuType,
}

impl ValueTypeError {
	pub fn new(expected: &'static str, got: NuType) -> Self {
		Self { expected, got }
	}
}

impl fmt::Display for ValueTypeError {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		write!(f, "expected {}, got {}", self.expected, self.got)
	}
}

impl std::error::Error for ValueTypeError {}

/// Conversion error between Xeno data values and vendored Nu values.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConversionError {
	message: String,
}

impl ConversionError {
	fn unsupported(ty: impl Into<String>) -> Self {
		Self {
			message: format!("unsupported Nu value type: {}", ty.into()),
		}
	}
}

impl fmt::Display for ConversionError {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		f.write_str(&self.message)
	}
}

impl std::error::Error for ConversionError {}

impl From<xeno_nu_protocol::Span> for Span {
	fn from(span: xeno_nu_protocol::Span) -> Self {
		Self {
			start: span.start,
			end: span.end,
		}
	}
}

impl From<Span> for xeno_nu_protocol::Span {
	fn from(span: Span) -> Self {
		Self::new(span.start, span.end)
	}
}

impl TryFrom<xeno_nu_protocol::Record> for Record {
	type Error = ConversionError;

	fn try_from(record: xeno_nu_protocol::Record) -> Result<Self, Self::Error> {
		let mut out = Self::with_capacity(record.len());
		for (key, value) in record.iter() {
			out.push(key.clone(), Value::try_from(value.clone())?);
		}
		Ok(out)
	}
}

impl From<Record> for xeno_nu_protocol::Record {
	fn from(record: Record) -> Self {
		let mut out = Self::new();
		for (key, value) in &record {
			out.push(key.clone(), value.clone().into());
		}
		out
	}
}

impl TryFrom<xeno_nu_protocol::Value> for Value {
	type Error = ConversionError;

	fn try_from(value: xeno_nu_protocol::Value) -> Result<Self, Self::Error> {
		match value {
			xeno_nu_protocol::Value::Bool { val, internal_span, .. } => Ok(Self::bool(val, internal_span.into())),
			xeno_nu_protocol::Value::Int { val, internal_span, .. } => Ok(Self::int(val, internal_span.into())),
			xeno_nu_protocol::Value::Float { val, internal_span, .. } => Ok(Self::float(val, internal_span.into())),
			xeno_nu_protocol::Value::String { val, internal_span, .. } => Ok(Self::string(val, internal_span.into())),
			xeno_nu_protocol::Value::Record { val, internal_span, .. } => {
				let mut out = Record::with_capacity(val.len());
				for (key, item) in val.iter() {
					out.push(key.clone(), Self::try_from(item.clone())?);
				}
				Ok(Self::record(out, internal_span.into()))
			}
			xeno_nu_protocol::Value::List { vals, internal_span, .. } => {
				let vals = vals.into_iter().map(Self::try_from).collect::<Result<Vec<_>, _>>()?;
				Ok(Self::list(vals, internal_span.into()))
			}
			xeno_nu_protocol::Value::Nothing { internal_span, .. } => Ok(Self::nothing(internal_span.into())),
			other => Err(ConversionError::unsupported(other.get_type().to_string())),
		}
	}
}

impl From<Value> for xeno_nu_protocol::Value {
	fn from(value: Value) -> Self {
		match value {
			Value::Bool { val, internal_span } => Self::bool(val, internal_span.into()),
			Value::Int { val, internal_span } => Self::int(val, internal_span.into()),
			Value::Float { val, internal_span } => Self::float(val, internal_span.into()),
			Value::String { val, internal_span } => Self::string(val, internal_span.into()),
			Value::Record { val, internal_span } => Self::record(val.into(), internal_span.into()),
			Value::List { vals, internal_span } => Self::list(vals.into_iter().map(Into::into).collect(), internal_span.into()),
			Value::Nothing { internal_span } => Self::nothing(internal_span.into()),
		}
	}
}

pub type NuValue = Value;
pub type NuRecord = Record;
pub type NuSpan = Span;
