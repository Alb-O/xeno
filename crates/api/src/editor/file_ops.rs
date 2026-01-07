//! File operations (save, load).
//!
//! Implements [`FileOpsAccess`] for the [`Editor`].

use std::path::PathBuf;

use xeno_registry::commands::CommandError;
use xeno_registry::{HookContext, HookEventData, emit as emit_hook};

use super::Editor;

impl xeno_core::editor_ctx::FileOpsAccess for Editor {
	fn is_modified(&self) -> bool {
		self.buffer().modified()
	}

	fn save(
		&mut self,
	) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<(), CommandError>> + '_>> {
		Box::pin(async move {
			let path_owned = match &self.buffer().path() {
				Some(p) => p.clone(),
				None => {
					return Err(CommandError::InvalidArgument(
						"No filename. Use :write <filename>".to_string(),
					));
				}
			};

			let text_slice = self.buffer().doc().content.clone();
			emit_hook(&HookContext::new(
				HookEventData::BufferWritePre {
					path: &path_owned,
					text: text_slice.slice(..),
				},
				Some(&self.extensions),
			))
			.await;

			#[cfg(feature = "lsp")]
			if let Err(e) = self.lsp.on_buffer_will_save(self.buffer()) {
				tracing::warn!(error = %e, "LSP will_save notification failed");
			}

			let mut content = Vec::new();
			for chunk in self.buffer().doc().content.chunks() {
				content.extend_from_slice(chunk.as_bytes());
			}

			tokio::fs::write(&path_owned, &content)
				.await
				.map_err(|e| CommandError::Io(e.to_string()))?;

			self.buffer_mut().set_modified(false);
			self.show_notification(xeno_registry_notifications::keys::file_saved::call(
				&path_owned,
			));

			#[cfg(feature = "lsp")]
			if let Err(e) = self.lsp.on_buffer_did_save(self.buffer(), true) {
				tracing::warn!(error = %e, "LSP did_save notification failed");
			}

			emit_hook(&HookContext::new(
				HookEventData::BufferWrite { path: &path_owned },
				Some(&self.extensions),
			))
			.await;

			Ok(())
		})
	}

	fn save_as(
		&mut self,
		path: PathBuf,
	) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<(), CommandError>> + '_>> {
		self.buffer_mut().set_path(Some(path));
		self.save()
	}
}
