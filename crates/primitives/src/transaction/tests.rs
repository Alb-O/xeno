use proptest::prelude::*;

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

/// Generates a random ASCII document of variable length.
fn arb_document() -> impl Strategy<Value = Rope> {
	"[ -~\n]{0,200}".prop_map(|s| Rope::from(s.as_str()))
}

/// Generates a single non-overlapping change for a document.
fn arb_change(doc_len: usize) -> impl Strategy<Value = Change> {
	if doc_len == 0 {
		Just(Change {
			start: 0,
			end: 0,
			replacement: Some("x".into()),
		})
		.boxed()
	} else {
		(0..=doc_len)
			.prop_flat_map(move |start| {
				(Just(start), start..=doc_len, any::<Option<String>>())
			})
			.prop_map(|(start, end, replacement)| {
				let replacement =
					replacement.map(|s| s.chars().take(50).collect::<String>().into());
				Change {
					start,
					end,
					replacement,
				}
			})
			.boxed()
	}
}

/// Generates a sorted, non-overlapping list of changes for a document.
fn arb_changes(doc_len: usize) -> impl Strategy<Value = Vec<Change>> {
	if doc_len == 0 {
		prop::collection::vec(
			any::<Option<String>>().prop_map(|replacement| {
				let replacement = replacement.map(|s| s.chars().take(20).collect::<String>().into());
				Change {
					start: 0,
					end: 0,
					replacement,
				}
			}),
			0..3,
		)
		.boxed()
	} else {
		prop::collection::vec((0..doc_len, 0..=10usize, any::<Option<String>>()), 0..5)
			.prop_map(move |mut items| {
				// Sort by start position and make non-overlapping
				items.sort_by_key(|(pos, _, _)| *pos);
				let mut changes = Vec::new();
				let mut last_end = 0;

				for (pos, delete_len, replacement) in items {
					let start = pos.max(last_end);
					if start >= doc_len {
						break;
					}
					let end = (start + delete_len).min(doc_len);
					let replacement =
						replacement.map(|s| s.chars().take(20).collect::<String>().into());
					changes.push(Change {
						start,
						end,
						replacement,
					});
					last_end = end;
				}
				changes
			})
			.boxed()
	}
}

proptest! {
	/// Undo round-trip: `apply tx`, then `apply tx.invert()` restores original content.
	#[test]
	fn prop_undo_roundtrip(doc in arb_document()) {
		let doc_len = doc.len_chars();
		let changes = arb_changes(doc_len);

		proptest!(|(changes in changes)| {
			let original = doc.clone();
			let mut modified = doc.clone();

			let tx = Transaction::change(original.slice(..), changes);
			tx.apply(&mut modified);

			let undo_tx = tx.invert(&original);
			undo_tx.apply(&mut modified);

			prop_assert_eq!(
				modified.to_string(),
				original.to_string(),
				"undo should restore original content"
			);
		});
	}

	/// Redo round-trip: `apply tx`, `undo`, `redo` equals post-apply state.
	#[test]
	fn prop_redo_roundtrip(doc in arb_document()) {
		let doc_len = doc.len_chars();
		let changes = arb_changes(doc_len);

		proptest!(|(changes in changes)| {
			let original = doc.clone();
			let mut modified = doc.clone();

			// Apply transaction
			let tx = Transaction::change(original.slice(..), changes);
			tx.apply(&mut modified);
			let after_apply = modified.clone();

			// Undo (invert uses document state before tx was applied)
			let undo_tx = tx.invert(&original);
			undo_tx.apply(&mut modified);

			// Redo (invert uses document state before undo was applied = after_apply)
			let redo_tx = undo_tx.invert(&after_apply);
			redo_tx.apply(&mut modified);

			prop_assert_eq!(
				modified.to_string(),
				after_apply.to_string(),
				"redo should restore post-apply state"
			);
		});
	}

	/// Selection mapping: mapped selection stays within document bounds.
	#[test]
	fn prop_selection_mapping_bounds(doc in arb_document()) {
		let doc_len = doc.len_chars();
		if doc_len == 0 {
			return Ok(());
		}

		let changes = arb_changes(doc_len);
		let selection = (0..doc_len, 0..doc_len).prop_map(|(a, h)| Selection::single(a, h));

		proptest!(|(changes in changes, sel in selection)| {
			let tx = Transaction::change(doc.slice(..), changes.clone());
			let mapped = tx.map_selection(&sel);

			let new_len = {
				let mut test_doc = doc.clone();
				let test_tx = Transaction::change(doc.slice(..), changes);
				test_tx.apply(&mut test_doc);
				test_doc.len_chars()
			};

			for range in mapped.iter() {
				prop_assert!(
					range.anchor <= new_len,
					"mapped anchor {} exceeds doc len {}",
					range.anchor,
					new_len
				);
				prop_assert!(
					range.head <= new_len,
					"mapped head {} exceeds doc len {}",
					range.head,
					new_len
				);
			}
		});
	}

	/// Single insert/delete inversion: simple operations invert correctly.
	#[test]
	fn prop_single_change_invert(doc in arb_document()) {
		let doc_len = doc.len_chars();
		let change = arb_change(doc_len);

		proptest!(|(change in change)| {
			let original = doc.clone();
			let mut modified = doc.clone();

			let tx = Transaction::change(original.slice(..), vec![change]);
			tx.apply(&mut modified);

			let undo_tx = tx.invert(&original);
			undo_tx.apply(&mut modified);

			prop_assert_eq!(
				modified.to_string(),
				original.to_string(),
				"single change undo should restore original"
			);
		});
	}
}
