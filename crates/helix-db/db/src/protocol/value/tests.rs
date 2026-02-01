use std::cmp::Ordering;
use std::collections::HashMap;

use chrono::Utc;

use super::*;

// ============================================================================
// Value Creation and From Implementations
// ============================================================================

#[test]
fn test_value_from_primitives() {
	assert!(matches!(Value::from("test"), Value::String(_)));
	assert!(matches!(
		Value::from(String::from("test")),
		Value::String(_)
	));
	assert!(matches!(Value::from(true), Value::Boolean(true)));
	assert!(matches!(Value::from(42i8), Value::I8(42)));
	assert!(matches!(Value::from(42i16), Value::I16(42)));
	assert!(matches!(Value::from(42i32), Value::I32(42)));
	assert!(matches!(Value::from(42i64), Value::I64(42)));
	assert!(matches!(Value::from(42u8), Value::U8(42)));
	assert!(matches!(Value::from(42u16), Value::U16(42)));
	assert!(matches!(Value::from(42u32), Value::U32(42)));
	assert!(matches!(Value::from(42u64), Value::U64(42)));
	assert!(matches!(Value::from(42u128), Value::U128(42)));
	assert!(matches!(Value::from(3.14f32), Value::F32(_)));
	assert!(matches!(Value::from(3.14f64), Value::F64(_)));
}

#[test]
fn test_value_from_string_trims_quotes() {
	let val = Value::from("\"quoted\"");
	assert_eq!(val, Value::String("quoted".to_string()));

	let val2 = Value::from(String::from("\"test\""));
	assert_eq!(val2, Value::String("test".to_string()));
}

#[test]
fn test_value_from_vec() {
	let vec_vals = vec![Value::I32(1), Value::I32(2), Value::I32(3)];
	let val = Value::from(vec_vals.clone());
	assert!(matches!(val, Value::Array(_)));
	if let Value::Array(arr) = val {
		assert_eq!(arr.len(), 3);
	}

	// Test From<Vec<primitive>>
	let vec_i64 = vec![1i64, 2i64, 3i64];
	let val = Value::from(vec_i64);
	assert!(matches!(val, Value::Array(_)));

	let vec_str = vec![String::from("a"), String::from("b")];
	let val = Value::from(vec_str);
	assert!(matches!(val, Value::Array(_)));
}

#[test]
fn test_value_from_usize() {
	let val = Value::from(42usize);
	// Should be U64 on 64-bit systems
	if cfg!(target_pointer_width = "64") {
		assert!(matches!(val, Value::U64(42)));
	} else {
		assert!(matches!(val, Value::U128(42)));
	}
}

#[test]
#[ignore]
fn test_value_from_datetime() {
	let dt = Utc::now();
	let val = Value::from(dt);
	// Now returns Value::Date instead of Value::String
	assert!(matches!(val, Value::Date(_)));
	if let Value::Date(d) = val {
		// Should be RFC3339 format when converted to string
		let s = d.to_rfc3339();
		assert!(s.contains('T'));
		assert!(s.contains('Z') || s.contains('+'));
	}
}

// ============================================================================
// Equality Tests (PartialEq)
// ============================================================================

#[test]
fn test_value_eq() {
	assert_eq!(Value::I64(1), Value::I64(1));
	assert_eq!(Value::U64(1), Value::U64(1));
	assert_eq!(Value::F64(1.0), Value::F64(1.0));
	assert_eq!(Value::I64(1), Value::U64(1));
	assert_eq!(Value::U64(1), Value::I64(1));
	assert_eq!(Value::I32(1), 1_i32);
	assert_eq!(Value::U32(1), 1_i32);
}

#[test]
fn test_value_cross_type_numeric_equality() {
	// Integer cross-type equality
	assert_eq!(Value::I8(42), Value::I16(42));
	assert_eq!(Value::I8(42), Value::I32(42));
	assert_eq!(Value::U8(42), Value::U16(42));
	assert_eq!(Value::U8(42), Value::I32(42));

	// Float cross-type equality (use value that's exactly representable in both f32 and f64)
	assert_eq!(Value::F32(2.0), Value::F64(2.0));

	// Integer to float equality
	assert_eq!(Value::I32(42), Value::F64(42.0));
	assert_eq!(Value::U64(100), Value::F32(100.0));
}

