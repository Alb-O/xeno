//! Host access trait for on-demand queries during Nu evaluation.
//!
//! Commands like `xeno buffer get` and `xeno buffer text` need to reach back
//! into the editor to fetch live data. The [`XenoNuHost`] trait provides a
//! read-only query interface that the editor implements.
//!
//! # Thread-local access pattern
//!
//! A raw pointer to a `'static` host is installed in a thread-local via RAII
//! guard before `eval_call` and restored on drop (panic-safe, nest-safe).
//! Commands call [`with_host`] to borrow the host for the duration of a closure.
//!
//! The `'static` bound is intentional: hosts must own their data (e.g.
//! [`NuHostSnapshot`](crate::host) captures a Rope clone), preventing borrow
//! lifetime issues and enforcing the snapshot-coherence invariant.

use std::cell::Cell;
use std::fmt;

/// Metadata about a buffer, returned by [`XenoNuHost::buffer_get`].
#[derive(Debug, Clone)]
pub struct BufferMeta {
	pub path: Option<String>,
	pub file_type: Option<String>,
	pub readonly: bool,
	pub modified: bool,
	pub line_count: usize,
}

/// A range expressed as line/col pairs (0-indexed).
#[derive(Debug, Clone, Copy)]
pub struct LineColRange {
	pub start_line: usize,
	pub start_col: usize,
	pub end_line: usize,
	pub end_col: usize,
}

/// A chunk of text returned by [`XenoNuHost::buffer_text`].
#[derive(Debug, Clone)]
pub struct TextChunk {
	pub text: String,
	pub truncated: bool,
}

/// Error from host queries.
#[derive(Debug, Clone)]
pub struct HostError(pub String);

impl fmt::Display for HostError {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		f.write_str(&self.0)
	}
}

impl std::error::Error for HostError {}

/// Read-only query interface into the editor, implemented by the host.
pub trait XenoNuHost {
	/// Return metadata for a buffer. `None` id means the active buffer.
	fn buffer_get(&self, id: Option<i64>) -> Result<BufferMeta, HostError>;

	/// Return bounded text from a buffer. `None` id means the active buffer.
	///
	/// If `range` is `None`, returns the full buffer text (clamped to `max_bytes`).
	/// If `range` is `Some`, returns the text within that range (clamped to `max_bytes`).
	fn buffer_text(&self, id: Option<i64>, range: Option<LineColRange>, max_bytes: usize) -> Result<TextChunk, HostError>;
}

thread_local! {
	static HOST: Cell<Option<*const (dyn XenoNuHost + 'static)>> = const { Cell::new(None) };
}

/// RAII guard that restores the previous host pointer on drop (panic-safe, nest-safe).
struct HostGuard {
	prev: Option<*const (dyn XenoNuHost + 'static)>,
}

impl Drop for HostGuard {
	fn drop(&mut self) {
		HOST.set(self.prev);
	}
}

/// Install a host reference for the duration of a closure.
///
/// Panic-safe: the previous host pointer is restored via RAII guard even if `f` panics.
/// Nest-safe: nested installs restore the outer pointer when their guard drops.
///
/// The `'static` bound ensures hosts own their data (snapshot pattern).
pub(crate) fn with_host_installed<R>(host: &(dyn XenoNuHost + 'static), f: impl FnOnce() -> R) -> R {
	let prev = HOST.replace(Some(host as *const (dyn XenoNuHost + 'static)));
	let _guard = HostGuard { prev };
	f()
}

/// Access the currently-installed host from within a Nu command.
///
/// Returns `None` if no host is installed (e.g. during tests or config evaluation).
pub(crate) fn with_host<R>(f: impl FnOnce(&dyn XenoNuHost) -> R) -> Option<R> {
	// SAFETY: The pointer is valid because `with_host_installed` holds a borrow
	// for the entire duration of evaluation via RAII guard.
	HOST.get().map(|p| f(unsafe { &*p }))
}
