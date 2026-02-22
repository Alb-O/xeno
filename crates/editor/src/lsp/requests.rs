//! LSP feature request methods.
//!
//! Provides `LspSystem` wrappers that prepare buffer state (path, language,
//! cursor position, encoding) and delegate to the underlying `ClientHandle`
//! API for each LSP request type.

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

	/// Prepares a URI-only request (no cursor position needed).
	pub(crate) fn prepare_uri_request(&self, buffer: &Buffer) -> xeno_lsp::Result<Option<(xeno_lsp::ClientHandle, xeno_lsp::lsp_types::Uri)>> {
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
		Ok(Some((client, uri)))
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

	pub async fn references(&self, buffer: &Buffer, include_declaration: bool) -> xeno_lsp::Result<Option<Vec<xeno_lsp::lsp_types::Location>>> {
		let Some((client, uri, position)) = self.prepare_position_request(buffer)? else {
			return Ok(None);
		};
		client.references(uri, position, include_declaration).await
	}

	pub async fn document_symbol(&self, buffer: &Buffer) -> xeno_lsp::Result<Option<xeno_lsp::lsp_types::DocumentSymbolResponse>> {
		let Some((client, uri)) = self.prepare_uri_request(buffer)? else {
			return Ok(None);
		};
		client.document_symbol(uri).await
	}

	pub async fn formatting(
		&self,
		buffer: &Buffer,
		options: xeno_lsp::lsp_types::FormattingOptions,
	) -> xeno_lsp::Result<Option<Vec<xeno_lsp::lsp_types::TextEdit>>> {
		let Some((client, uri)) = self.prepare_uri_request(buffer)? else {
			return Ok(None);
		};
		client.formatting(uri, options).await
	}

	pub async fn goto_declaration(&self, buffer: &Buffer) -> xeno_lsp::Result<Option<xeno_lsp::lsp_types::GotoDefinitionResponse>> {
		let Some((client, uri, position)) = self.prepare_position_request(buffer)? else {
			return Ok(None);
		};
		client.goto_declaration(uri, position).await
	}

	pub async fn goto_implementation(&self, buffer: &Buffer) -> xeno_lsp::Result<Option<xeno_lsp::lsp_types::GotoDefinitionResponse>> {
		let Some((client, uri, position)) = self.prepare_position_request(buffer)? else {
			return Ok(None);
		};
		client.goto_implementation(uri, position).await
	}

	pub async fn goto_type_definition(&self, buffer: &Buffer) -> xeno_lsp::Result<Option<xeno_lsp::lsp_types::GotoDefinitionResponse>> {
		let Some((client, uri, position)) = self.prepare_position_request(buffer)? else {
			return Ok(None);
		};
		client.goto_type_definition(uri, position).await
	}

	pub async fn workspace_symbol(&self, buffer: &Buffer, query: String) -> xeno_lsp::Result<Option<xeno_lsp::lsp_types::WorkspaceSymbolResponse>> {
		let Some((client, _)) = self.prepare_uri_request(buffer)? else {
			return Ok(None);
		};
		client.workspace_symbol(query).await
	}

	pub async fn range_formatting(
		&self,
		buffer: &Buffer,
		range: xeno_lsp::lsp_types::Range,
		options: xeno_lsp::lsp_types::FormattingOptions,
	) -> xeno_lsp::Result<Option<Vec<xeno_lsp::lsp_types::TextEdit>>> {
		let Some((client, uri)) = self.prepare_uri_request(buffer)? else {
			return Ok(None);
		};
		client.range_formatting(uri, range, options).await
	}

	pub async fn inlay_hints(&self, buffer: &Buffer, range: xeno_lsp::lsp_types::Range) -> xeno_lsp::Result<Option<Vec<xeno_lsp::lsp_types::InlayHint>>> {
		let Some((client, uri)) = self.prepare_uri_request(buffer)? else {
			return Ok(None);
		};
		client.inlay_hints(uri, range).await
	}
}
