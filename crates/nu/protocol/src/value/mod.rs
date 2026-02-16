mod custom_value;
mod duration;
mod filesize;
mod from_value;
mod glob;
mod into_value;
mod range;
#[cfg(test)]
mod test_derive;

pub mod record;
use std::borrow::Cow;
use std::cmp::Ordering;
use std::fmt::{Debug, Display, Write};
use std::ops::{Bound, ControlFlow};
use std::path::PathBuf;

use chrono::{DateTime, Datelike, Duration, FixedOffset, Local, Locale, TimeZone};
use chrono_humanize::HumanTime;
pub use custom_value::CustomValue;
pub use duration::*;
use fancy_regex::Regex;
pub use filesize::*;
pub use from_value::FromValue;
pub use glob::*;
pub use into_value::{IntoValue, TryIntoValue};
pub use range::{FloatRange, IntRange, Range};
pub use record::Record;
use serde::{Deserialize, Serialize};
pub use xeno_nu_utils::MultiLife;
use xeno_nu_utils::locale::{LOCALE_OVERRIDE_ENV_VAR, get_system_locale_string};
use xeno_nu_utils::{ObviousFloat, SharedCow, contains_emoji};

use crate::ast::{Bits, Boolean, CellPath, Comparison, Math, Operator, PathMember};
use crate::engine::{Closure, EngineState};
use crate::{BlockId, Config, ShellError, Signals, Span, Type, did_you_mean};

/// Core structured values that pass through the pipeline in Nushell.
// NOTE: Please do not reorder these enum cases without thinking through the
// impact on the PartialOrd implementation and the global sort order
// NOTE: All variants are marked as `non_exhaustive` to prevent them
// from being constructed (outside of this crate) with the struct
// expression syntax. This makes using the constructor methods the
// only way to construct `Value`'s
#[derive(Debug, Serialize, Deserialize)]
pub enum Value {
	#[non_exhaustive]
	Bool {
		val: bool,
		/// note: spans are being refactored out of Value
		/// please use .span() instead of matching this span value
		#[serde(rename = "span")]
		internal_span: Span,
	},
	#[non_exhaustive]
	Int {
		val: i64,
		/// note: spans are being refactored out of Value
		/// please use .span() instead of matching this span value
		#[serde(rename = "span")]
		internal_span: Span,
	},
	#[non_exhaustive]
	Float {
		val: f64,
		/// note: spans are being refactored out of Value
		/// please use .span() instead of matching this span value
		#[serde(rename = "span")]
		internal_span: Span,
	},
	#[non_exhaustive]
	String {
		val: String,
		/// note: spans are being refactored out of Value
		/// please use .span() instead of matching this span value
		#[serde(rename = "span")]
		internal_span: Span,
	},
	#[non_exhaustive]
	Glob {
		val: String,
		no_expand: bool,
		/// note: spans are being refactored out of Value
		/// please use .span() instead of matching this span value
		#[serde(rename = "span")]
		internal_span: Span,
	},
	#[non_exhaustive]
	Filesize {
		val: Filesize,
		/// note: spans are being refactored out of Value
		/// please use .span() instead of matching this span value
		#[serde(rename = "span")]
		internal_span: Span,
	},
	#[non_exhaustive]
	Duration {
		/// The duration in nanoseconds.
		val: i64,
		/// note: spans are being refactored out of Value
		/// please use .span() instead of matching this span value
		#[serde(rename = "span")]
		internal_span: Span,
	},
	#[non_exhaustive]
	Date {
		val: DateTime<FixedOffset>,
		/// note: spans are being refactored out of Value
		/// please use .span() instead of matching this span value
		#[serde(rename = "span")]
		internal_span: Span,
	},
	#[non_exhaustive]
	Range {
		val: Box<Range>,
		#[serde(skip)]
		signals: Option<Signals>,
		/// note: spans are being refactored out of Value
		/// please use .span() instead of matching this span value
		#[serde(rename = "span")]
		internal_span: Span,
	},
	#[non_exhaustive]
	Record {
		val: SharedCow<Record>,
		/// note: spans are being refactored out of Value
		/// please use .span() instead of matching this span value
		#[serde(rename = "span")]
		internal_span: Span,
	},
	#[non_exhaustive]
	List {
		vals: Vec<Value>,
		#[serde(skip)]
		signals: Option<Signals>,
		/// note: spans are being refactored out of Value
		/// please use .span() instead of matching this span value
		#[serde(rename = "span")]
		internal_span: Span,
	},
	#[non_exhaustive]
	Closure {
		val: Box<Closure>,
		/// note: spans are being refactored out of Value
		/// please use .span() instead of matching this span value
		#[serde(rename = "span")]
		internal_span: Span,
	},
	#[non_exhaustive]
	Error {
		error: Box<ShellError>,
		/// note: spans are being refactored out of Value
		/// please use .span() instead of matching this span value
		#[serde(rename = "span")]
		internal_span: Span,
	},
	#[non_exhaustive]
	Binary {
		val: Vec<u8>,
		/// note: spans are being refactored out of Value
		/// please use .span() instead of matching this span value
		#[serde(rename = "span")]
		internal_span: Span,
	},
	#[non_exhaustive]
	CellPath {
		val: CellPath,
		/// note: spans are being refactored out of Value
		/// please use .span() instead of matching this span value
		#[serde(rename = "span")]
		internal_span: Span,
	},
	#[non_exhaustive]
	Custom {
		val: Box<dyn CustomValue>,
		/// note: spans are being refactored out of Value
		/// please use .span() instead of matching this span value
		#[serde(rename = "span")]
		internal_span: Span,
	},
	#[non_exhaustive]
	Nothing {
		/// note: spans are being refactored out of Value
		/// please use .span() instead of matching this span value
		#[serde(rename = "span")]
		internal_span: Span,
	},
}

