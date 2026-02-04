//! Conversion between [`WireTx`]/[`WireOp`] and [`Transaction`].
//!
//! These functions bridge the serializable wire format used by the broker protocol
//! with the internal transaction representation used by `xeno-primitives`.

use xeno_broker_proto::types::{WireOp, WireTx};
use xeno_primitives::transaction::{Change, Operation, Transaction};
use xeno_primitives::{Rope, RopeSlice};

/// Converts a [`Transaction`] into a [`WireTx`] for serialization over IPC.
///
/// Maps each [`Operation`] in the transaction to the corresponding [`WireOp`].
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

/// Validation failures when converting [`WireTx`] to [`Transaction`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WireTxError {
	/// Retain advanced past EOF.
	RetainPastEof {
		/// Cursor position before retain.
		pos: usize,
		/// Retain length.
		n: usize,
		/// Document length in chars.
		len: usize,
	},
	/// Delete advanced past EOF.
	DeletePastEof {
		/// Cursor position before delete.
		pos: usize,
		/// Delete length.
		n: usize,
		/// Document length in chars.
		len: usize,
	},
	/// Cursor arithmetic overflowed.
	Overflow {
		/// Cursor position before overflow.
		pos: usize,
		/// Operation length.
		n: usize,
	},
}

/// Converts a [`WireTx`] back into a [`Transaction`] for application to a rope.
///
/// Returns an error if any wire operation would advance past EOF or overflow.
pub fn wire_to_tx(wire: &WireTx, doc: RopeSlice<'_>) -> Result<Transaction, WireTxError> {
	let mut changes = Vec::new();
	let mut pos: usize = 0;
	let len = doc.len_chars();

	for op in &wire.0 {
		match op {
			WireOp::Retain(n) => {
				let Some(next) = pos.checked_add(*n) else {
					return Err(WireTxError::Overflow { pos, n: *n });
				};
				if next > len {
					return Err(WireTxError::RetainPastEof { pos, n: *n, len });
				}
				pos = next;
			}
			WireOp::Delete(n) => {
				let Some(next) = pos.checked_add(*n) else {
					return Err(WireTxError::Overflow { pos, n: *n });
				};
				if next > len {
					return Err(WireTxError::DeletePastEof { pos, n: *n, len });
				}
				changes.push(Change {
					start: pos,
					end: next,
					replacement: None,
				});
				pos = next;
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

	Ok(Transaction::change(doc, changes))
}

/// Computes a minimal [`Transaction`] that transforms `old` into `new`.
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
	use ropey::Rope;
	use xeno_primitives::transaction::{Change, Transaction};

	use super::*;

	#[test]
	fn round_trip_retain_only() {
		let rope = Rope::from("hello");
		let tx = Transaction::change(rope.slice(..), std::iter::empty::<Change>());
		let wire = tx_to_wire(&tx);
		let reconstructed = wire_to_tx(&wire, rope.slice(..)).expect("wire_to_tx");

		let mut r1 = rope.clone();
		let mut r2 = rope;
		tx.apply(&mut r1);
		reconstructed.apply(&mut r2);
		assert_eq!(r1.to_string(), r2.to_string());
	}

	#[test]
	fn round_trip_insert() {
		let rope = Rope::from("hello");
		let tx = Transaction::change(
			rope.slice(..),
			vec![Change {
				start: 5,
				end: 5,
				replacement: Some(" world".into()),
			}],
		);
		let wire = tx_to_wire(&tx);
		let reconstructed = wire_to_tx(&wire, rope.slice(..)).expect("wire_to_tx");

		let mut r1 = rope.clone();
		let mut r2 = rope;
		tx.apply(&mut r1);
		reconstructed.apply(&mut r2);
		assert_eq!(r1.to_string(), "hello world");
		assert_eq!(r1.to_string(), r2.to_string());
	}

	#[test]
	fn round_trip_delete() {
		let rope = Rope::from("hello world");
		let tx = Transaction::change(
			rope.slice(..),
			vec![Change {
				start: 5,
				end: 11,
				replacement: None,
			}],
		);
		let wire = tx_to_wire(&tx);
		let reconstructed = wire_to_tx(&wire, rope.slice(..)).expect("wire_to_tx");

		let mut r1 = rope.clone();
		let mut r2 = rope;
		tx.apply(&mut r1);
		reconstructed.apply(&mut r2);
		assert_eq!(r1.to_string(), "hello");
		assert_eq!(r1.to_string(), r2.to_string());
	}

	#[test]
	fn round_trip_mixed_ops() {
		let rope = Rope::from("hello world");
		let tx = Transaction::change(
			rope.slice(..),
			vec![
				Change {
					start: 0,
					end: 5,
					replacement: Some("hi".into()),
				},
				Change {
					start: 6,
					end: 11,
					replacement: Some("earth".into()),
				},
			],
		);
		let wire = tx_to_wire(&tx);
		let reconstructed = wire_to_tx(&wire, rope.slice(..)).expect("wire_to_tx");

		let mut r1 = rope.clone();
		let mut r2 = rope;
		tx.apply(&mut r1);
		reconstructed.apply(&mut r2);
		assert_eq!(r1.to_string(), "hi earth");
		assert_eq!(r1.to_string(), r2.to_string());
	}

	#[test]
	fn round_trip_unicode() {
		let rope = Rope::from("héllo wörld");
		let tx = Transaction::change(
			rope.slice(..),
			vec![Change {
				start: 6,
				end: 11,
				replacement: Some("日本語".into()),
			}],
		);
		let wire = tx_to_wire(&tx);
		let reconstructed = wire_to_tx(&wire, rope.slice(..)).expect("wire_to_tx");

		let mut r1 = rope.clone();
		let mut r2 = rope;
		tx.apply(&mut r1);
		reconstructed.apply(&mut r2);
		assert_eq!(r1.to_string(), "héllo 日本語");
		assert_eq!(r1.to_string(), r2.to_string());
	}

	#[test]
	fn wire_ops_are_correct() {
		let rope = Rope::from("abcdef");
		let tx = Transaction::change(
			rope.slice(..),
			vec![Change {
				start: 2,
				end: 4,
				replacement: Some("XY".into()),
			}],
		);
		let wire = tx_to_wire(&tx);
		assert_eq!(
			wire.0,
			vec![
				WireOp::Retain(2),
				WireOp::Insert("XY".into()),
				WireOp::Delete(2),
				WireOp::Retain(2),
			]
		);
	}

	#[test]
	fn test_wire_to_tx_rejects_retain_past_eof() {
		let rope = Rope::from("hello");
		let wire = WireTx(vec![WireOp::Retain(6)]);
		assert_eq!(
			wire_to_tx(&wire, rope.slice(..)).unwrap_err(),
			WireTxError::RetainPastEof {
				pos: 0,
				n: 6,
				len: 5
			}
		);
	}

	#[test]
	fn test_wire_to_tx_rejects_delete_past_eof() {
		let rope = Rope::from("hello");
		let wire = WireTx(vec![WireOp::Retain(4), WireOp::Delete(2)]);
		assert_eq!(
			wire_to_tx(&wire, rope.slice(..)).unwrap_err(),
			WireTxError::DeletePastEof {
				pos: 4,
				n: 2,
				len: 5
			}
		);
	}

	#[test]
	fn test_wire_to_tx_accepts_insert_at_eof() {
		let rope = Rope::from("hello");
		let wire = WireTx(vec![WireOp::Retain(5), WireOp::Insert("x".into())]);
		assert!(wire_to_tx(&wire, rope.slice(..)).is_ok());
	}
}
