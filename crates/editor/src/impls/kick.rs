//! Background task spawning helpers.
//!
//! These methods spawn fire-and-forget tasks that send [`EditorMsg`] on completion.
//! The main loop drains messages and applies them to editor state.

use std::path::PathBuf;

use crate::msg::{EditorMsg, IoMsg, MsgSender, ThemeMsg};

use super::Editor;

impl Editor {
    /// Spawns a background task to load and register themes.
    ///
    /// Loads embedded themes and user themes from the config directory.
    /// Sends [`ThemeMsg::ThemesReady`] when complete.
    pub fn kick_theme_load(&self) {
        let tx = self.msg_tx();
        let user_themes_dir = crate::paths::get_config_dir().map(|d| d.join("themes"));

        tokio::spawn(async move {
            let errors = load_themes_blocking(user_themes_dir).await;
            send(&tx, ThemeMsg::ThemesReady { errors });
        });
    }

    /// Spawns a background task to load a file.
    ///
    /// Sends [`IoMsg::FileLoaded`] or [`IoMsg::LoadFailed`] on completion.
    pub fn kick_file_load(&self, path: PathBuf) {
        let tx = self.msg_tx();
        tokio::spawn(async move {
            match tokio::fs::read_to_string(&path).await {
                Ok(content) => {
                    let rope = ropey::Rope::from_str(&content);
                    let readonly = !is_writable(&path);
                    send(&tx, IoMsg::FileLoaded { path, rope, readonly });
                }
                Err(error) => {
                    send(&tx, IoMsg::LoadFailed { path, error });
                }
            }
        });
    }
}

/// Loads embedded and user themes in a blocking context.
///
/// Returns parse errors as (filename, error message) pairs.
async fn load_themes_blocking(user_themes_dir: Option<PathBuf>) -> Vec<(String, String)> {
    tokio::task::spawn_blocking(move || {
        let mut errors = xeno_runtime_config::load_and_register_embedded_themes();

        if let Some(dir) = user_themes_dir {
            if dir.exists() {
                match xeno_runtime_config::load_and_register_themes(&dir) {
                    Ok(e) => errors.extend(e),
                    Err(e) => {
                        tracing::warn!(error = %e, "failed to read user themes directory");
                    }
                }
            }
        }

        errors
    })
    .await
    .unwrap_or_default()
}

fn send<M: Into<EditorMsg>>(tx: &MsgSender, msg: M) {
    let _ = tx.send(msg.into());
}

fn is_writable(path: &std::path::Path) -> bool {
    std::fs::OpenOptions::new().write(true).open(path).is_ok()
}