#[test]
fn test_value_string_equality() {
	let val = Value::String("test".to_string());
	assert_eq!(val, Value::String("test".to_string()));
	assert_eq!(val, String::from("test"));
	assert_eq!(val, "test");
	assert_ne!(val, "other");
}

#[test]
fn test_value_boolean_equality() {
	assert_eq!(Value::Boolean(true), Value::Boolean(true));
	assert_eq!(Value::Boolean(true), true);
	assert_eq!(Value::Boolean(false), false);
	assert_ne!(Value::Boolean(true), Value::Boolean(false));
}

#[test]
fn test_value_array_equality() {
	let arr1 = Value::Array(vec![Value::I32(1), Value::I32(2)]);
	let arr2 = Value::Array(vec![Value::I32(1), Value::I32(2)]);
	let arr3 = Value::Array(vec![Value::I32(1), Value::I32(3)]);

	assert_eq!(arr1, arr2);
	assert_ne!(arr1, arr3);
}

#[test]
fn test_value_empty_equality() {
	assert_eq!(Value::Empty, Value::Empty);
	assert_ne!(Value::Empty, Value::I32(0));
	assert_ne!(Value::Empty, Value::String(String::new()));
}

// ============================================================================
// Ordering Tests (Ord, PartialOrd)
// ============================================================================

#[test]
fn test_value_ordering_integers() {
	assert!(Value::I32(1) < Value::I32(2));
	assert!(Value::I32(2) > Value::I32(1));
	assert!(Value::I32(1) == Value::I32(1));

	// Cross-type integer ordering
	assert!(Value::I8(10) < Value::I32(20));
	assert!(Value::U8(5) < Value::I16(10));
}

#[test]
fn test_value_ordering_floats() {
	assert!(Value::F64(1.5) < Value::F64(2.5));
	assert!(Value::F32(3.14) > Value::F32(2.71));
}

#[test]
fn test_value_ordering_strings() {
	assert!(Value::String("apple".to_string()) < Value::String("banana".to_string()));
	assert!(Value::String("xyz".to_string()) > Value::String("abc".to_string()));
}

#[test]
fn test_value_ordering_empty() {
	// Empty is always less than other values
	assert!(Value::Empty < Value::I32(0));
	assert!(Value::Empty < Value::String(String::new()));
	assert!(Value::Empty < Value::Boolean(false));
	assert_eq!(Value::Empty.cmp(&Value::Empty), Ordering::Equal);
}

#[test]
fn test_value_ordering_mixed_types() {
	// Non-comparable types should return Equal
	assert_eq!(
		Value::String("test".to_string()).cmp(&Value::I32(42)),
		Ordering::Equal
	);
	assert_eq!(Value::Boolean(true).cmp(&Value::F64(3.14)), Ordering::Equal);
}

#[test]
fn test_value_ordering_u128_edge_cases() {
	// Test U128 values that exceed i128::MAX
	let large_u128 = u128::MAX;
	let small_u128 = 100u128;

	assert!(Value::U128(small_u128) < Value::U128(large_u128));
	assert!(Value::U128(large_u128) > Value::U128(small_u128));
}

// ============================================================================
// Value Methods
// ============================================================================

#[test]
fn test_inner_stringify() {
	assert_eq!(Value::String("test".to_string()).inner_stringify(), "test");
	assert_eq!(Value::I32(42).inner_stringify(), "42");
	assert_eq!(Value::F64(3.14).inner_stringify(), "3.14");
	assert_eq!(Value::Boolean(true).inner_stringify(), "true");
	assert_eq!(Value::U64(100).inner_stringify(), "100");
}

#[test]
fn test_try_stringify_primitive_array_errors() {
	let arr = Value::Array(vec![Value::I32(1), Value::I32(2), Value::I32(3)]);
	assert!(arr.try_stringify_primitive().is_err());
}

