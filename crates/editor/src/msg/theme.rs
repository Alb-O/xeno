//! Theme-related messages.

use super::Dirty;
use crate::Editor;

/// Messages for theme configuration refresh.
///
/// Theme loading is split into two phases: a background task collects and
/// deduplicates theme definitions from disk, then sends `ThemesReady` with the
/// parsed data. The editor thread validates the token (latest-wins), refreshes
/// configured theme state, and reports parse errors.
pub enum ThemeMsg {
	/// Background theme loading completed.
	///
	/// Carries a token for latest-wins gating, the parsed theme definitions,
	/// and any parse errors encountered during loading.
	ThemesReady {
		token: u64,
		themes: Vec<xeno_registry::themes::LinkedThemeDef>,
		errors: Vec<(String, String)>,
	},
}

impl std::fmt::Debug for ThemeMsg {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		match self {
			Self::ThemesReady { token, themes, errors } => f
				.debug_struct("ThemesReady")
				.field("token", token)
				.field("themes_count", &themes.len())
				.field("errors", errors)
				.finish(),
		}
	}
}

impl ThemeMsg {
	/// Applies this message to editor state, returning redraw flags.
	///
	/// Validates the token against the pending theme load. Stale completions
	/// (superseded by a newer `kick_theme_load`) are silently ignored.
	pub fn apply(self, editor: &mut Editor) -> Dirty {
		match self {
			Self::ThemesReady {
				token,
				themes: _themes,
				errors,
			} => {
				if editor.state.async_state.pending_theme_load_token != Some(token) {
					tracing::debug!(token, "Ignoring stale theme load");
					return Dirty::NONE;
				}
				editor.state.async_state.pending_theme_load_token = None;

				editor.resolve_configured_theme();
				crate::bootstrap::cache_theme(&editor.state.config.config.theme);
				for (filename, error) in errors {
					editor.notify(xeno_registry::notifications::keys::error(format!("{filename}: {error}")));
				}
				Dirty::FULL
			}
		}
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	/// Verifies that a stale theme load token produces `Dirty::NONE` and does
	/// not clear the pending token (so the correct load can still land).
	#[tokio::test(flavor = "current_thread")]
	async fn theme_load_stale_token_does_not_register() {
		let mut editor = Editor::new_scratch();

		// Simulate two sequential kick_theme_load calls: tokens 0 then 1.
		// Only token 1 is "current".
		editor.state.async_state.pending_theme_load_token = Some(1);

		// Stale load (token=0) completes first.
		let stale = ThemeMsg::ThemesReady {
			token: 0,
			themes: vec![],
			errors: vec![],
		};
		let dirty = stale.apply(&mut editor);

		assert_eq!(dirty, Dirty::NONE, "stale token should produce Dirty::NONE");
		assert_eq!(
			editor.state.async_state.pending_theme_load_token,
			Some(1),
			"pending token should remain for the current load"
		);
	}

	/// Verifies that the latest token wins even when completions arrive in
	/// reversed order (stale first, then current).
	#[tokio::test(flavor = "current_thread")]
	async fn theme_load_latest_wins_even_if_completion_order_reversed() {
		let mut editor = Editor::new_scratch();
		editor.state.async_state.pending_theme_load_token = Some(5);

		// Stale load (token=3) arrives first → ignored.
		let stale = ThemeMsg::ThemesReady {
			token: 3,
			themes: vec![],
			errors: vec![],
		};
		let dirty = stale.apply(&mut editor);
		assert_eq!(dirty, Dirty::NONE);
		assert_eq!(editor.state.async_state.pending_theme_load_token, Some(5));

		// Current load (token=5) arrives → accepted.
		let current = ThemeMsg::ThemesReady {
			token: 5,
			themes: vec![],
			errors: vec![],
		};
		let dirty = current.apply(&mut editor);
		assert_eq!(dirty, Dirty::FULL, "current token should produce Dirty::FULL");
		assert_eq!(
			editor.state.async_state.pending_theme_load_token, None,
			"pending token should be cleared after apply"
		);
	}
}
