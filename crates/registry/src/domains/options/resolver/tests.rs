use super::*;
use crate::options::keys;

#[test]
fn test_resolve_default() {
	let resolver = OptionResolver::new();
	let options = &crate::db::OPTIONS;
	let tab_width = options.get_key(&keys::TAB_WIDTH.untyped()).unwrap();

	// Should return compile-time default
	let value = resolver.resolve(&tab_width);
	assert_eq!(value.as_int(), Some(4)); // Default is 4
}

#[test]
fn test_resolve_global() {
	let mut global = OptionStore::new();
	let options = &crate::db::OPTIONS;
	let tab_width = options.get_key(&keys::TAB_WIDTH.untyped()).unwrap();
	global.set(tab_width.clone(), OptionValue::Int(8));

	let resolver = OptionResolver::new().with_global(&global);

	assert_eq!(resolver.resolve_int(&tab_width), 8);
}

#[test]
fn test_resolve_language_overrides_global() {
	let mut global = OptionStore::new();
	let options = &crate::db::OPTIONS;
	let tab_width = options.get_key(&keys::TAB_WIDTH.untyped()).unwrap();
	global.set(tab_width.clone(), OptionValue::Int(4));

	let mut language = OptionStore::new();
	language.set(tab_width.clone(), OptionValue::Int(2));

	let resolver = OptionResolver::new().with_global(&global).with_language(&language);

	assert_eq!(resolver.resolve_int(&tab_width), 2);
}

#[test]
fn test_resolve_buffer_overrides_all() {
	let mut global = OptionStore::new();
	let options = &crate::db::OPTIONS;
	let tab_width = options.get_key(&keys::TAB_WIDTH.untyped()).unwrap();
	global.set(tab_width.clone(), OptionValue::Int(4));

	let mut language = OptionStore::new();
	language.set(tab_width.clone(), OptionValue::Int(2));

	let mut buffer = OptionStore::new();
	buffer.set(tab_width.clone(), OptionValue::Int(8));

	let resolver = OptionResolver::new().with_global(&global).with_language(&language).with_buffer(&buffer);

	assert_eq!(resolver.resolve_int(&tab_width), 8);
}

#[test]
fn test_resolve_fallthrough() {
	// Only global has tab_width, only buffer has theme
	let mut global = OptionStore::new();
	let options = &crate::db::OPTIONS;
	let tab_width = options.get_key(&keys::TAB_WIDTH.untyped()).unwrap();
	let theme = options.get_key(&keys::THEME.untyped()).unwrap();

	global.set(tab_width.clone(), OptionValue::Int(4));

	let mut buffer = OptionStore::new();
	buffer.set(theme.clone(), OptionValue::String("monokai".to_string()));

	let resolver = OptionResolver::new().with_global(&global).with_buffer(&buffer);

	// tab_width from global (buffer doesn't have it)
	assert_eq!(resolver.resolve_int(&tab_width), 4);
	// theme from buffer
	assert_eq!(resolver.resolve_string(&theme), "monokai");
}

#[test]
fn test_resolve_string() {
	let mut global = OptionStore::new();
	let options = &crate::db::OPTIONS;
	let theme = options.get_key(&keys::THEME.untyped()).unwrap();
	global.set(theme.clone(), OptionValue::String("nord".to_string()));

	let resolver = OptionResolver::new().with_global(&global);

	assert_eq!(resolver.resolve_string(&theme), "nord");
}

#[test]
fn test_type_mismatch_falls_back_to_default() {
	let mut global = OptionStore::new();
	let options = &crate::db::OPTIONS;
	let tab_width = options.get_key(&keys::TAB_WIDTH.untyped()).unwrap();
	// Incorrectly set an int option with a string value
	global.set(tab_width.clone(), OptionValue::String("bad".to_string()));

	let resolver = OptionResolver::new().with_global(&global);

	// Should fall back to default (4) since type doesn't match
	assert_eq!(resolver.resolve_int(&tab_width), 4);
}
