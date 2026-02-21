//! LSP feature request methods (hover, goto-definition).

#[cfg(feature = "lsp")]
use super::system::LspSystem;
#[cfg(feature = "lsp")]
use crate::buffer::Buffer;

#[cfg(feature = "lsp")]
impl LspSystem {
	pub(crate) fn prepare_position_request(
		&self,
		buffer: &Buffer,
	) -> xeno_lsp::Result<Option<(xeno_lsp::ClientHandle, xeno_lsp::lsp_types::Uri, xeno_lsp::lsp_types::Position)>> {
		let Some(path) = buffer.path() else {
			return Ok(None);
		};
		let Some(language) = buffer.file_type() else {
			return Ok(None);
		};

		let abs_path = self.canonicalize_path(&path);

		let Some(client) = self.sync().registry().get(&language, &abs_path) else {
			return Ok(None);
		};
		if !client.is_ready() {
			return Ok(None);
		}

		let uri = xeno_lsp::uri_from_path(&abs_path).ok_or_else(|| xeno_lsp::Error::Protocol("Invalid path".into()))?;

		let encoding = client.offset_encoding();
		let position = buffer
			.with_doc(|doc| xeno_lsp::char_to_lsp_position(doc.content(), buffer.cursor, encoding))
			.ok_or_else(|| xeno_lsp::Error::Protocol("Invalid position".into()))?;

		Ok(Some((client, uri, position)))
	}

	pub async fn hover(&self, buffer: &Buffer) -> xeno_lsp::Result<Option<xeno_lsp::lsp_types::Hover>> {
		let Some((client, uri, position)) = self.prepare_position_request(buffer)? else {
			return Ok(None);
		};
		client.hover(uri, position).await
	}

	pub async fn goto_definition(&self, buffer: &Buffer) -> xeno_lsp::Result<Option<xeno_lsp::lsp_types::GotoDefinitionResponse>> {
		let Some((client, uri, position)) = self.prepare_position_request(buffer)? else {
			return Ok(None);
		};
		client.goto_definition(uri, position).await
	}
}
