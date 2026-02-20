//! Atomic file writing utilities.
//!
//! Provides crash-safe file persistence by writing to a temporary file
//! in the same directory, syncing, then atomically renaming over the
//! target. On POSIX, `rename(2)` on the same filesystem is atomic — if
//! the process dies mid-write, the original file remains intact.
//!
//! When overwriting an existing file, the original's permission mode
//! bits are captured before the write and re-applied after the rename
//! so that `temp+persist` doesn't silently alter the file's permissions.

use std::io;
use std::path::Path;

/// Atomically writes `bytes` to `path`, preserving existing permissions.
///
/// Writes to a temporary file in the same parent directory (ensuring
/// same-filesystem rename), calls `sync_all` to flush to stable
/// storage, then renames onto the target path. If the target already
/// exists, its permission mode bits are captured and re-applied after
/// the rename so that the atomic write doesn't alter file permissions.
///
/// # Errors
///
/// Returns [`io::Error`] if the parent directory is missing, the temp
/// file cannot be created, writing fails, sync fails, or the rename
/// fails. In all error cases, the original file at `path` is untouched.
pub fn write_atomic(path: &Path, bytes: &[u8]) -> io::Result<()> {
	// Capture existing permissions before overwriting.
	let original_perms = std::fs::metadata(path).ok().map(|m| m.permissions());

	let parent = path.parent().unwrap_or(Path::new("."));
	let mut tmp = tempfile::NamedTempFile::new_in(parent)?;

	io::Write::write_all(&mut tmp, bytes)?;
	tmp.as_file().sync_all()?;
	tmp.persist(path).map_err(|e| e.error)?;

	// Restore original permissions after rename.
	if let Some(perms) = original_perms {
		std::fs::set_permissions(path, perms)?;
	}

	// Best-effort parent dir sync (Linux/macOS). Not critical but
	// ensures the directory entry is durable across power loss.
	#[cfg(unix)]
	{
		if let Ok(dir) = std::fs::File::open(parent) {
			let _ = dir.sync_all();
		}
	}

	Ok(())
}

/// Errors from [`save_buffer_to_disk`].
#[derive(Debug)]
pub(crate) enum SaveError {
	/// Buffer has no file path.
	NoPath,
	/// Buffer is read-only.
	ReadOnly(String),
	/// IO error during write.
	Io { path: String, error: String },
	/// spawn_blocking task failed (panic or cancellation).
	TaskFailed(String),
}

impl std::fmt::Display for SaveError {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		match self {
			Self::NoPath => write!(f, "buffer has no file path"),
			Self::ReadOnly(p) => write!(f, "buffer is read-only: {p}"),
			Self::Io { path, error } => write!(f, "io error: {path} — {error}"),
			Self::TaskFailed(e) => write!(f, "save task failed: {e}"),
		}
	}
}

impl std::error::Error for SaveError {}

/// Serializes a buffer's content to bytes (rope → `Vec<u8>`).
pub(crate) fn serialize_buffer(buffer: &crate::buffer::Buffer) -> Vec<u8> {
	buffer.with_doc(|doc| {
		let rope = doc.content();
		let mut bytes = Vec::with_capacity(rope.len_bytes());
		for chunk in rope.chunks() {
			bytes.extend_from_slice(chunk.as_bytes());
		}
		bytes
	})
}

