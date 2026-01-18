use super::*;
use crate::keys;

#[test]
fn test_set_and_get() {
	let mut store = OptionStore::new();
	store.set(keys::TAB_WIDTH.untyped(), OptionValue::Int(8));

	assert_eq!(
		store.get(keys::TAB_WIDTH.untyped()),
		Some(&OptionValue::Int(8))
	);
	assert_eq!(store.get_int(keys::TAB_WIDTH.untyped()), Some(8));
}

#[test]
fn test_get_missing() {
	let store = OptionStore::new();
	assert_eq!(store.get(keys::TAB_WIDTH.untyped()), None);
	assert_eq!(store.get_int(keys::TAB_WIDTH.untyped()), None);
}

#[test]
fn test_set_by_kdl() {
	let mut store = OptionStore::new();
	store.set_by_kdl("tab-width", OptionValue::Int(2)).unwrap();

	assert_eq!(store.get_int(keys::TAB_WIDTH.untyped()), Some(2));
}

#[test]
fn test_set_by_kdl_unknown() {
	let mut store = OptionStore::new();
	let result = store.set_by_kdl("unknown-option", OptionValue::Int(1));

	assert!(matches!(result, Err(OptionError::UnknownOption(_))));
}

#[test]
fn test_merge() {
	let mut store1 = OptionStore::new();
	store1.set(keys::TAB_WIDTH.untyped(), OptionValue::Int(4));
	store1.set(
		keys::THEME.untyped(),
		OptionValue::String("gruvbox".to_string()),
	);

	let mut store2 = OptionStore::new();
	store2.set(keys::TAB_WIDTH.untyped(), OptionValue::Int(2));

	store1.merge(&store2);

	// store2's value wins
	assert_eq!(store1.get_int(keys::TAB_WIDTH.untyped()), Some(2));
	// store1's value preserved
	assert_eq!(store1.get_string(keys::THEME.untyped()), Some("gruvbox"));
}

#[test]
fn test_remove() {
	let mut store = OptionStore::new();
	store.set(keys::TAB_WIDTH.untyped(), OptionValue::Int(4));

	let removed = store.remove(keys::TAB_WIDTH.untyped());
	assert_eq!(removed, Some(OptionValue::Int(4)));
	assert_eq!(store.get(keys::TAB_WIDTH.untyped()), None);
}

#[test]
fn test_len_and_is_empty() {
	let mut store = OptionStore::new();
	assert!(store.is_empty());
	assert_eq!(store.len(), 0);

	store.set(keys::TAB_WIDTH.untyped(), OptionValue::Int(4));
	assert!(!store.is_empty());
	assert_eq!(store.len(), 1);
}

#[test]
fn test_string_option() {
	let mut store = OptionStore::new();
	store.set(
		keys::THEME.untyped(),
		OptionValue::String("monokai".to_string()),
	);

	assert_eq!(store.get_string(keys::THEME.untyped()), Some("monokai"));
}
