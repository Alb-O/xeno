//! Conversion between [`WireTx`]/[`WireOp`] and [`Transaction`].
//!
//! These functions bridge the serializable wire format used by the broker protocol
//! with the internal transaction representation used by `xeno-primitives`.

use ropey::Rope;
use xeno_broker_proto::types::{WireOp, WireTx};
use xeno_primitives::transaction::{Change, Operation, Transaction};

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

/// Converts a [`WireTx`] back into a [`Transaction`] for application to a rope.
///
/// Reconstructs a transaction by translating the wire operations into a sequence
/// of [`Change`] items and building the transaction against the given rope slice.
pub fn wire_to_tx(wire: &WireTx, doc: ropey::RopeSlice<'_>) -> Transaction {
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
		let reconstructed = wire_to_tx(&wire, rope.slice(..));

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
		let reconstructed = wire_to_tx(&wire, rope.slice(..));

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
		let reconstructed = wire_to_tx(&wire, rope.slice(..));

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
		let reconstructed = wire_to_tx(&wire, rope.slice(..));

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
		let reconstructed = wire_to_tx(&wire, rope.slice(..));

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
}
