#[cfg(feature = "lsp")]
use futures::channel::oneshot;

#[cfg(feature = "lsp")]
pub struct FlushHandle {
	pub(crate) handles: Vec<oneshot::Receiver<()>>,
}

#[cfg(feature = "lsp")]
impl FlushHandle {
	/// Wait until all didChange messages have been written.
	pub async fn await_synced(self) {
		for handle in self.handles {
			let _ = handle.await;
		}
	}
}
