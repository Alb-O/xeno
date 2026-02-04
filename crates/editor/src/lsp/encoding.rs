//! LSP offset encoding helpers.

#[cfg(feature = "lsp")]
use std::path::Path;

#[cfg(feature = "lsp")]
use xeno_lsp::OffsetEncoding;
#[cfg(feature = "lsp")]
use xeno_lsp::lsp_types::{TextDocumentSyncCapability, TextDocumentSyncKind};

#[cfg(feature = "lsp")]
use super::document_ops::*;
#[cfg(feature = "lsp")]
use super::system::LspSystem;
#[cfg(feature = "lsp")]
use crate::buffer::Buffer;

#[cfg(feature = "lsp")]
impl LspSystem {
	pub fn incremental_encoding_for_buffer(
		&self,
		buffer: &Buffer,
	) -> Option<xeno_lsp::OffsetEncoding> {
		let path = buffer.path()?;
		let language = buffer.file_type()?;
		self.incremental_encoding(&path, &language)
	}

	pub fn offset_encoding_for_buffer(&self, buffer: &Buffer) -> xeno_lsp::OffsetEncoding {
		let Some(path) = buffer.path() else {
			return OffsetEncoding::Utf16;
		};
		let Some(language) = buffer.file_type() else {
			return OffsetEncoding::Utf16;
		};

		let abs_path = self.canonicalize_path(&path);
		self.sync()
			.registry()
			.get(&language, &abs_path)
			.map(|client| client.offset_encoding())
			.unwrap_or(OffsetEncoding::Utf16)
	}

	fn incremental_encoding(&self, path: &Path, language: &str) -> Option<OffsetEncoding> {
		let abs_path = self.canonicalize_path(path);
		let client = self.sync().registry().get(language, &abs_path)?;
		let caps = client.capabilities()?;
		let supports_incremental = match &caps.text_document_sync {
			Some(TextDocumentSyncCapability::Kind(kind)) => {
				*kind == TextDocumentSyncKind::INCREMENTAL
			}
			Some(TextDocumentSyncCapability::Options(options)) => {
				matches!(options.change, Some(TextDocumentSyncKind::INCREMENTAL))
			}
			None => false,
		};

		if supports_incremental {
			Some(client.offset_encoding())
		} else {
			None
		}
	}
}
