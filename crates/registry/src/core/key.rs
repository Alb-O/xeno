use super::index::RegistryRef;
use super::traits::RegistryEntry;

/// The value of an option.
#[derive(Debug, Clone, PartialEq)]
pub enum OptionValue {
	/// Boolean value (true/false).
	Bool(bool),
	/// Integer value.
	Int(i64),
	/// String value.
	String(String),
}

impl OptionValue {
	/// Returns the boolean value if this is a `Bool` variant.
	pub fn as_bool(&self) -> Option<bool> {
		match self {
			OptionValue::Bool(v) => Some(*v),
			_ => None,
		}
	}

	/// Returns the integer value if this is an `Int` variant.
	pub fn as_int(&self) -> Option<i64> {
		match self {
			OptionValue::Int(v) => Some(*v),
			_ => None,
		}
	}

	/// Returns the string value if this is a `String` variant.
	pub fn as_str(&self) -> Option<&str> {
		match self {
			OptionValue::String(v) => Some(v),
			_ => None,
		}
	}

	/// Returns true if this value matches the given type.
	pub fn matches_type(&self, ty: OptionType) -> bool {
		matches!(
			(self, ty),
			(OptionValue::Bool(_), OptionType::Bool) | (OptionValue::Int(_), OptionType::Int) | (OptionValue::String(_), OptionType::String)
		)
	}

	/// Returns the type name of this value.
	pub fn type_name(&self) -> &'static str {
		match self {
			OptionValue::Bool(_) => "bool",
			OptionValue::Int(_) => "int",
			OptionValue::String(_) => "string",
		}
	}
}

impl From<bool> for OptionValue {
	fn from(v: bool) -> Self {
		OptionValue::Bool(v)
	}
}

impl From<i64> for OptionValue {
	fn from(v: i64) -> Self {
		OptionValue::Int(v)
	}
}

impl From<String> for OptionValue {
	fn from(v: String) -> Self {
		OptionValue::String(v)
	}
}

impl From<&str> for OptionValue {
	fn from(v: &str) -> Self {
		OptionValue::String(v.to_string())
	}
}

/// The type of an option's value.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OptionType {
	/// Boolean type.
	Bool,
	/// Integer type.
	Int,
	/// String type.
	String,
}

/// Typed carrier for option default values.
///
/// Unlike [`OptionValue`], this encodes the variant type at the Rust level via
/// function pointers. This enables build-time validation of option definitions
/// (ensuring the default matches the declared [`OptionType`]) and eliminates
/// runtime downcasting/unwraps during resolution.
#[derive(Clone)]
pub enum OptionDefault {
	/// Boolean default value factory.
	Bool(fn() -> bool),
	/// Integer default value factory.
	Int(fn() -> i64),
	/// String default value factory.
	String(fn() -> String),
	/// Static value (for registry-linked definitions).
	Value(OptionValue),
}

impl core::fmt::Debug for OptionDefault {
	fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
		match self {
			OptionDefault::Bool(_) => f.write_str("OptionDefault::Bool(..)"),
			OptionDefault::Int(_) => f.write_str("OptionDefault::Int(..)"),
			OptionDefault::String(_) => f.write_str("OptionDefault::String(..)"),
			OptionDefault::Value(v) => f.debug_tuple("OptionDefault::Value").field(v).finish(),
		}
	}
}

impl OptionDefault {
	/// Returns the [`OptionType`] produced by this default.
	pub fn value_type(&self) -> OptionType {
		match self {
			OptionDefault::Bool(_) => OptionType::Bool,
			OptionDefault::Int(_) => OptionType::Int,
			OptionDefault::String(_) => OptionType::String,
			OptionDefault::Value(v) => match v {
				OptionValue::Bool(_) => OptionType::Bool,
				OptionValue::Int(_) => OptionType::Int,
				OptionValue::String(_) => OptionType::String,
			},
		}
	}

	/// Invokes the factory function and returns the value as an [`OptionValue`].
	pub fn to_value(&self) -> OptionValue {
		match self {
			OptionDefault::Bool(f) => OptionValue::Bool(f()),
			OptionDefault::Int(f) => OptionValue::Int(f()),
			OptionDefault::String(f) => OptionValue::String(f()),
			OptionDefault::Value(v) => v.clone(),
		}
	}
}

// Seal the FromOptionValue trait to prevent external implementations.
mod sealed {
	pub trait Sealed {}
	impl Sealed for i64 {}
	impl Sealed for bool {}
	impl Sealed for String {}
}

/// Trait for types that can be extracted from an [`OptionValue`].
pub trait FromOptionValue: sealed::Sealed + Sized {
	/// Extracts the value from an `OptionValue`, returning `None` if the type doesn't match.
	fn from_option(value: &OptionValue) -> Option<Self>;

	/// Returns the `OptionType` corresponding to this Rust type.
	fn option_type() -> OptionType;
}

impl FromOptionValue for i64 {
	fn from_option(value: &OptionValue) -> Option<Self> {
		value.as_int()
	}

	fn option_type() -> OptionType {
		OptionType::Int
	}
}

impl FromOptionValue for bool {
	fn from_option(value: &OptionValue) -> Option<Self> {
		value.as_bool()
	}

	fn option_type() -> OptionType {
		OptionType::Bool
	}
}

impl FromOptionValue for String {
	fn from_option(value: &OptionValue) -> Option<Self> {
		value.as_str().map(|s| s.to_string())
	}

	fn option_type() -> OptionType {
		OptionType::String
	}
}

/// Typed handle for looking up a registry entry by canonical ID string.
///
/// Stores either a `&'static str` canonical ID (for compile-time handles emitted
/// by `*_handler!` macros) or a [`RegistryRef<T, Id>`] (for runtime-resolved
/// entries). Resolves entries through the registry's 3-stage lookup
/// (canonical ID, name, alias).
pub enum LookupKey<T, Id>
where
	T: RegistryEntry + Send + Sync + 'static,
	Id: crate::core::DenseId,
{
	/// Compile-time handle storing a canonical ID string (e.g., `"xeno-registry::move_left"`).
	Static(&'static str),
	/// Runtime handle from a registry snapshot.
	Ref(RegistryRef<T, Id>),
}

impl<T, Id> Clone for LookupKey<T, Id>
where
	T: RegistryEntry + Send + Sync + 'static,
	Id: crate::core::DenseId,
{
	fn clone(&self) -> Self {
		match self {
			Self::Static(s) => Self::Static(s),
			Self::Ref(r) => Self::Ref(r.clone()),
		}
	}
}

impl<T, Id> LookupKey<T, Id>
where
	T: RegistryEntry + Send + Sync + 'static,
	Id: crate::core::DenseId,
{
	/// Creates a compile-time handle from a canonical ID string.
	pub const fn new(canonical_id: &'static str) -> Self {
		Self::Static(canonical_id)
	}

	/// Creates a runtime handle from a registry reference.
	pub fn new_ref(r: RegistryRef<T, Id>) -> Self {
		Self::Ref(r)
	}
}

impl<T, Id> core::fmt::Debug for LookupKey<T, Id>
where
	T: RegistryEntry + Send + Sync + 'static,
	Id: crate::core::DenseId,
{
	fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
		match self {
			Self::Static(id) => f.debug_tuple("LookupKey::Static").field(id).finish(),
			Self::Ref(r) => f.debug_tuple("LookupKey::Ref").field(&r.dense_id()).finish(),
		}
	}
}
