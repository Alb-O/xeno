//! Syntax-manager invariant test harness.
//!
//! Supplies deterministic mock engines and shared helpers used by
//! `syntax_manager` invariant proofs.

use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

use tokio::sync::Notify;
use tokio::time::{sleep, timeout};
use xeno_language::LanguageLoader;
use xeno_language::syntax::{InjectionPolicy, Syntax, SyntaxError, SyntaxOptions};
use xeno_primitives::{ChangeSet, Rope};

use super::*;
use crate::core::document::DocumentId;

/// Mock parsing engine that blocks until explicitly released.
///
/// Allows tests to control task scheduling deterministically by gating
/// `parse` calls behind a [`Notify`] barrier.
pub(crate) struct MockEngine {
	pub(crate) parse_count: AtomicUsize,
	pub(crate) result: Arc<parking_lot::Mutex<std::result::Result<Syntax, String>>>,
	pub(crate) notify: Arc<Notify>,
}

impl MockEngine {
	pub(crate) fn new() -> Self {
		let loader = LanguageLoader::from_embedded();
		let lang = loader.language_for_name("rust").unwrap();
		let syntax = Syntax::new(Rope::from("").slice(..), lang, &loader, SyntaxOptions::default()).unwrap();

		Self {
			parse_count: AtomicUsize::new(0),
			result: Arc::new(parking_lot::Mutex::new(Ok(syntax))),
			notify: Arc::new(Notify::new()),
		}
	}

	/// Allows one pending parse to proceed.
	pub(crate) fn proceed(&self) {
		self.notify.notify_one();
	}

	/// Allows all pending parses to proceed immediately.
	pub(crate) fn proceed_all(&self) {
		for _ in 0..100 {
			self.notify.notify_one();
		}
	}
}

impl SyntaxEngine for MockEngine {
	fn parse(&self, _content: ropey::RopeSlice<'_>, _lang: LanguageId, _loader: &LanguageLoader, _opts: SyntaxOptions) -> Result<Syntax, SyntaxError> {
		self.parse_count.fetch_add(1, Ordering::SeqCst);
		futures::executor::block_on(self.notify.notified());

		match &*self.result.lock() {
			Ok(s) => Ok(s.clone()),
			Err(e) => {
				if e == "timeout" {
					Err(SyntaxError::Timeout)
				} else {
					Err(SyntaxError::Parse(e.clone()))
				}
			}
		}
	}
}

/// RAII guard that unblocks all pending parses on drop, preventing test hangs.
pub(crate) struct EngineGuard(pub(crate) Arc<MockEngine>);

impl Drop for EngineGuard {
	fn drop(&mut self) {
		self.0.proceed_all();
	}
}

/// Mock engine that simulates timeout based on the requested budget.
pub(crate) struct TimeoutSensitiveEngine {
	pub(crate) parse_count: AtomicUsize,
	pub(crate) threshold: Duration,
	pub(crate) result: Syntax,
}

impl TimeoutSensitiveEngine {
	pub(crate) fn new(threshold: Duration) -> Self {
		let loader = LanguageLoader::from_embedded();
		let lang = loader.language_for_name("rust").unwrap();
		let syntax = Syntax::new(Rope::from("").slice(..), lang, &loader, SyntaxOptions::default()).unwrap();

		Self {
			parse_count: AtomicUsize::new(0),
			threshold,
			result: syntax,
		}
	}
}

impl SyntaxEngine for TimeoutSensitiveEngine {
	fn parse(&self, _content: ropey::RopeSlice<'_>, _lang: LanguageId, _loader: &LanguageLoader, opts: SyntaxOptions) -> Result<Syntax, SyntaxError> {
		self.parse_count.fetch_add(1, Ordering::SeqCst);
		if opts.parse_timeout <= self.threshold {
			Err(SyntaxError::Timeout)
		} else {
			Ok(self.result.clone())
		}
	}
}

/// Convenience: creates a standard [`EnsureSyntaxContext`] for tests.
fn make_ctx<'a>(
	doc_id: DocumentId,
	doc_version: u64,
	language_id: Option<LanguageId>,
	content: &'a Rope,
	hotness: SyntaxHotness,
	loader: &'a Arc<LanguageLoader>,
) -> EnsureSyntaxContext<'a> {
	EnsureSyntaxContext {
		doc_id,
		doc_version,
		language_id,
		content,
		hotness,
		loader,
		viewport: None,
	}
}

/// Spins until `mgr.any_task_finished()` returns true, up to 100 ms.
pub(super) async fn wait_for_finish(mgr: &SyntaxManager) {
	timeout(Duration::from_secs(1), async {
		while !mgr.any_task_finished() {
			sleep(Duration::from_millis(1)).await;
		}
	})
	.await
	.expect("Task did not finish in time");
}

/// Must enforce single-flight per document.
///
/// * Enforced in: `SyntaxManager::ensure_syntax`
/// * Failure symptom: Multiple redundant parse tasks for the same document identity.
#[cfg(test)]
mod tests;