// This is to document/enforce the size of `Value` in bytes.
// We should try to avoid increasing the size of `Value`,
// and PRs that do so will have to change the number below so that it's noted in review.
const _: () = assert!(std::mem::size_of::<Value>() <= 56);

include!("clone_impl.rs");
include!("conversion_impl.rs");
include!("cmp_impl.rs");
include!("operator_impl.rs");

#[cfg(test)]
mod tests {
	use super::{Record, Value};
	use crate::record;

	mod at_cell_path {
		use super::super::PathMember;
		use super::*;
		use crate::casing::Casing;
		use crate::{IntoValue, Span};

		#[test]
		fn test_record_with_data_at_cell_path() {
			let value_to_insert = Value::test_string("value");
			let span = Span::test_data();
			assert_eq!(
				Value::with_data_at_cell_path(
					&[
						PathMember::test_string("a".to_string(), false, Casing::Sensitive),
						PathMember::test_string("b".to_string(), false, Casing::Sensitive),
						PathMember::test_string("c".to_string(), false, Casing::Sensitive),
						PathMember::test_string("d".to_string(), false, Casing::Sensitive),
					],
					value_to_insert,
				),
				// {a:{b:c{d:"value"}}}
				Ok(record!(
					"a" => record!(
						"b" => record!(
							"c" => record!(
								"d" => Value::test_string("value")
							).into_value(span)
						).into_value(span)
					).into_value(span)
				)
				.into_value(span))
			);
		}

