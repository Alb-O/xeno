use super::*;
use crate::options::keys;

#[test]
fn test_set_and_get() {
	let mut store = OptionStore::new();
	let options = &crate::db::OPTIONS;
	let tab_width = options.get_key(&keys::TAB_WIDTH.untyped()).unwrap();
	store.set(tab_width.clone(), OptionValue::Int(8));

	assert_eq!(store.get(tab_width.dense_id()), Some(&OptionValue::Int(8)));
	assert_eq!(store.get_int(tab_width.dense_id()), Some(8));
}

#[test]
fn test_get_missing() {
	let store = OptionStore::new();
	let options = &crate::db::OPTIONS;
	let tab_width = options.get_key(&keys::TAB_WIDTH.untyped()).unwrap();
	assert_eq!(store.get(tab_width.dense_id()), None);
	assert_eq!(store.get_int(tab_width.dense_id()), None);
}

#[test]
fn test_set_by_kdl() {
	let mut store = OptionStore::new();
	let options = &crate::db::OPTIONS;
	let tab_width = options.get_key(&keys::TAB_WIDTH.untyped()).unwrap();
	store
		.set_by_kdl(options, "tab-width", OptionValue::Int(2))
		.unwrap();

	assert_eq!(store.get_int(tab_width.dense_id()), Some(2));
}

#[test]
fn test_set_by_kdl_unknown() {
	let mut store = OptionStore::new();
	let options = &crate::db::OPTIONS;
	let result = store.set_by_kdl(options, "unknown-option", OptionValue::Int(1));

	assert!(matches!(result, Err(OptionError::UnknownOption(_))));
}

#[test]
fn test_merge() {
	let mut store1 = OptionStore::new();
	let options = &crate::db::OPTIONS;
	let tab_width = options.get_key(&keys::TAB_WIDTH.untyped()).unwrap();
	let theme = options.get_key(&keys::THEME.untyped()).unwrap();

	store1.set(tab_width.clone(), OptionValue::Int(4));
	store1.set(theme.clone(), OptionValue::String("gruvbox".to_string()));

	let mut store2 = OptionStore::new();
	store2.set(tab_width.clone(), OptionValue::Int(2));

	store1.merge(&store2);

	// store2's value wins
	assert_eq!(store1.get_int(tab_width.dense_id()), Some(2));
	// store1's value preserved
	assert_eq!(store1.get_string(theme.dense_id()), Some("gruvbox"));
}

#[test]
fn test_remove() {
	let mut store = OptionStore::new();
	let options = &crate::db::OPTIONS;
	let tab_width = options.get_key(&keys::TAB_WIDTH.untyped()).unwrap();
	store.set(tab_width.clone(), OptionValue::Int(4));

	let removed = store.remove(tab_width.clone());
	assert_eq!(removed, Some(OptionValue::Int(4)));
	assert_eq!(store.get(tab_width.dense_id()), None);
}

#[test]
fn test_len_and_is_empty() {
	let mut store = OptionStore::new();
	let options = &crate::db::OPTIONS;
	let tab_width = options.get_key(&keys::TAB_WIDTH.untyped()).unwrap();
	assert!(store.is_empty());
	assert_eq!(store.len(), 0);

	store.set(tab_width.clone(), OptionValue::Int(4));
	assert!(!store.is_empty());
	assert_eq!(store.len(), 1);
}

#[test]
fn test_string_option() {
	let mut store = OptionStore::new();
	let options = &crate::db::OPTIONS;
	let theme = options.get_key(&keys::THEME.untyped()).unwrap();
	store.set(theme.clone(), OptionValue::String("monokai".to_string()));

	assert_eq!(store.get_string(theme.dense_id()), Some("monokai"));
}
