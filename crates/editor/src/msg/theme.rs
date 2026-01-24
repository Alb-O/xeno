//! Theme-related messages.

use super::Dirty;
use crate::Editor;

/// Messages for theme registration and switching.
#[derive(Debug)]
pub enum ThemeMsg {
    /// Themes have been registered; resolve and apply configured theme.
    ThemesReady,
}

impl ThemeMsg {
    pub fn apply(self, editor: &mut Editor) -> Dirty {
        match self {
            Self::ThemesReady => {
                editor.resolve_configured_theme();
                Dirty::FULL
            }
        }
    }
}
