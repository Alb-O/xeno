//! Conversion between [`WireTx`]/[`WireOp`] and [`Transaction`].
//!
//! Editor-side duplicate of `xeno_broker::wire_convert`. Kept separate to
//! avoid adding `xeno-broker` as a dependency of the editor crate.

use xeno_broker_proto::types::{WireOp, WireTx};
use xeno_primitives::RopeSlice;
use xeno_primitives::transaction::{Change, Operation, Transaction};

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