#[test]
fn test_try_stringify_primitive_object_errors() {
	let mut map = HashMap::new();
	map.insert("key1".to_string(), Value::I32(1));
	map.insert("key2".to_string(), Value::I32(2));

	let obj = Value::Object(map);
	assert!(obj.try_stringify_primitive().is_err());
}

#[test]
fn test_try_stringify_primitive_empty_errors() {
	assert!(Value::Empty.try_stringify_primitive().is_err());
}

#[test]
fn test_to_variant_string() {
	assert_eq!(
		Value::String("test".to_string()).to_variant_string(),
		"String"
	);
	assert_eq!(Value::I32(42).to_variant_string(), "I32");
	assert_eq!(Value::F64(3.14).to_variant_string(), "F64");
	assert_eq!(Value::Boolean(true).to_variant_string(), "Boolean");
	assert_eq!(Value::Empty.to_variant_string(), "Empty");
	assert_eq!(Value::Array(vec![]).to_variant_string(), "Array");
	assert_eq!(Value::Object(HashMap::new()).to_variant_string(), "Object");
}

#[test]
fn test_as_str() {
	let val = Value::String("test".to_string());
	assert_eq!(val.as_str(), "test");
}

#[test]
#[should_panic(expected = "Value::as_str failed")]
fn test_as_str_panics_on_non_string() {
	Value::I32(42).as_str();
}

// ============================================================================
// Serialization/Deserialization
// ============================================================================

#[test]
fn test_json_serialization() {
	// JSON should serialize without enum variant names
	let val = Value::I32(42);
	let json = sonic_rs::to_string(&val).unwrap();
	assert_eq!(json, "42");

	let val = Value::String("test".to_string());
	let json = sonic_rs::to_string(&val).unwrap();
	assert_eq!(json, "\"test\"");

	let val = Value::Boolean(true);
	let json = sonic_rs::to_string(&val).unwrap();
	assert_eq!(json, "true");
}

#[test]
fn test_json_deserialization() {
	let val: Value = sonic_rs::from_str("42").unwrap();
	assert_eq!(val, Value::I64(42)); // JSON integers default to I64

	let val: Value = sonic_rs::from_str("\"test\"").unwrap();
	assert_eq!(val, Value::String("test".to_string()));

	let val: Value = sonic_rs::from_str("true").unwrap();
	assert_eq!(val, Value::Boolean(true));

	let val: Value = sonic_rs::from_str("3.14").unwrap();
	assert!(matches!(val, Value::F64(_)));
}

#[test]
fn test_json_array_serialization() {
	let arr = Value::Array(vec![Value::I32(1), Value::I32(2), Value::I32(3)]);
	let json = sonic_rs::to_string(&arr).unwrap();
	assert_eq!(json, "[1,2,3]");
}

#[test]
fn test_json_object_serialization() {
	let mut map = HashMap::new();
	map.insert("key".to_string(), Value::String("value".to_string()));
	let obj = Value::Object(map);
	let json = sonic_rs::to_string(&obj).unwrap();
	assert!(json.contains("\"key\""));
	assert!(json.contains("\"value\""));
}

#[test]
fn test_postcard_serialization_roundtrip() {
	let test_values = vec![
		Value::String("test".to_string()),
		Value::I32(42),
		Value::F64(3.14),
		Value::Boolean(true),
		Value::U128(u128::MAX),
		Value::Empty,
		Value::Array(vec![Value::I32(1), Value::I32(2)]),
	];

	for val in test_values {
		let encoded = postcard::to_stdvec(&val).unwrap();
		let decoded: Value = postcard::from_bytes(&encoded).unwrap();
		assert_eq!(val, decoded);
	}
}
// ============================================================================
// Type Conversions (Into implementations)
// ============================================================================

#[test]
fn test_value_into_primitives() {
	let val = Value::I32(42);
	let i: i32 = val.into();
	assert_eq!(i, 42);

	let val = Value::F64(3.14);
	let f: f64 = val.into();
	assert_eq!(f, 3.14);

	let val = Value::Boolean(true);
	let b: bool = val.into();
	assert!(b);

	let val = Value::String("test".to_string());
	let s: String = val.into();
	assert_eq!(s, "test");
}