		#[test]
		fn test_lists_with_data_at_cell_path() {
			let value_to_insert = Value::test_string("value");
			assert_eq!(
				Value::with_data_at_cell_path(
					&[
						PathMember::test_int(0, false),
						PathMember::test_int(0, false),
						PathMember::test_int(0, false),
						PathMember::test_int(0, false),
					],
					value_to_insert.clone(),
				),
				// [[[[["value"]]]]]
				Ok(Value::test_list(vec![Value::test_list(vec![Value::test_list(vec![Value::test_list(vec![
					value_to_insert
				])])])]))
			);
		}
		#[test]
		fn test_mixed_with_data_at_cell_path() {
			let value_to_insert = Value::test_string("value");
			let span = Span::test_data();
			assert_eq!(
				Value::with_data_at_cell_path(
					&[
						PathMember::test_string("a".to_string(), false, Casing::Sensitive),
						PathMember::test_int(0, false),
						PathMember::test_string("b".to_string(), false, Casing::Sensitive),
						PathMember::test_int(0, false),
						PathMember::test_string("c".to_string(), false, Casing::Sensitive),
						PathMember::test_int(0, false),
						PathMember::test_string("d".to_string(), false, Casing::Sensitive),
						PathMember::test_int(0, false),
					],
					value_to_insert.clone(),
				),
				// [{a:[{b:[{c:[{d:["value"]}]}]}]]}
				Ok(record!(
					"a" => Value::test_list(vec![record!(
						"b" => Value::test_list(vec![record!(
							"c" => Value::test_list(vec![record!(
								"d" => Value::test_list(vec![value_to_insert])
							).into_value(span)])
						).into_value(span)])
					).into_value(span)])
				)
				.into_value(span))
			);
		}

		#[test]
		fn test_nested_upsert_data_at_cell_path() {
			let span = Span::test_data();
			let mut base_value = record!(
				"a" => Value::test_list(vec![])
			)
			.into_value(span);

			let value_to_insert = Value::test_string("value");
			let res = base_value.upsert_data_at_cell_path(
				&[
					PathMember::test_string("a".to_string(), false, Casing::Sensitive),
					PathMember::test_int(0, false),
					PathMember::test_string("b".to_string(), false, Casing::Sensitive),
					PathMember::test_int(0, false),
				],
				value_to_insert.clone(),
			);
			assert_eq!(res, Ok(()));
			assert_eq!(
				base_value,
				// {a:[{b:["value"]}]}
				record!(
					"a" => Value::test_list(vec![
						record!(
							"b" => Value::test_list(vec![value_to_insert])
						)
						.into_value(span)
					])
				)
				.into_value(span)
			);
		}

		#[test]
		fn test_nested_insert_data_at_cell_path() {
			let span = Span::test_data();
			let mut base_value = record!(
				"a" => Value::test_list(vec![])
			)
			.into_value(span);

			let value_to_insert = Value::test_string("value");
			let res = base_value.insert_data_at_cell_path(
				&[
					PathMember::test_string("a".to_string(), false, Casing::Sensitive),
					PathMember::test_int(0, false),
					PathMember::test_string("b".to_string(), false, Casing::Sensitive),
					PathMember::test_int(0, false),
				],
				value_to_insert.clone(),
				span,
			);
			assert_eq!(res, Ok(()));
			assert_eq!(
				base_value,
				// {a:[{b:["value"]}]}
				record!(
					"a" => Value::test_list(vec![
						record!(
							"b" => Value::test_list(vec![value_to_insert])
						)
						.into_value(span)
					])
				)
				.into_value(span)
			);
		}
	}

	mod is_empty {
		use super::*;

		#[test]
		fn test_string() {
			let value = Value::test_string("");
			assert!(value.is_empty());
		}

		#[test]
		fn test_list() {
			let list_with_no_values = Value::test_list(vec![]);
			let list_with_one_empty_string = Value::test_list(vec![Value::test_string("")]);

			assert!(list_with_no_values.is_empty());
			assert!(!list_with_one_empty_string.is_empty());
		}

		#[test]
		fn test_record() {
			let no_columns_nor_cell_values = Value::test_record(Record::new());

			let one_column_and_one_cell_value_with_empty_strings = Value::test_record(record! {
				"" => Value::test_string(""),
			});

			let one_column_with_a_string_and_one_cell_value_with_empty_string = Value::test_record(record! {
				"column" => Value::test_string(""),
			});

			let one_column_with_empty_string_and_one_value_with_a_string = Value::test_record(record! {
				"" => Value::test_string("text"),
			});

			assert!(no_columns_nor_cell_values.is_empty());
			assert!(!one_column_and_one_cell_value_with_empty_strings.is_empty());
			assert!(!one_column_with_a_string_and_one_cell_value_with_empty_string.is_empty());
			assert!(!one_column_with_empty_string_and_one_value_with_a_string.is_empty());
		}
	}

