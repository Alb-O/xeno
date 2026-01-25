//! Language configuration loading.
//!
//! Configurations are loaded from bincode blobs compiled at build time.

use thiserror::Error;

use crate::language::{LanguageData, LanguageDataRaw};

/// Errors that can occur when loading language configurations.
#[derive(Debug, Error)]
pub enum LanguageConfigError {
	#[error("failed to deserialize precompiled data: {0}")]
	Bincode(#[from] bincode::Error),
	#[error("invalid precompiled blob (magic/version mismatch)")]
	InvalidBlob,
}

/// Result type for language configuration operations.
pub type Result<T> = std::result::Result<T, LanguageConfigError>;

static LANGUAGES_BIN: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/languages.bin"));

/// Loads language configurations from precompiled bincode.
pub fn load_language_configs() -> Result<Vec<LanguageData>> {
	let payload =
		crate::precompiled::validate_blob(LANGUAGES_BIN).ok_or(LanguageConfigError::InvalidBlob)?;
	let raw: Vec<LanguageDataRaw> = bincode::deserialize(payload)?;
	Ok(raw.into_iter().map(LanguageData::from).collect())
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn load_embedded_languages() {
		let langs = load_language_configs().expect("precompiled languages should load");
		assert!(!langs.is_empty());

		let rust = langs
			.iter()
			.find(|l| l.name == "rust")
			.expect("rust language");
		assert!(rust.extensions.contains(&"rs".to_string()));
		assert!(rust.lsp_servers.contains(&"rust-analyzer".to_string()));
	}
}