#[test]
fn test_value_into_cross_type_conversion() {
	// I32 to I64
	let val = Value::I32(42);
	let i: i64 = val.into();
	assert_eq!(i, 42);

	// U8 to U32
	let val = Value::U8(255);
	let u: u32 = val.into();
	assert_eq!(u, 255);

	// I32 to F32
	let val = Value::I32(42);
	let f: f32 = val.into();
	assert_eq!(f, 42.0);
}

#[test]
fn test_value_string_parsing_conversion() {
	let val = Value::String("42".to_string());
	let i: i32 = val.into();
	assert_eq!(i, 42);

	let val = Value::String("3.14".to_string());
	let f: f64 = val.into();
	assert_eq!(f, 3.14);
}

#[test]
#[should_panic(expected = "Value is not a string")]
fn test_value_into_string_panics_on_non_string() {
	let val = Value::I32(42);
	let _: String = val.into();
}

#[test]
#[should_panic(expected = "Value cannot be cast to boolean")]
fn test_value_into_bool_panics_on_non_boolean() {
	let val = Value::I32(1);
	let _: bool = val.into();
}

#[test]
fn test_value_into_array() {
	let arr = vec![Value::I32(1), Value::I32(2)];
	let val = Value::Array(arr.clone());
	let result: Vec<Value> = val.into();
	assert_eq!(result, arr);
}

#[test]
#[should_panic(expected = "Value cannot be cast to array")]
fn test_value_into_array_panics_on_non_array() {
	let val = Value::I32(42);
	let _: Vec<Value> = val.into();
}

// ============================================================================
// IntoPrimitive Trait
// ============================================================================

#[test]
fn test_into_primitive() {
	let val = Value::I32(42);
	let i: &i32 = val.into_primitive();
	assert_eq!(*i, 42);

	let val = Value::Boolean(true);
	let b: &bool = val.into_primitive();
	assert!(*b);

	let val = Value::String("test".to_string());
	let s = val.as_str();
	assert_eq!(s, "test");
}

#[test]
#[should_panic(expected = "Value is not an i32")]
fn test_into_primitive_panics_on_wrong_type() {
	let val = Value::I64(42);
	let _: &i32 = val.into_primitive();
}

// ============================================================================
// Edge Cases and UTF-8
// ============================================================================

#[test]
fn test_value_utf8_strings() {
	let utf8_strings = vec![
		"Hello",
		"世界",   // Chinese
		"🚀🌟",   // Emojis
		"Привет", // Russian
		"مرحبا",  // Arabic
		"Ñoño",   // Spanish with tildes
	];

	for s in utf8_strings {
		let val = Value::String(s.to_string());
		assert_eq!(val.inner_stringify(), s);

		// Test serialization roundtrip
		let json = sonic_rs::to_string(&val).unwrap();
		let decoded: Value = sonic_rs::from_str(&json).unwrap();
		assert_eq!(val, decoded);
	}
}

#[test]
fn test_value_large_numbers() {
	let val = Value::U128(u128::MAX);
	assert_eq!(val, Value::U128(u128::MAX));

	let val = Value::I64(i64::MAX);
	assert_eq!(val, Value::I64(i64::MAX));

	let val = Value::I64(i64::MIN);
	assert_eq!(val, Value::I64(i64::MIN));
}

#[test]
fn test_value_nested_arrays() {
	let inner = vec![Value::I32(1), Value::I32(2)];
	let outer = Value::Array(vec![
		Value::Array(inner.clone()),
		Value::Array(inner.clone()),
	]);

	assert!(matches!(outer, Value::Array(_)));
	if let Value::Array(arr) = outer {
		assert_eq!(arr.len(), 2);
		assert!(matches!(arr[0], Value::Array(_)));
	}
}

#[test]
fn test_value_nested_objects() {
	let mut inner = HashMap::new();
	inner.insert("inner_key".to_string(), Value::I32(42));

	let mut outer = HashMap::new();
	outer.insert("outer_key".to_string(), Value::Object(inner));

	let val = Value::Object(outer);
	assert!(matches!(val, Value::Object(_)));
}

