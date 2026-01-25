//! Precompiled asset blob handling.
//!
//! Provides constants and utilities for loading build-time compiled KDL data.

use std::mem::size_of;

/// Magic bytes identifying a Xeno precompiled asset blob.
pub const MAGIC: &[u8; 8] = b"XENOASST";

/// Schema version for blob format compatibility.
pub const SCHEMA_VERSION: u32 = 1;

/// Total header size in bytes (magic + version).
pub const HEADER_SIZE: usize = MAGIC.len() + size_of::<u32>();

/// Validates blob header and returns payload slice.
///
/// Returns `None` if magic or version mismatch.
pub fn validate_blob(data: &[u8]) -> Option<&[u8]> {
	if data.len() < HEADER_SIZE {
		return None;
	}
	if &data[..8] != MAGIC {
		return None;
	}
	let version = u32::from_le_bytes(data[8..12].try_into().ok()?);
	if version != SCHEMA_VERSION {
		return None;
	}
	Some(&data[HEADER_SIZE..])
}