	mod get_type {
		use super::*;
		use crate::Type;

		#[test]
		fn test_list() {
			let list_of_ints = Value::test_list(vec![Value::test_int(0)]);
			let list_of_floats = Value::test_list(vec![Value::test_float(0.0)]);
			let list_of_ints_and_floats = Value::test_list(vec![Value::test_int(0), Value::test_float(0.0)]);
			let list_of_ints_and_floats_and_bools = Value::test_list(vec![Value::test_int(0), Value::test_float(0.0), Value::test_bool(false)]);
			assert_eq!(list_of_ints.get_type(), Type::List(Box::new(Type::Int)));
			assert_eq!(list_of_floats.get_type(), Type::List(Box::new(Type::Float)));
			assert_eq!(
				list_of_ints_and_floats_and_bools.get_type(),
				Type::List(Box::new(Type::OneOf(vec![Type::Number, Type::Bool].into_boxed_slice())))
			);
			assert_eq!(list_of_ints_and_floats.get_type(), Type::List(Box::new(Type::Number)));
		}
	}

	mod is_subtype {
		use super::*;
		use crate::Type;

		fn assert_subtype_equivalent(value: &Value, ty: &Type) {
			assert_eq!(value.is_subtype_of(ty), value.get_type().is_subtype_of(ty));
		}

		#[test]
		fn test_list() {
			let ty_int_list = Type::list(Type::Int);
			let ty_str_list = Type::list(Type::String);
			let ty_any_list = Type::list(Type::Any);
			let ty_list_list_int = Type::list(Type::list(Type::Int));

			let list = Value::test_list(vec![Value::test_int(1), Value::test_int(2), Value::test_int(3)]);

			assert_subtype_equivalent(&list, &ty_int_list);
			assert_subtype_equivalent(&list, &ty_str_list);
			assert_subtype_equivalent(&list, &ty_any_list);

			let list = Value::test_list(vec![Value::test_int(1), Value::test_string("hi"), Value::test_int(3)]);

			assert_subtype_equivalent(&list, &ty_int_list);
			assert_subtype_equivalent(&list, &ty_str_list);
			assert_subtype_equivalent(&list, &ty_any_list);

			let list = Value::test_list(vec![Value::test_list(vec![Value::test_int(1)])]);

			assert_subtype_equivalent(&list, &ty_list_list_int);

			// The type of an empty lists is a subtype of any list or table type
			let ty_table = Type::Table(Box::new([("a".into(), Type::Int), ("b".into(), Type::Int), ("c".into(), Type::Int)]));
			let empty = Value::test_list(vec![]);

			assert_subtype_equivalent(&empty, &ty_any_list);
			assert!(empty.is_subtype_of(&ty_int_list));
			assert!(empty.is_subtype_of(&ty_table));
		}

		#[test]
		fn test_record() {
			let ty_abc = Type::Record(Box::new([("a".into(), Type::Int), ("b".into(), Type::Int), ("c".into(), Type::Int)]));
			let ty_ab = Type::Record(Box::new([("a".into(), Type::Int), ("b".into(), Type::Int)]));
			let ty_inner = Type::Record(Box::new([("inner".into(), ty_abc.clone())]));

			let record_abc = Value::test_record(record! {
				"a" => Value::test_int(1),
				"b" => Value::test_int(2),
				"c" => Value::test_int(3),
			});
			let record_ab = Value::test_record(record! {
				"a" => Value::test_int(1),
				"b" => Value::test_int(2),
			});

			assert_subtype_equivalent(&record_abc, &ty_abc);
			assert_subtype_equivalent(&record_abc, &ty_ab);
			assert_subtype_equivalent(&record_ab, &ty_abc);
			assert_subtype_equivalent(&record_ab, &ty_ab);

			let record_inner = Value::test_record(record! {
				"inner" => record_abc
			});
			assert_subtype_equivalent(&record_inner, &ty_inner);
		}

