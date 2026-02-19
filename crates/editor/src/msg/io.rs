//! File I/O messages.

use std::io;
use std::path::PathBuf;

use ropey::Rope;

use super::Dirty;
use crate::Editor;

/// Messages for file loading completion.
#[derive(Debug)]
pub enum IoMsg {
	/// File loaded successfully.
	FileLoaded { path: PathBuf, rope: Rope, readonly: bool, token: u64 },
	/// File load failed.
	LoadFailed { path: PathBuf, error: io::Error, token: u64 },
}

impl IoMsg {
	pub fn apply(self, editor: &mut Editor) -> Dirty {
		match self {
			Self::FileLoaded { path, rope, readonly, token } => {
				editor.apply_loaded_file(path, rope, readonly, token);
				Dirty::FULL
			}
			Self::LoadFailed { path, error, token } => {
				editor.notify_load_error(&path, &error, token);
				Dirty::NONE
			}
		}
	}
}
