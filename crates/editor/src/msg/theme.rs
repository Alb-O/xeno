//! Theme-related messages.

use super::Dirty;
use crate::Editor;

/// Messages for theme registration and switching.
#[derive(Debug)]
pub enum ThemeMsg {
    /// Themes have been loaded and registered; resolve configured theme.
    ///
    /// Contains any parse errors from theme loading (filename, error message).
    ThemesReady { errors: Vec<(String, String)> },
}

impl ThemeMsg {
    /// Applies this message to editor state, returning redraw flags.
    pub fn apply(self, editor: &mut Editor) -> Dirty {
        match self {
            Self::ThemesReady { errors } => {
                editor.resolve_configured_theme();
                crate::bootstrap::cache_theme(editor.state.config.theme);
                for (filename, error) in errors {
                    editor.notify(xeno_registry::notification_keys::error(format!(
                        "{filename}: {error}"
                    )));
                }
                Dirty::FULL
            }
        }
    }
}