		#[test]
		fn test_table() {
			let ty_abc = Type::Table(Box::new([("a".into(), Type::Int), ("b".into(), Type::Int), ("c".into(), Type::Int)]));
			let ty_ab = Type::Table(Box::new([("a".into(), Type::Int), ("b".into(), Type::Int)]));
			let ty_list_any = Type::list(Type::Any);

			let record_abc = Value::test_record(record! {
				"a" => Value::test_int(1),
				"b" => Value::test_int(2),
				"c" => Value::test_int(3),
			});
			let record_ab = Value::test_record(record! {
				"a" => Value::test_int(1),
				"b" => Value::test_int(2),
			});

			let table_abc = Value::test_list(vec![record_abc.clone(), record_abc.clone()]);
			let table_ab = Value::test_list(vec![record_ab.clone(), record_ab.clone()]);

			assert_subtype_equivalent(&table_abc, &ty_abc);
			assert_subtype_equivalent(&table_abc, &ty_ab);
			assert_subtype_equivalent(&table_ab, &ty_abc);
			assert_subtype_equivalent(&table_ab, &ty_ab);
			assert_subtype_equivalent(&table_abc, &ty_list_any);

			let table_mixed = Value::test_list(vec![record_abc.clone(), record_ab.clone()]);
			assert_subtype_equivalent(&table_mixed, &ty_abc);
			assert!(table_mixed.is_subtype_of(&ty_ab));

			let ty_a = Type::Table(Box::new([("a".into(), Type::Any)]));
			let table_mixed_types = Value::test_list(vec![
				Value::test_record(record! {
					"a" => Value::test_int(1),
				}),
				Value::test_record(record! {
					"a" => Value::test_string("a"),
				}),
			]);
			assert!(table_mixed_types.is_subtype_of(&ty_a));
		}
	}

	mod into_string {
		use chrono::{DateTime, FixedOffset};

		use super::*;

		#[test]
		fn test_datetime() {
			let date = DateTime::from_timestamp_millis(-123456789)
				.unwrap()
				.with_timezone(&FixedOffset::east_opt(0).unwrap());

			let string = Value::test_date(date).to_expanded_string("", &Default::default());

			// We need to cut the humanized part off for tests to work, because
			// it is relative to current time.
			let formatted = string.split('(').next().unwrap();
			assert_eq!("Tue, 30 Dec 1969 13:42:23 +0000 ", formatted);
		}

		#[test]
		fn test_negative_year_datetime() {
			let date = DateTime::from_timestamp_millis(-72135596800000)
				.unwrap()
				.with_timezone(&FixedOffset::east_opt(0).unwrap());

			let string = Value::test_date(date).to_expanded_string("", &Default::default());

			// We need to cut the humanized part off for tests to work, because
			// it is relative to current time.
			let formatted = string.split(' ').next().unwrap();
			assert_eq!("-0316-02-11T06:13:20+00:00", formatted);
		}
	}

