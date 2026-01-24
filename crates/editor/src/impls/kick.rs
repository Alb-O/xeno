//! Background task spawning helpers.
//!
//! These methods spawn fire-and-forget tasks that send [`EditorMsg`] on completion.
//! The main loop drains messages and applies them to editor state.

use std::path::PathBuf;

use crate::msg::{EditorMsg, IoMsg, MsgSender, ThemeMsg};

use super::Editor;

impl Editor {
    /// Spawns a background task to load themes.
    ///
    /// Sends [`ThemeMsg::ThemesReady`] when complete.
    pub fn kick_theme_load(&self) {
        let tx = self.msg_tx();
        tokio::spawn(async move {
            send(&tx, ThemeMsg::ThemesReady);
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

fn send<M: Into<EditorMsg>>(tx: &MsgSender, msg: M) {
    let _ = tx.send(msg.into());
}

fn is_writable(path: &std::path::Path) -> bool {
    std::fs::OpenOptions::new().write(true).open(path).is_ok()
}
