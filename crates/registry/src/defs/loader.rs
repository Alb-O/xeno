pub use super::blob_header::{MAGIC, SCHEMA_VERSION};

/// Total header size in bytes (magic + version).
pub const HEADER_SIZE: usize = MAGIC.len() + std::mem::size_of::<u32>();

/// Validates blob header and returns payload slice.
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

/// Deserializes a blob with header validation.
pub fn load_blob<T: serde::de::DeserializeOwned>(data: &[u8], name: &str) -> T {
	let payload = validate_blob(data).unwrap_or_else(|| panic!("invalid {name} blob header"));
	postcard::from_bytes(payload).unwrap_or_else(|e| panic!("failed to deserialize {name} blob: {e}"))
}
