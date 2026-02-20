//! LSP-related messages.
//!
//! LSP catalog loading is split into two phases: a background task reads the
//! immutable registry-backed catalog and builds `(language, config)` pairs, then sends
//! `CatalogReady` with the data. The editor thread validates the token
//! (latest-wins) before registering configs, ensuring stale loads never
//! overwrite a newer catalog.

use super::Dirty;
use crate::Editor;

/// Resolved LSP server configuration for a single language.
///
/// This is the data-only payload extracted by the background loader. The
/// editor thread registers these into the LSP registry after token validation.
#[cfg(feature = "lsp")]
pub type LspCatalogConfig = (String, xeno_lsp::LanguageServerConfig);

/// Messages for LSP lifecycle events.
pub enum LspMsg {
	/// Background LSP catalog loading completed.
	///
	/// Carries a token for latest-wins gating and the parsed language server
	/// configurations to register.
	CatalogReady {
		token: u64,
		#[cfg(feature = "lsp")]
		configs: Vec<LspCatalogConfig>,
	},
	/// A server failed to start.
	ServerFailed { language: String, error: String },
}

impl std::fmt::Debug for LspMsg {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		match self {
			#[cfg(feature = "lsp")]
			Self::CatalogReady { token, configs } => f
				.debug_struct("CatalogReady")
				.field("token", token)
				.field("configs_count", &configs.len())
				.finish(),
			#[cfg(not(feature = "lsp"))]
			Self::CatalogReady { token } => f.debug_struct("CatalogReady").field("token", token).finish(),
			Self::ServerFailed { language, error } => f.debug_struct("ServerFailed").field("language", language).field("error", error).finish(),
		}
	}
}

impl LspMsg {
	/// Applies this message to editor state, returning dirty flags.
	///
	/// Validates the token against the pending LSP catalog load. Stale
	/// completions (superseded by a newer `kick_lsp_catalog_load`) are
	/// silently ignored.
	pub fn apply(self, editor: &mut Editor) -> Dirty {
		match self {
			#[cfg(feature = "lsp")]
			Self::CatalogReady { token, configs } => {
				if editor.state.async_state.pending_lsp_catalog_load_token != Some(token) {
					tracing::debug!(token, "Ignoring stale LSP catalog load");
					return Dirty::NONE;
				}
				editor.state.async_state.pending_lsp_catalog_load_token = None;

				for (language, config) in configs {
					editor.state.integration.lsp.registry().register(language, config);
				}

				tracing::debug!("LSP catalog ready, initializing for open buffers");
				editor.state.config.lsp_catalog_ready = true;
				editor.kick_lsp_init_for_open_buffers();
				Dirty::NONE
			}
			#[cfg(not(feature = "lsp"))]
			Self::CatalogReady { token } => {
				if editor.state.async_state.pending_lsp_catalog_load_token != Some(token) {
					tracing::debug!(token, "Ignoring stale LSP catalog load");
					return Dirty::NONE;
				}
				editor.state.async_state.pending_lsp_catalog_load_token = None;
				editor.state.config.lsp_catalog_ready = true;
				Dirty::NONE
			}
			Self::ServerFailed { language, error } => {
				tracing::warn!(language, error, "LSP server failed");
				Dirty::NONE
			}
		}
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn lsp_catalog_stale_token_does_not_register() {
		let mut editor = Editor::new_scratch();
		editor.state.async_state.pending_lsp_catalog_load_token = Some(2);

		// Stale load (token=1) arrives — should be ignored.
		let stale = LspMsg::CatalogReady {
			token: 1,
			#[cfg(feature = "lsp")]
			configs: vec![(
				"rust".to_string(),
				xeno_lsp::LanguageServerConfig {
					command: "rust-analyzer".to_string(),
					..Default::default()
				},
			)],
		};
		let dirty = stale.apply(&mut editor);

		assert_eq!(dirty, Dirty::NONE, "stale token should produce Dirty::NONE");
		assert!(!editor.state.config.lsp_catalog_ready, "catalog should not be marked ready from stale load");
		assert_eq!(editor.state.async_state.pending_lsp_catalog_load_token, Some(2), "pending token should remain");
	}

	#[test]
	fn lsp_catalog_latest_wins_even_if_completion_order_reversed() {
		let mut editor = Editor::new_scratch();
		editor.state.async_state.pending_lsp_catalog_load_token = Some(5);

		// Stale load (token=3) → ignored.
		let stale = LspMsg::CatalogReady {
			token: 3,
			#[cfg(feature = "lsp")]
			configs: vec![],
		};
		let dirty = stale.apply(&mut editor);
		assert_eq!(dirty, Dirty::NONE);
		assert_eq!(editor.state.async_state.pending_lsp_catalog_load_token, Some(5));

		// Current load (token=5) → accepted.
		let current = LspMsg::CatalogReady {
			token: 5,
			#[cfg(feature = "lsp")]
			configs: vec![],
		};
		let dirty = current.apply(&mut editor);
		assert_eq!(dirty, Dirty::NONE);
		assert!(editor.state.config.lsp_catalog_ready, "catalog should be marked ready");
		assert_eq!(editor.state.async_state.pending_lsp_catalog_load_token, None);
	}
}
