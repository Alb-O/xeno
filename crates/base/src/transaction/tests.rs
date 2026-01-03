use super::Transaction;
use super::changeset::ChangeSet;
use super::types::Change;
use crate::{Rope, Selection};

#[test]
fn test_changeset_retain() {
	let doc = Rope::from("hello");
	let mut cs = ChangeSet::new(doc.slice(..));
	cs.retain(5);
	assert_eq!(cs.len(), 5);
	assert_eq!(cs.len_after(), 5);
}

#[test]
fn test_changeset_delete() {
	let doc = Rope::from("hello");
	let mut cs = ChangeSet::new(doc.slice(..));
	cs.delete(2);
	cs.retain(3);
	assert_eq!(cs.len(), 5);
	assert_eq!(cs.len_after(), 3);
}

#[test]
fn test_changeset_insert() {
	let doc = Rope::from("hello");
	let mut cs = ChangeSet::new(doc.slice(..));
	cs.insert("world".into());
	cs.retain(5);
	assert_eq!(cs.len(), 5);
	assert_eq!(cs.len_after(), 10);
}

#[test]
fn test_changeset_apply() {
	let mut doc = Rope::from("hello");
	let mut cs = ChangeSet::new(doc.slice(..));
	cs.delete(2);
	cs.insert("aa".into());
	cs.retain(3);
	cs.apply(&mut doc);
	assert_eq!(doc.to_string(), "aallo");
}

#[test]
fn test_transaction_insert() {
	let mut doc = Rope::from("hello world");
	let sel = Selection::single(5, 5);
	let tx = Transaction::insert(doc.slice(..), &sel, ",".into());
	tx.apply(&mut doc);
	assert_eq!(doc.to_string(), "hello, world");
}

#[test]
fn test_transaction_delete() {
	let mut doc = Rope::from("hello world");
	let sel = Selection::single(5, 6);
	let tx = Transaction::delete(doc.slice(..), &sel);
	tx.apply(&mut doc);
	assert_eq!(doc.to_string(), "helloworld");
}

#[test]
fn test_transaction_change() {
	let mut doc = Rope::from("hello world");
	let changes = vec![Change {
		start: 0,
		end: 5,
		replacement: Some("hi".into()),
	}];
	let tx = Transaction::change(doc.slice(..), changes);
	tx.apply(&mut doc);
	assert_eq!(doc.to_string(), "hi world");
}

#[test]
fn test_map_selection() {
	let doc = Rope::from("hello world");
	let sel = Selection::single(6, 11);
	let tx = Transaction::change(
		doc.slice(..),
		vec![Change {
			start: 0,
			end: 0,
			replacement: Some("!! ".into()),
		}],
	);
	let mapped = tx.map_selection(&sel);
	assert_eq!(mapped.primary().anchor, 9);
	assert_eq!(mapped.primary().head, 14);
}