#[test]
fn test_value_empty_collections() {
	let empty_arr = Value::Array(vec![]);
	assert!(empty_arr.try_stringify_primitive().is_err());

	let empty_obj = Value::Object(HashMap::new());
	assert!(empty_obj.try_stringify_primitive().is_err());
}

// ============================================================================
// Casting Module
// ============================================================================

#[test]
fn test_cast_to_string() {
	let val = Value::I32(42);
	let result = casting::cast(val, casting::CastType::String);
	assert_eq!(result, Value::String("42".to_string()));
}

#[test]
fn test_cast_between_numeric_types() {
	let val = Value::I32(42);
	let result = casting::cast(val, casting::CastType::I64);
	assert_eq!(result, Value::I64(42));

	let val = Value::F64(3.14);
	let result = casting::cast(val, casting::CastType::F32);
	assert!(matches!(result, Value::F32(_)));
}

#[test]
fn test_cast_to_empty() {
	let val = Value::I32(42);
	let result = casting::cast(val, casting::CastType::Empty);
	assert_eq!(result, Value::Empty);
}

// ============================================================================
// Additional Edge Case Tests for inner_str()
// ============================================================================

#[test]
fn test_inner_str_string_returns_borrowed() {
	let val = Value::String("test".to_string());
	let cow = val.inner_str();
	assert!(matches!(cow, std::borrow::Cow::Borrowed(_)));
	assert_eq!(&*cow, "test");
}

#[test]
fn test_inner_str_numeric_returns_owned() {
	let val = Value::I32(42);
	let cow = val.inner_str();
	assert!(matches!(cow, std::borrow::Cow::Owned(_)));
	assert_eq!(&*cow, "42");
}

#[test]
fn test_inner_str_boolean_returns_owned() {
	let val_true = Value::Boolean(true);
	let cow_true = val_true.inner_str();
	assert!(matches!(cow_true, std::borrow::Cow::Owned(_)));
	assert_eq!(&*cow_true, "true");

	let val_false = Value::Boolean(false);
	let cow_false = val_false.inner_str();
	assert!(matches!(cow_false, std::borrow::Cow::Owned(_)));
	assert_eq!(&*cow_false, "false");
}

#[test]
fn test_inner_str_all_numeric_types() {
	assert_eq!(&*Value::I8(-42).inner_str(), "-42");
	assert_eq!(&*Value::I16(-1000).inner_str(), "-1000");
	assert_eq!(&*Value::I32(-100000).inner_str(), "-100000");
	assert_eq!(&*Value::I64(-1000000000).inner_str(), "-1000000000");
	assert_eq!(&*Value::U8(255).inner_str(), "255");
	assert_eq!(&*Value::U16(65535).inner_str(), "65535");
	assert_eq!(&*Value::U32(4294967295).inner_str(), "4294967295");
	assert_eq!(
		&*Value::U64(18446744073709551615).inner_str(),
		"18446744073709551615"
	);
	assert_eq!(&*Value::U128(u128::MAX).inner_str(), u128::MAX.to_string());
}

#[test]
fn test_inner_str_float_precision() {
	let val = Value::F64(3.141592653589793);
	let cow = val.inner_str();
	assert!(cow.starts_with("3.14159"));
}

#[test]
#[should_panic(expected = "Value::inner_str failed")]
fn test_inner_str_empty_panics() {
	Value::Empty.inner_str();
}

#[test]
#[should_panic(expected = "Value::inner_str failed")]
fn test_inner_str_array_panics() {
	Value::Array(vec![Value::I32(1)]).inner_str();
}

// ============================================================================
// contains() Method Tests
// ============================================================================

#[test]
fn test_contains_string_with_substring() {
	let val = Value::String("hello world".to_string());
	assert!(val.contains("world"));
	assert!(val.contains("hello"));
	assert!(val.contains("o w"));
	assert!(val.contains(""));
}

#[test]
fn test_contains_string_without_substring() {
	let val = Value::String("hello world".to_string());
	assert!(!val.contains("xyz"));
	assert!(!val.contains("World")); // Case sensitive
}

