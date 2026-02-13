use super::*;

#[test]
fn test_positive_int() {
	assert!(positive_int(&OptionValue::Int(1)).is_ok());
	assert!(positive_int(&OptionValue::Int(100)).is_ok());
	assert!(positive_int(&OptionValue::Int(0)).is_err());
	assert!(positive_int(&OptionValue::Int(-1)).is_err());
	assert!(positive_int(&OptionValue::String("foo".into())).is_err());
}
