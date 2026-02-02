//! Shared wire types for xeno-broker IPC.
//!
//! This crate defines the protocol messages exchanged between the editor and the broker
//! over Unix domain sockets. The protocol uses binary framing with postcard encoding
//! for efficiency.

#![warn(missing_docs)]

pub mod paths;
pub mod protocol;
pub mod types;

pub use protocol::BrokerProtocol;
pub use types::*;

/// Computes a canonical fingerprint for a rope.
///
/// Returns a tuple of `(length_in_characters, xxh3_64_hash)`.
/// The hash is computed over the full UTF-8 byte stream of the rope.
pub fn fingerprint_rope(rope: &ropey::Rope) -> (u64, u64) {
	let len = rope.len_chars() as u64;
	let mut hasher = xxhash_rust::xxh3::Xxh3::new();
	for chunk in rope.chunks() {
		hasher.update(chunk.as_bytes());
	}
	(len, hasher.digest())
}

#[cfg(test)]
mod tests {
	use ropey::Rope;

	use super::*;

	#[test]
	fn test_fingerprint_rope_consistency() {
		let s = "hello 🦀 world";
		let rope = Rope::from_str(s);
		let (len, hash) = fingerprint_rope(&rope);

		assert_eq!(len, s.chars().count() as u64);
		assert_eq!(hash, xxhash_rust::xxh3::xxh3_64(s.as_bytes()));
	}

	#[test]
	fn test_fingerprint_rope_incremental() {
		let mut rope = Rope::from_str("base content");
		rope.insert(5, " shiny");
		let (len, hash) = fingerprint_rope(&rope);

		let s = rope.to_string();
		assert_eq!(len, s.chars().count() as u64);
		assert_eq!(hash, xxhash_rust::xxh3::xxh3_64(s.as_bytes()));
	}
}