/// Atomically writes a buffer's content to its file path via
/// [`write_atomic`] on a blocking thread.
///
/// Returns `Ok(path)` on success (caller decides whether to clear
/// modified flag, send notifications, etc.). Does not mutate the
/// buffer itself.
///
/// # Errors
///
/// * [`SaveError::NoPath`] — buffer has no file path
/// * [`SaveError::ReadOnly`] — buffer is marked read-only
/// * [`SaveError::Io`] — write_atomic failed
/// * [`SaveError::TaskFailed`] — spawn_blocking panicked
pub(crate) async fn save_buffer_to_disk(buffer: &crate::buffer::Buffer, worker_runtime: &xeno_worker::WorkerRuntime) -> Result<std::path::PathBuf, SaveError> {
	let path = buffer.path().map(|p| p.to_path_buf()).ok_or(SaveError::NoPath)?;
	if buffer.is_readonly() {
		return Err(SaveError::ReadOnly(path.display().to_string()));
	}

	let bytes = serialize_buffer(buffer);
	let write_path = path.clone();
	let result = worker_runtime
		.spawn_blocking(xeno_worker::TaskClass::IoBlocking, move || write_atomic(&write_path, &bytes))
		.await;
	match result {
		Ok(Ok(())) => Ok(path),
		Ok(Err(e)) => Err(SaveError::Io {
			path: path.display().to_string(),
			error: e.to_string(),
		}),
		Err(e) => Err(SaveError::TaskFailed(e.to_string())),
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[tokio::test]
	async fn save_buffer_to_disk_clears_on_success() {
		let dir = tempfile::tempdir().unwrap();
		let path = dir.path().join("save_success.txt");
		std::fs::write(&path, "old\n").unwrap();

		let mut editor = crate::Editor::new_scratch();
		let view_id = editor.open_file(path.clone()).await.unwrap();

		// Edit to make modified.
		{
			use xeno_primitives::{SyntaxPolicy, Transaction, UndoPolicy};

			use crate::buffer::ApplyPolicy;
			let buffer = editor.state.core.editor.buffers.get_buffer_mut(view_id).unwrap();
			let tx = buffer.with_doc(|doc| {
				Transaction::change(
					doc.content().slice(..),
					vec![xeno_primitives::transaction::Change {
						start: 0,
						end: 3,
						replacement: Some("new".into()),
					}],
				)
			});
			buffer.apply(
				&tx,
				ApplyPolicy {
					undo: UndoPolicy::Record,
					syntax: SyntaxPolicy::IncrementalOrDirty,
				},
			);
		}
		assert!(editor.state.core.editor.buffers.get_buffer(view_id).unwrap().modified());

		let buffer = editor.state.core.editor.buffers.get_buffer(view_id).unwrap();
		let saved_path = save_buffer_to_disk(buffer, &editor.state.async_state.worker_runtime).await.unwrap();
		assert_eq!(saved_path, path);
		assert_eq!(std::fs::read_to_string(&path).unwrap(), "new\n");
	}

	#[tokio::test]
	async fn save_buffer_to_disk_rejects_readonly() {
		let dir = tempfile::tempdir().unwrap();
		let path = dir.path().join("readonly.txt");
		std::fs::write(&path, "locked\n").unwrap();

		let mut editor = crate::Editor::new_scratch();
		let view_id = editor.open_file(path.clone()).await.unwrap();
		editor.state.core.editor.buffers.get_buffer_mut(view_id).unwrap().set_readonly(true);

		let buffer = editor.state.core.editor.buffers.get_buffer(view_id).unwrap();
		let err = save_buffer_to_disk(buffer, &editor.state.async_state.worker_runtime).await.unwrap_err();
		assert!(matches!(err, SaveError::ReadOnly(_)), "expected ReadOnly, got: {err}");
		assert_eq!(std::fs::read_to_string(&path).unwrap(), "locked\n", "disk must be unchanged");
	}

	#[test]
	fn write_atomic_creates_new_file() {
		let dir = tempfile::tempdir().unwrap();
		let path = dir.path().join("new.txt");
		write_atomic(&path, b"hello").unwrap();
		assert_eq!(std::fs::read(&path).unwrap(), b"hello");
	}

	#[test]
	fn write_atomic_overwrites_existing() {
		let dir = tempfile::tempdir().unwrap();
		let path = dir.path().join("existing.txt");
		std::fs::write(&path, b"old").unwrap();
		write_atomic(&path, b"new").unwrap();
		assert_eq!(std::fs::read(&path).unwrap(), b"new");
	}

	#[cfg(unix)]
	#[test]
	fn write_atomic_fails_on_readonly_dir() {
		use std::os::unix::fs::PermissionsExt;
		let dir = tempfile::tempdir().unwrap();
		let path = dir.path().join("fail.txt");
		std::fs::write(&path, b"original").unwrap();

		// Make directory read-only so temp file creation fails.
		std::fs::set_permissions(dir.path(), std::fs::Permissions::from_mode(0o555)).unwrap();
		let err = write_atomic(&path, b"boom");
		assert!(err.is_err(), "write_atomic should fail on read-only directory");

		// Original file untouched.
		assert_eq!(std::fs::read(&path).unwrap(), b"original");

		// Cleanup: restore permissions.
		std::fs::set_permissions(dir.path(), std::fs::Permissions::from_mode(0o755)).unwrap();
	}

	#[cfg(unix)]
	#[test]
	fn write_atomic_preserves_permissions_on_overwrite() {
		use std::os::unix::fs::PermissionsExt;
		let dir = tempfile::tempdir().unwrap();
		let path = dir.path().join("perms.txt");
		std::fs::write(&path, b"old").unwrap();

		// Set a distinctive permission mode.
		std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o640)).unwrap();

		write_atomic(&path, b"new").unwrap();
		assert_eq!(std::fs::read(&path).unwrap(), b"new");

		let mode = std::fs::metadata(&path).unwrap().permissions().mode() & 0o777;
		assert_eq!(mode, 0o640, "permissions must be preserved after atomic write");
	}
}
