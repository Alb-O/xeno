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

	/// Sends `workspace/willRenameFiles` to the server for the buffer's language,
	/// returning any workspace edit the server wants applied before the rename.
	pub async fn will_rename_file(
		&self,
		buffer: &Buffer,
		old_path: &std::path::Path,
		new_path: &std::path::Path,
	) -> xeno_lsp::Result<Option<xeno_lsp::lsp_types::WorkspaceEdit>> {
		let Some(language) = buffer.file_type() else {
			return Ok(None);
		};
		let abs_old = self.canonicalize_path(old_path);
		let Some(client) = self.sync().registry().get(&language, &abs_old) else {
			return Ok(None);
		};
		if !client.is_ready() {
			return Ok(None);
		}
		let old_uri = xeno_lsp::uri_from_path(&abs_old)
			.ok_or_else(|| xeno_lsp::Error::Protocol("Invalid old path".into()))?
			.to_string();
		let abs_new = self.canonicalize_path(new_path);
		let new_uri = xeno_lsp::uri_from_path(&abs_new)
			.ok_or_else(|| xeno_lsp::Error::Protocol("Invalid new path".into()))?
			.to_string();
		client.will_rename_files(vec![xeno_lsp::lsp_types::FileRename { old_uri, new_uri }]).await
	}

	/// Sends `workspace/didRenameFiles` notification to the server for the
	/// buffer's language.
	pub async fn did_rename_file(&self, language: &str, old_path: &std::path::Path, new_path: &std::path::Path) -> xeno_lsp::Result<()> {
		let abs_new = self.canonicalize_path(new_path);
		let Some(client) = self.sync().registry().get(language, &abs_new) else {
			return Ok(());
		};
		if !client.is_ready() {
			return Ok(());
		}
		let old_uri = xeno_lsp::uri_from_path(&self.canonicalize_path(old_path))
			.ok_or_else(|| xeno_lsp::Error::Protocol("Invalid old path".into()))?
			.to_string();
		let new_uri = xeno_lsp::uri_from_path(&abs_new)
			.ok_or_else(|| xeno_lsp::Error::Protocol("Invalid new path".into()))?
			.to_string();
		client.did_rename_files(vec![xeno_lsp::lsp_types::FileRename { old_uri, new_uri }]).await
	}
}
