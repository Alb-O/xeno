//! Buffer open/close/save lifecycle operations.

#[cfg(feature = "lsp")]
use super::system::LspSystem;
#[cfg(feature = "lsp")]
use crate::buffer::Buffer;

#[cfg(feature = "lsp")]
impl LspSystem {
	pub async fn on_buffer_will_save(&self, buffer: &Buffer) -> xeno_lsp::Result<()> {
		let Some(path) = buffer.path().map(|p| p.to_path_buf()) else {
			return Ok(());
		};
		let Some(language) = buffer.file_type().map(|s| s.to_string()) else {
			return Ok(());
		};
		let abs_path = self.canonicalize_path(&path);
		self.sync().notify_will_save(&abs_path, &language).await
	}

	pub async fn on_buffer_did_save(&self, buffer: &Buffer, include_text: bool) -> xeno_lsp::Result<()> {
		let Some(path) = buffer.path().map(|p| p.to_path_buf()) else {
			return Ok(());
		};
		let Some(language) = buffer.file_type().map(|s| s.to_string()) else {
			return Ok(());
		};
		let abs_path = self.canonicalize_path(&path);
		let text = buffer.with_doc(|doc| if include_text { Some(doc.content().clone()) } else { None });
		self.sync().notify_did_save(&abs_path, &language, include_text, text.as_ref()).await
	}
}
