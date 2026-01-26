use super::*;
use crate::options::keys;

#[test]
fn test_resolve_default() {
	let resolver = OptionResolver::new();

	// Should return compile-time default
	let value = resolver.resolve(keys::TAB_WIDTH.untyped());
	assert_eq!(value.as_int(), Some(4)); // Default is 4
}

#[test]
fn test_resolve_global() {
	let mut global = OptionStore::new();
	global.set(keys::TAB_WIDTH.untyped(), OptionValue::Int(8));

	let resolver = OptionResolver::new().with_global(&global);

	assert_eq!(resolver.resolve_int(keys::TAB_WIDTH.untyped()), 8);
}

#[test]
fn test_resolve_language_overrides_global() {
	let mut global = OptionStore::new();
	global.set(keys::TAB_WIDTH.untyped(), OptionValue::Int(4));

	let mut language = OptionStore::new();
	language.set(keys::TAB_WIDTH.untyped(), OptionValue::Int(2));

	let resolver = OptionResolver::new()
		.with_global(&global)
		.with_language(&language);

	assert_eq!(resolver.resolve_int(keys::TAB_WIDTH.untyped()), 2);
}

#[test]
fn test_resolve_buffer_overrides_all() {
	let mut global = OptionStore::new();
	global.set(keys::TAB_WIDTH.untyped(), OptionValue::Int(4));

	let mut language = OptionStore::new();
	language.set(keys::TAB_WIDTH.untyped(), OptionValue::Int(2));

	let mut buffer = OptionStore::new();
	buffer.set(keys::TAB_WIDTH.untyped(), OptionValue::Int(8));

	let resolver = OptionResolver::new()
		.with_global(&global)
		.with_language(&language)
		.with_buffer(&buffer);

	assert_eq!(resolver.resolve_int(keys::TAB_WIDTH.untyped()), 8);
}

#[test]
fn test_resolve_fallthrough() {
	// Only global has tab_width, only buffer has theme
	let mut global = OptionStore::new();
	global.set(keys::TAB_WIDTH.untyped(), OptionValue::Int(4));

	let mut buffer = OptionStore::new();
	buffer.set(
		keys::THEME.untyped(),
		OptionValue::String("monokai".to_string()),
	);

	let resolver = OptionResolver::new()
		.with_global(&global)
		.with_buffer(&buffer);

	// tab_width from global (buffer doesn't have it)
	assert_eq!(resolver.resolve_int(keys::TAB_WIDTH.untyped()), 4);
	// theme from buffer
	assert_eq!(resolver.resolve_string(keys::THEME.untyped()), "monokai");
}

#[test]
fn test_resolve_string() {
	let mut global = OptionStore::new();
	global.set(
		keys::THEME.untyped(),
		OptionValue::String("nord".to_string()),
	);

	let resolver = OptionResolver::new().with_global(&global);

	assert_eq!(resolver.resolve_string(keys::THEME.untyped()), "nord");
}

#[test]
fn test_type_mismatch_falls_back_to_default() {
	let mut global = OptionStore::new();
	// Incorrectly set an int option with a string value
	global.set(
		keys::TAB_WIDTH.untyped(),
		OptionValue::String("bad".to_string()),
	);

	let resolver = OptionResolver::new().with_global(&global);

	// Should fall back to default (4) since type doesn't match
	assert_eq!(resolver.resolve_int(keys::TAB_WIDTH.untyped()), 4);
}
