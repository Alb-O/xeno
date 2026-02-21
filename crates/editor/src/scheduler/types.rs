use std::future::Future;
use std::pin::Pin;

use xeno_registry::hooks::HookPriority;

/// Unique identifier for a document (used for cancellation).
pub type DocId = u64;

/// Kind of async work being scheduled.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum WorkKind {
	/// Hook execution.
	Hook,
	/// Nu hook async evaluation.
	NuHook,
}

/// A scheduled work item.
pub struct WorkItem {
	/// The future to execute.
	pub future: Pin<Box<dyn Future<Output = ()> + Send + 'static>>,
	/// Kind of work.
	pub kind: WorkKind,
	/// Execution priority.
	pub priority: HookPriority,
	/// Optional document ID for cancellation.
	pub doc_id: Option<DocId>,
}
