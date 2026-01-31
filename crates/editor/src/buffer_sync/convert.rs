//! Conversion between [`WireTx`]/[`WireOp`] and [`Transaction`].
//!
//! Editor-side duplicate of `xeno_broker::wire_convert`. Kept separate to
//! avoid adding `xeno-broker` as a dependency of the editor crate.

use xeno_broker_proto::types::{WireOp, WireTx};
use xeno_primitives::transaction::{Change, Operation, Transaction};
use xeno_primitives::{Rope, RopeSlice};

/// Converts a [`Transaction`] into a [`WireTx`] for serialization over IPC.
pub fn tx_to_wire(tx: &Transaction) -> WireTx {
	let ops = tx
		.operations()
		.iter()
		.map(|op| match op {
			Operation::Retain(n) => WireOp::Retain(*n),
			Operation::Delete(n) => WireOp::Delete(*n),
			Operation::Insert(ins) => WireOp::Insert(ins.text.clone()),
		})
		.collect();
	WireTx(ops)
}

/// Converts a [`WireTx`] back into a [`Transaction`] against the given document slice.
pub fn wire_to_tx(wire: &WireTx, doc: RopeSlice<'_>) -> Transaction {
	let mut changes = Vec::new();
	let mut pos: usize = 0;

	for op in &wire.0 {
		match op {
			WireOp::Retain(n) => {
				pos += n;
			}
			WireOp::Delete(n) => {
				changes.push(Change {
					start: pos,
					end: pos + n,
					replacement: None,
				});
				pos += n;
			}
			WireOp::Insert(text) => {
				changes.push(Change {
					start: pos,
					end: pos,
					replacement: Some(text.clone()),
				});
			}
		}
	}

	Transaction::change(doc, changes)
}

/// Computes a minimal [`Transaction`] that transforms `old` into `new`.
///
/// Scans for a common prefix and suffix to isolate the single differing
/// region, then emits one delete+insert [`Change`] for that region.
///
/// Used as a fallback when the undo backend's stored inverse transaction
/// is incomplete (e.g. merged insert-mode groups where only the first
/// keystroke's inverse was recorded).
pub fn rope_delta(old: &Rope, new: &Rope) -> Transaction {
	let old_len = old.len_chars();
	let new_len = new.len_chars();

	let mut prefix = 0;
	let mut old_chars = old.chars();
	let mut new_chars = new.chars();
	loop {
		match (old_chars.next(), new_chars.next()) {
			(Some(a), Some(b)) if a == b => prefix += 1,
			_ => break,
		}
	}

	let max_suffix = (old_len - prefix).min(new_len - prefix);
	let mut suffix = 0;
	while suffix < max_suffix {
		if old.char(old_len - 1 - suffix) != new.char(new_len - 1 - suffix) {
			break;
		}
		suffix += 1;
	}

	let del_end = old_len - suffix;
	let ins_end = new_len - suffix;

	if prefix == del_end && prefix == ins_end {
		return Transaction::change(old.slice(..), std::iter::empty::<Change>());
	}

	let replacement = (prefix < ins_end).then(|| new.slice(prefix..ins_end).to_string());

	Transaction::change(
		old.slice(..),
		std::iter::once(Change {
			start: prefix,
			end: del_end,
			replacement,
		}),
	)
}

#[cfg(test)]
mod tests {
	use super::*;

	fn apply_and_check(old_text: &str, new_text: &str) {
		let old = Rope::from(old_text);
		let new = Rope::from(new_text);
		let tx = rope_delta(&old, &new);
		let mut result = old.clone();
		tx.apply(&mut result);
		assert_eq!(
			result.to_string(),
			new_text,
			"rope_delta({old_text:?} -> {new_text:?}) produced wrong result"
		);
	}

	#[test]
	fn rope_delta_identical() {
		apply_and_check("hello", "hello");
	}

	#[test]
	fn rope_delta_insert_at_end() {
		apply_and_check("hello", "helloabc");
	}

	#[test]
	fn rope_delta_delete_at_end() {
		apply_and_check("helloabc", "hello");
	}

	#[test]
	fn rope_delta_insert_at_start() {
		apply_and_check("world", "hello world");
	}

	#[test]
	fn rope_delta_delete_at_start() {
		apply_and_check("hello world", "world");
	}

	#[test]
	fn rope_delta_replace_middle() {
		apply_and_check("hello world", "hello rust");
	}

	#[test]
	fn rope_delta_empty_to_text() {
		apply_and_check("", "abc");
	}

	#[test]
	fn rope_delta_text_to_empty() {
		apply_and_check("abc", "");
	}

	/// Regression: merged insert-mode group where stored undo_tx only
	/// describes the first keystroke's inverse (Delete(1) instead of Delete(3)).
	#[test]
	fn rope_delta_merged_undo_scenario() {
		apply_and_check("Helloabc", "Hello");
	}

	#[test]
	fn rope_delta_wire_roundtrip() {
		let old = Rope::from("Hello world");
		let new = Rope::from("Hello rust");
		let tx = rope_delta(&old, &new);
		let wire = tx_to_wire(&tx);
		let reconstructed = wire_to_tx(&wire, old.slice(..));
		let mut result = old.clone();
		reconstructed.apply(&mut result);
		assert_eq!(result.to_string(), "Hello rust");
	}
}