#[test]
fn test_contains_numeric_converted_to_string() {
	let val = Value::I32(12345);
	assert!(val.contains("123"));
	assert!(val.contains("345"));
	assert!(val.contains("12345"));
	assert!(!val.contains("999"));
}

#[test]
fn test_contains_boolean() {
	let val_true = Value::Boolean(true);
	assert!(val_true.contains("true"));
	assert!(val_true.contains("ru"));
	assert!(!val_true.contains("false"));

	let val_false = Value::Boolean(false);
	assert!(val_false.contains("false"));
	assert!(val_false.contains("als"));
}

// ============================================================================
// to_variant_string() Complete Coverage
// ============================================================================

#[test]
fn test_to_variant_string_all_variants() {
	assert_eq!(Value::String("".to_string()).to_variant_string(), "String");
	assert_eq!(Value::F32(0.0).to_variant_string(), "F32");
	assert_eq!(Value::F64(0.0).to_variant_string(), "F64");
	assert_eq!(Value::I8(0).to_variant_string(), "I8");
	assert_eq!(Value::I16(0).to_variant_string(), "I16");
	assert_eq!(Value::I32(0).to_variant_string(), "I32");
	assert_eq!(Value::I64(0).to_variant_string(), "I64");
	assert_eq!(Value::U8(0).to_variant_string(), "U8");
	assert_eq!(Value::U16(0).to_variant_string(), "U16");
	assert_eq!(Value::U32(0).to_variant_string(), "U32");
	assert_eq!(Value::U64(0).to_variant_string(), "U64");
	assert_eq!(Value::U128(0).to_variant_string(), "U128");
	assert_eq!(Value::Boolean(false).to_variant_string(), "Boolean");
	assert_eq!(Value::Empty.to_variant_string(), "Empty");
	assert_eq!(Value::Array(vec![]).to_variant_string(), "Array");
	assert_eq!(Value::Object(HashMap::new()).to_variant_string(), "Object");
}

// ============================================================================
// Float Edge Cases (NaN, Infinity)
// ============================================================================

#[test]
fn test_float_nan_ordering() {
	let nan = Value::F64(f64::NAN);
	let num = Value::F64(1.0);
	// NaN comparisons should return Equal as per the implementation
	assert_eq!(nan.cmp(&num), Ordering::Equal);
	assert_eq!(nan.cmp(&nan), Ordering::Equal);
}

#[test]
fn test_float_infinity() {
	let inf = Value::F64(f64::INFINITY);
	let neg_inf = Value::F64(f64::NEG_INFINITY);
	let num = Value::F64(1000.0);

	assert!(inf > num);
	assert!(neg_inf < num);
	assert!(inf > neg_inf);
}

#[test]
fn test_float_negative_zero() {
	let pos_zero = Value::F64(0.0);
	let neg_zero = Value::F64(-0.0);
	// IEEE 754: 0.0 == -0.0
	assert_eq!(pos_zero, neg_zero);
}

// ============================================================================
// inner_stringify() Complete Coverage
// ============================================================================

#[test]
fn test_inner_stringify_all_numeric_types() {
	assert_eq!(Value::I8(-128).inner_stringify(), "-128");
	assert_eq!(Value::I8(127).inner_stringify(), "127");
	assert_eq!(Value::I16(-32768).inner_stringify(), "-32768");
	assert_eq!(Value::I32(i32::MIN).inner_stringify(), i32::MIN.to_string());
	assert_eq!(Value::I64(i64::MAX).inner_stringify(), i64::MAX.to_string());
	assert_eq!(Value::U8(0).inner_stringify(), "0");
	assert_eq!(Value::U8(255).inner_stringify(), "255");
	assert_eq!(Value::U16(65535).inner_stringify(), "65535");
	assert_eq!(Value::U32(u32::MAX).inner_stringify(), u32::MAX.to_string());
	assert_eq!(Value::U64(u64::MAX).inner_stringify(), u64::MAX.to_string());
	assert_eq!(
		Value::U128(u128::MAX).inner_stringify(),
		u128::MAX.to_string()
	);
}