	#[test]
	fn test_env_as_bool() {
		// explicit false values
		assert_eq!(Value::test_bool(false).coerce_bool(), Ok(false));
		assert_eq!(Value::test_int(0).coerce_bool(), Ok(false));
		assert_eq!(Value::test_float(0.0).coerce_bool(), Ok(false));
		assert_eq!(Value::test_string("").coerce_bool(), Ok(false));
		assert_eq!(Value::test_string("0").coerce_bool(), Ok(false));
		assert_eq!(Value::test_nothing().coerce_bool(), Ok(false));

		// explicit true values
		assert_eq!(Value::test_bool(true).coerce_bool(), Ok(true));
		assert_eq!(Value::test_int(1).coerce_bool(), Ok(true));
		assert_eq!(Value::test_float(1.0).coerce_bool(), Ok(true));
		assert_eq!(Value::test_string("1").coerce_bool(), Ok(true));

		// implicit true values
		assert_eq!(Value::test_int(42).coerce_bool(), Ok(true));
		assert_eq!(Value::test_float(0.5).coerce_bool(), Ok(true));
		assert_eq!(Value::test_string("not zero").coerce_bool(), Ok(true));

		// complex values returning None
		assert!(Value::test_record(Record::default()).coerce_bool().is_err());
		assert!(Value::test_list(vec![Value::test_int(1)]).coerce_bool().is_err());
		assert!(
			Value::test_date(chrono::DateTime::parse_from_rfc3339("2024-01-01T12:00:00+00:00").unwrap(),)
				.coerce_bool()
				.is_err()
		);
		assert!(Value::test_glob("*.rs").coerce_bool().is_err());
		assert!(Value::test_binary(vec![1, 2, 3]).coerce_bool().is_err());
		assert!(Value::test_duration(3600).coerce_bool().is_err());
	}

	mod memory_size {
		use super::*;

		#[test]
		fn test_primitive_sizes() {
			// All primitive values should have the same base size (size of the Value enum)
			let base_size = std::mem::size_of::<Value>();

			assert_eq!(Value::test_bool(true).memory_size(), base_size);
			assert_eq!(Value::test_int(42).memory_size(), base_size);
			assert_eq!(Value::test_float(1.5).memory_size(), base_size);
			assert_eq!(Value::test_nothing().memory_size(), base_size);
		}

		#[test]
		fn test_string_size() {
			let s = "hello world";
			let val = Value::test_string(s);
			let base_size = std::mem::size_of::<Value>();
			// String memory size should be base + capacity (allocated size)
			let string_val = String::from(s);
			let expected = base_size + string_val.capacity();
			assert_eq!(val.memory_size(), expected);
		}

		#[test]
		fn test_binary_size() {
			let data = vec![1, 2, 3, 4, 5];
			let val = Value::test_binary(data.clone());
			let base_size = std::mem::size_of::<Value>();
			let expected = base_size + data.capacity();
			assert_eq!(val.memory_size(), expected);
		}

		#[test]
		fn test_list_size() {
			let list = Value::test_list(vec![Value::test_int(1), Value::test_int(2), Value::test_int(3)]);

			let base_size = std::mem::size_of::<Value>();
			let element_size = std::mem::size_of::<Value>();
			// List size = base + sum of element sizes
			let expected = base_size + 3 * element_size;
			assert_eq!(list.memory_size(), expected);
		}

		#[test]
		fn test_record_size() {
			let record = Value::test_record(record! {
				"a" => Value::test_int(1),
				"b" => Value::test_string("hello"),
			});

			let base_size = std::mem::size_of::<Value>();
			let record_base_size = std::mem::size_of::<Record>();
			let key1_size = String::from("a").capacity();
			let key2_size = String::from("b").capacity();
			let val1_size = std::mem::size_of::<Value>();
			let val2_base_size = std::mem::size_of::<Value>();
			let val2_string_size = String::from("hello").capacity();

			let expected = base_size + record_base_size + key1_size + key2_size + val1_size + (val2_base_size + val2_string_size);
			assert_eq!(record.memory_size(), expected);
		}

		#[test]
		fn test_nested_structure_size() {
			// Test a more complex nested structure
			let inner_record = Value::test_record(record! {
				"x" => Value::test_int(10),
				"y" => Value::test_string("test"),
			});

			let list = Value::test_list(vec![inner_record]);

			let record_size = list.memory_size();
			// The list contains one record, so size should be base + record_size
			let base_size = std::mem::size_of::<Value>();
			assert!(record_size > base_size);

			// Verify it's larger than a simple list
			let simple_list = Value::test_list(vec![Value::test_int(1)]);
			assert!(record_size > simple_list.memory_size());
		}
	}
}
