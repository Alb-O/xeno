//! File operations (save, load).
//!
//! Implements [`FileOpsAccess`] for the [`Editor`].

use std::path::PathBuf;

use evildoer_registry::{emit as emit_hook, HookContext, HookEventData};
use evildoer_registry::commands::CommandError;

use super::Editor;

impl evildoer_manifest::editor_ctx::FileOpsAccess for Editor {
	fn is_modified(&self) -> bool {
		// Panels are never considered "modified" for save purposes
		if self.is_panel_focused() {
			return false;
		}
		self.buffer().modified()
	}

	fn save(
		&mut self,
	) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<(), CommandError>> + '_>> {
		Box::pin(async move {
			if self.is_panel_focused() {
				return Err(CommandError::InvalidArgument(
					"Cannot save a panel".to_string(),
				));
			}

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

			let mut content = Vec::new();
			for chunk in self.buffer().doc().content.chunks() {
				content.extend_from_slice(chunk.as_bytes());
			}

			tokio::fs::write(&path_owned, &content)
				.await
				.map_err(|e| CommandError::Io(e.to_string()))?;

			self.buffer_mut().set_modified(false);
			self.notify("info", format!("Saved {}", path_owned.display()));

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
		if self.is_panel_focused() {
			return Box::pin(async {
				Err(CommandError::InvalidArgument(
					"Cannot save a panel".to_string(),
				))
			});
		}

		self.buffer_mut().set_path(Some(path));
		self.save()
	}
}
