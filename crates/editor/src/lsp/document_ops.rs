//! Buffer open/close/save lifecycle operations.

#[cfg(feature = "lsp")]
use super::system::LspSystem;
#[cfg(feature = "lsp")]
use crate::buffer::Buffer;

#[cfg(feature = "lsp")]
impl LspSystem {
	pub(super) fn canonicalize_path(&self, path: &std::path::Path) -> std::path::PathBuf {
		path.canonicalize()
			.unwrap_or_else(|_| std::env::current_dir().unwrap_or_default().join(path))
	}

	pub async fn on_buffer_open(
		&self,
		buffer: &Buffer,
	) -> xeno_lsp::Result<Option<xeno_lsp::ClientHandle>> {
		let Some(path) = buffer.path() else {
			return Ok(None);
		};
		let Some(language) = &buffer.file_type() else {
			return Ok(None);
		};

		if self.registry().get_config(language).is_none() {
			return Ok(None);
		}

		let abs_path = self.canonicalize_path(&path);

		let content = buffer.with_doc(|doc| doc.content().clone());
		let client = self
			.sync()
			.open_document(&abs_path, language, &content)
			.await?;
		Ok(Some(client))
	}

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

	pub async fn on_buffer_did_save(
		&self,
		buffer: &Buffer,
		include_text: bool,
	) -> xeno_lsp::Result<()> {
		let Some(path) = buffer.path().map(|p| p.to_path_buf()) else {
			return Ok(());
		};
		let Some(language) = buffer.file_type().map(|s| s.to_string()) else {
			return Ok(());
		};
		let abs_path = self.canonicalize_path(&path);
		let text = buffer.with_doc(|doc| {
			if include_text {
				Some(doc.content().clone())
			} else {
				None
			}
		});
		self.sync()
			.notify_did_save(&abs_path, &language, include_text, text.as_ref())
			.await
	}

	pub async fn on_buffer_close(&self, buffer: &Buffer) -> xeno_lsp::Result<()> {
		let Some(path) = buffer.path().map(|p| p.to_path_buf()) else {
			return Ok(());
		};
		let Some(language) = buffer.file_type().map(|s| s.to_string()) else {
			return Ok(());
		};
		let abs_path = self.canonicalize_path(&path);
		self.sync().close_document(&abs_path, &language).await
	}
}
