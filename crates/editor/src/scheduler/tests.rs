use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::time::{Duration, Instant};

use parking_lot::Mutex;
use xeno_registry::hooks::{HookPriority, HookScheduler};

use super::ops::DrainBudget;
use super::state::*;
use super::types::*;

#[tokio::test]
async fn test_schedule_and_drain() {
	let counter = Arc::new(AtomicUsize::new(0));
	let mut scheduler = WorkScheduler::new();

	let c = counter.clone();
	scheduler.schedule(WorkItem {
		future: Box::pin(async move {
			c.fetch_add(1, Ordering::SeqCst);
		}),
		kind: WorkKind::Hook,
		priority: HookPriority::Interactive,
		doc_id: None,
	});

	let c = counter.clone();
	scheduler.schedule(WorkItem {
		future: Box::pin(async move {
			c.fetch_add(10, Ordering::SeqCst);
		}),
		kind: WorkKind::LspFlush,
		priority: HookPriority::Background,
		doc_id: Some(1),
	});

	assert_eq!(scheduler.pending_count(), 2);
	scheduler.drain_all().await;
	assert_eq!(counter.load(Ordering::SeqCst), 11);
}

#[tokio::test]
async fn test_interactive_before_background() {
	let order = Arc::new(Mutex::new(Vec::new()));
	let mut scheduler = WorkScheduler::new();

	let o = order.clone();
	scheduler.schedule(WorkItem {
		future: Box::pin(async move {
			o.lock().push("background");
		}),
		kind: WorkKind::Hook,
		priority: HookPriority::Background,
		doc_id: None,
	});

	let o = order.clone();
	scheduler.schedule(WorkItem {
		future: Box::pin(async move {
			o.lock().push("interactive");
		}),
		kind: WorkKind::Hook,
		priority: HookPriority::Interactive,
		doc_id: None,
	});

	scheduler.drain_all().await;
	let completed = order.lock();
	assert_eq!(completed[0], "interactive");
	assert_eq!(completed[1], "background");
}

#[tokio::test]
async fn test_background_drop_under_backlog() {
	let mut scheduler = WorkScheduler::new();

	for _ in 0..BACKGROUND_DROP_THRESHOLD {
		scheduler.schedule(WorkItem {
			future: Box::pin(async {}),
			kind: WorkKind::Hook,
			priority: HookPriority::Background,
			doc_id: None,
		});
	}
	assert_eq!(scheduler.dropped_total(), 0);

	scheduler.schedule(WorkItem {
		future: Box::pin(async {}),
		kind: WorkKind::Hook,
		priority: HookPriority::Background,
		doc_id: None,
	});
	assert_eq!(scheduler.dropped_total(), 1);
}

#[tokio::test]
async fn test_pending_for_doc() {
	let mut scheduler = WorkScheduler::new();

	scheduler.schedule(WorkItem {
		future: Box::pin(async {}),
		kind: WorkKind::LspFlush,
		priority: HookPriority::Interactive,
		doc_id: Some(42),
	});

	scheduler.schedule(WorkItem {
		future: Box::pin(async {}),
		kind: WorkKind::LspFlush,
		priority: HookPriority::Interactive,
		doc_id: Some(42),
	});

	scheduler.schedule(WorkItem {
		future: Box::pin(async {}),
		kind: WorkKind::Hook,
		priority: HookPriority::Interactive,
		doc_id: Some(42),
	});

	assert_eq!(scheduler.pending_for_doc(42, WorkKind::LspFlush), 2);
	assert_eq!(scheduler.pending_for_doc(42, WorkKind::Hook), 1);
	assert_eq!(scheduler.pending_for_doc(99, WorkKind::LspFlush), 0);
}

#[tokio::test]
async fn test_cancel() {
	let mut scheduler = WorkScheduler::new();

	for _ in 0..3 {
		scheduler.schedule(WorkItem {
			future: Box::pin(async {}),
			kind: WorkKind::LspFlush,
			priority: HookPriority::Interactive,
			doc_id: Some(42),
		});
	}

	assert_eq!(scheduler.pending_for_doc(42, WorkKind::LspFlush), 3);
	let cancelled = scheduler.cancel(42, WorkKind::LspFlush);
	assert_eq!(cancelled, 3);
	assert_eq!(scheduler.pending_for_doc(42, WorkKind::LspFlush), 0);
}

/// Must schedule registry async hooks via HookScheduler trait.
///
/// * Enforced in: `HookScheduler for WorkScheduler::schedule`
/// * Failure symptom: async hooks silently dropped after emit_sync_with
#[tokio::test]
async fn test_hook_scheduler_trait_routes_to_work_scheduler() {
	let counter = Arc::new(AtomicUsize::new(0));
	let mut scheduler = WorkScheduler::new();

	let c = counter.clone();
	HookScheduler::schedule(
		&mut scheduler,
		Box::pin(async move {
			c.fetch_add(1, Ordering::SeqCst);
			xeno_registry::hooks::HookResult::Continue
		}),
		HookPriority::Interactive,
	);

	let c = counter.clone();
	HookScheduler::schedule(
		&mut scheduler,
		Box::pin(async move {
			c.fetch_add(10, Ordering::SeqCst);
			xeno_registry::hooks::HookResult::Continue
		}),
		HookPriority::Background,
	);

	assert_eq!(scheduler.pending_count(), 2);
	scheduler.drain_all().await;
	assert_eq!(counter.load(Ordering::SeqCst), 11);
}

/// Must enforce background drop threshold for hooks scheduled via HookScheduler.
///
/// * Enforced in: `WorkScheduler::schedule` (via WorkItem path)
/// * Failure symptom: unbounded hook queue growth under sustained input
#[tokio::test]
async fn test_hook_scheduler_respects_background_drop_threshold() {
	let mut scheduler = WorkScheduler::new();

	for _ in 0..BACKGROUND_DROP_THRESHOLD {
		HookScheduler::schedule(
			&mut scheduler,
			Box::pin(async { xeno_registry::hooks::HookResult::Continue }),
			HookPriority::Background,
		);
	}
	assert_eq!(scheduler.dropped_total(), 0);

	HookScheduler::schedule(
		&mut scheduler,
		Box::pin(async { xeno_registry::hooks::HookResult::Continue }),
		HookPriority::Background,
	);
	assert_eq!(scheduler.dropped_total(), 1);
}

/// Must respect max_completions in drain budget.
///
/// * Enforced in: `WorkScheduler::drain_budget`
/// * Failure symptom: UI stalls from draining too many completions per tick
#[tokio::test]
async fn test_drain_budget_max_completions() {
	let mut scheduler = WorkScheduler::new();

	for _ in 0..10 {
		scheduler.schedule(WorkItem {
			future: Box::pin(async {}),
			kind: WorkKind::Hook,
			priority: HookPriority::Interactive,
			doc_id: None,
		});
	}

	// Allow tasks to finish executing so they're ready to be collected.
	tokio::task::yield_now().await;

	let stats = scheduler.drain_budget(DrainBudget::new(Duration::from_secs(10), 3)).await;

	// Drain only collects up to max_completions results per cycle.
	assert_eq!(stats.completed, 3);
	assert!(stats.pending > 0);
}

/// Must return DrainStats with correct completed and pending counts.
///
/// * Enforced in: `WorkScheduler::drain_budget`
/// * Failure symptom: metrics report wrong hook completion counts
#[tokio::test]
async fn test_drain_budget_returns_stats() {
	let mut scheduler = WorkScheduler::new();

	for _ in 0..5 {
		scheduler.schedule(WorkItem {
			future: Box::pin(async {}),
			kind: WorkKind::Hook,
			priority: HookPriority::Interactive,
			doc_id: None,
		});
	}

	let stats = scheduler.drain_budget(DrainBudget::new(Duration::from_secs(10), usize::MAX)).await;

	assert_eq!(stats.completed, 5);
	assert_eq!(stats.pending, 0);
}

/// Must surface panicked task counts in DrainStats.
///
/// * Enforced in: `WorkScheduler::drain_budget`
/// * Failure symptom: panicked tasks silently swallowed without metrics
#[tokio::test]
async fn test_drain_budget_surfaces_panicked_tasks() {
	let mut scheduler = WorkScheduler::new();

	scheduler.schedule(WorkItem {
		future: Box::pin(async { panic!("test panic") }),
		kind: WorkKind::Hook,
		priority: HookPriority::Interactive,
		doc_id: None,
	});

	scheduler.schedule(WorkItem {
		future: Box::pin(async {}),
		kind: WorkKind::Hook,
		priority: HookPriority::Interactive,
		doc_id: None,
	});

	tokio::task::yield_now().await;

	let stats = scheduler.drain_budget(DrainBudget::new(Duration::from_secs(10), usize::MAX)).await;
	assert_eq!(stats.completed, 2);
	assert_eq!(stats.panicked, 1);
	assert_eq!(stats.cancelled, 0);
	assert_eq!(stats.pending, 0);
	assert!(
		stats.panic_sample.as_deref().unwrap_or("").contains("test panic"),
		"expected panic_sample to contain 'test panic', got: {:?}",
		stats.panic_sample
	);
}

/// Must safely truncate unicode panic messages without panicking.
///
/// * Enforced in: `panic_sample` / `truncate_utf8`
/// * Failure symptom: editor crashes trying to format a unicode panic message
#[tokio::test]
async fn test_drain_budget_unicode_panic_sample_safe() {
	let mut scheduler = WorkScheduler::new();

	// 26 fire emojis, each 4 bytes = 104 bytes. With max_bytes=40, must split on a char boundary.
	scheduler.schedule(WorkItem {
		future: Box::pin(async { panic!("🔥🔥🔥🔥🔥🔥🔥🔥🔥🔥🔥🔥🔥🔥🔥🔥🔥🔥🔥🔥🔥🔥🔥🔥🔥🔥") }),
		kind: WorkKind::Hook,
		priority: HookPriority::Interactive,
		doc_id: None,
	});

	tokio::task::yield_now().await;

	let stats = scheduler.drain_budget(DrainBudget::new(Duration::from_secs(10), usize::MAX)).await;
	assert_eq!(stats.panicked, 1);
	let sample = stats.panic_sample.expect("should have a panic sample");
	assert!(sample.contains('🔥'), "sample should contain fire emoji: {sample}");
	assert!(sample.is_char_boundary(sample.len()), "sample must be valid UTF-8");
}

/// Must wake drain via notify when scheduled work completes asynchronously.
///
/// * Enforced in: `WorkScheduler::schedule` (NotifyOnDrop wrapping)
/// * Failure symptom: drain_budget sleeps until deadline even though work completed
#[tokio::test]
async fn drain_budget_waits_for_completion_via_notify() {
	let mut scheduler = WorkScheduler::new();

	scheduler.schedule(WorkItem {
		future: Box::pin(async {
			tokio::time::sleep(Duration::from_millis(20)).await;
		}),
		kind: WorkKind::Hook,
		priority: HookPriority::Interactive,
		doc_id: None,
	});

	let start = Instant::now();
	let stats = scheduler.drain_budget(DrainBudget::new(Duration::from_millis(200), 1)).await;
	let elapsed = start.elapsed();

	assert_eq!(stats.completed, 1, "should drain the one task");
	assert_eq!(stats.pending, 0, "nothing should remain");
	assert!(
		elapsed < Duration::from_millis(150),
		"drain should wake promptly via notify, not sleep full budget ({elapsed:?})"
	);
}

/// Must cancel doc-scoped work when cancel_doc is called.
///
/// * Enforced in: `WorkScheduler::schedule` (CancellationToken wrapping)
/// * Failure symptom: work runs to completion after buffer close
#[tokio::test]
async fn drain_budget_doc_cancel_drops_future() {
	let ran = Arc::new(AtomicBool::new(false));
	let mut scheduler = WorkScheduler::new();

	let r = ran.clone();
	scheduler.schedule(WorkItem {
		future: Box::pin(async move {
			tokio::time::sleep(Duration::from_millis(50)).await;
			r.store(true, Ordering::SeqCst);
		}),
		kind: WorkKind::Hook,
		priority: HookPriority::Interactive,
		doc_id: Some(42),
	});

	// Cancel all work for doc 42 before the future completes.
	scheduler.cancel_doc(42);

	let stats = scheduler.drain_budget(DrainBudget::new(Duration::from_millis(200), usize::MAX)).await;

	assert!(!ran.load(Ordering::SeqCst), "cancelled future should not have run to completion");
	assert_eq!(stats.completed, 1, "cancelled task still counts as a completion");
	assert_eq!(stats.pending, 0, "nothing should remain");
}

/// Must sticky-cancel: scheduling after cancel_doc immediately cancels the future.
///
/// * Enforced in: `WorkScheduler::cancel_doc` (keeps token in cancelled state)
/// * Failure symptom: work scheduled after buffer close runs to completion
#[tokio::test]
async fn cancel_doc_is_sticky() {
	let ran = Arc::new(AtomicBool::new(false));
	let mut scheduler = WorkScheduler::new();

	// Cancel doc 42 before scheduling any work.
	scheduler.cancel_doc(42);

	let r = ran.clone();
	scheduler.schedule(WorkItem {
		future: Box::pin(async move {
			r.store(true, Ordering::SeqCst);
		}),
		kind: WorkKind::Hook,
		priority: HookPriority::Interactive,
		doc_id: Some(42),
	});

	// Schedule was short-circuited — nothing spawned, no token created.
	assert_eq!(scheduler.pending_count(), 0, "cancelled schedule should not spawn");
	assert!(!ran.load(Ordering::SeqCst), "work scheduled after cancel_doc should never run");
	assert_eq!(scheduler.pending_for_doc(42, WorkKind::Hook), 0, "no pending count for cancelled doc");
	assert_eq!(scheduler.doc_cancel_len(), 0, "no token should be created for cancelled doc");
}

/// Must not cancel doc work when closing one view if other views still reference the doc.
///
/// * Enforced in: `finalize_document_if_orphaned` (only cancels when no views remain)
/// * Failure symptom: closing a split cancels work for the still-open buffer
#[tokio::test]
async fn cancel_doc_does_not_fire_with_pending_by_doc_entries() {
	let ran = Arc::new(AtomicBool::new(false));
	let mut scheduler = WorkScheduler::new();

	let r = ran.clone();
	scheduler.schedule(WorkItem {
		future: Box::pin(async move {
			tokio::time::sleep(Duration::from_millis(20)).await;
			r.store(true, Ordering::SeqCst);
		}),
		kind: WorkKind::Hook,
		priority: HookPriority::Interactive,
		doc_id: Some(42),
	});

	// Simulate: doc_id 42 is NOT cancelled (another view still open).
	// Just drain normally — the work should complete.
	let stats = scheduler.drain_budget(DrainBudget::new(Duration::from_millis(200), usize::MAX)).await;

	assert!(ran.load(Ordering::SeqCst), "work should complete when doc is not cancelled");
	assert_eq!(stats.completed, 1);
	assert_eq!(stats.pending, 0);
}

/// Must purge pending_by_doc bookkeeping when cancel_doc is called.
///
/// * Enforced in: `WorkScheduler::cancel_doc`
/// * Failure symptom: stale pending counts for closed documents
#[tokio::test]
async fn cancel_doc_purges_pending_by_doc() {
	let mut scheduler = WorkScheduler::new();

	scheduler.schedule(WorkItem {
		future: Box::pin(async {}),
		kind: WorkKind::LspFlush,
		priority: HookPriority::Interactive,
		doc_id: Some(42),
	});

	assert_eq!(scheduler.pending_for_doc(42, WorkKind::LspFlush), 1);

	scheduler.cancel_doc(42);

	assert_eq!(scheduler.pending_for_doc(42, WorkKind::LspFlush), 0, "cancel_doc should purge pending_by_doc");
}

/// Must decrement pending_by_doc on normal completion via drop guard.
///
/// * Enforced in: `PendingCountGuard` (Drop impl)
/// * Failure symptom: pending_for_doc drifts upward, never reaches 0
#[tokio::test]
async fn pending_by_doc_decrements_on_completion() {
	let mut scheduler = WorkScheduler::new();

	scheduler.schedule(WorkItem {
		future: Box::pin(async {}),
		kind: WorkKind::Hook,
		priority: HookPriority::Interactive,
		doc_id: Some(42),
	});

	assert_eq!(scheduler.pending_for_doc(42, WorkKind::Hook), 1);

	scheduler.drain_all().await;

	assert_eq!(scheduler.pending_for_doc(42, WorkKind::Hook), 0, "pending should reach 0 after completion");
}

/// Must decrement pending_by_doc on panicked task via drop guard.
///
/// * Enforced in: `PendingCountGuard` (Drop impl, unwind-safe)
/// * Failure symptom: pending counts leak for panicking tasks
#[tokio::test]
async fn pending_by_doc_decrements_on_panic() {
	let mut scheduler = WorkScheduler::new();

	scheduler.schedule(WorkItem {
		future: Box::pin(async { panic!("test") }),
		kind: WorkKind::Hook,
		priority: HookPriority::Interactive,
		doc_id: Some(42),
	});

	assert_eq!(scheduler.pending_for_doc(42, WorkKind::Hook), 1);

	tokio::task::yield_now().await;
	let stats = scheduler.drain_budget(DrainBudget::new(Duration::from_secs(10), usize::MAX)).await;

	assert_eq!(stats.panicked, 1);
	assert_eq!(scheduler.pending_for_doc(42, WorkKind::Hook), 0, "pending should reach 0 after panic");
}

/// Must remove token entry from doc_cancel when cancel_doc is called.
///
/// * Enforced in: `WorkScheduler::cancel_doc`
/// * Failure symptom: unbounded doc_cancel HashMap growth in long sessions
#[tokio::test]
async fn cancel_doc_removes_token_entry() {
	let mut scheduler = WorkScheduler::new();

	// Scheduling with a doc_id creates a token.
	scheduler.schedule(WorkItem {
		future: Box::pin(async {}),
		kind: WorkKind::Hook,
		priority: HookPriority::Interactive,
		doc_id: Some(1),
	});
	assert_eq!(scheduler.doc_cancel_len(), 1, "scheduling should create a token");

	scheduler.cancel_doc(1);
	assert_eq!(scheduler.doc_cancel_len(), 0, "cancel_doc should remove the token");
}

/// Must bound the cancelled-docs LRU set.
///
/// * Enforced in: `WorkScheduler::mark_doc_cancelled`
/// * Failure symptom: unbounded HashSet growth over long sessions
#[tokio::test]
async fn cancelled_docs_lru_is_bounded() {
	let mut scheduler = WorkScheduler::new();

	for i in 0..(CANCELLED_DOC_LRU_CAP + 10) as u64 {
		scheduler.cancel_doc(i);
	}

	assert!(
		scheduler.cancelled_docs_len() <= CANCELLED_DOC_LRU_CAP,
		"cancelled_docs should not exceed LRU cap: {} > {}",
		scheduler.cancelled_docs_len(),
		CANCELLED_DOC_LRU_CAP,
	);
}

/// Must cancel in-flight work when cancel(doc_id, kind) is called.
///
/// * Enforced in: `WorkScheduler::cancel` (kind token cancellation)
/// * Failure symptom: cancelled work runs to completion
#[tokio::test]
async fn cancel_doc_kind_cancels_inflight_work() {
	let ran = Arc::new(AtomicBool::new(false));
	let mut scheduler = WorkScheduler::new();

	let r = ran.clone();
	scheduler.schedule(WorkItem {
		future: Box::pin(async move {
			tokio::time::sleep(Duration::from_millis(50)).await;
			r.store(true, Ordering::SeqCst);
		}),
		kind: WorkKind::Hook,
		priority: HookPriority::Interactive,
		doc_id: Some(1),
	});

	tokio::task::yield_now().await;
	assert_eq!(scheduler.pending_for_doc(1, WorkKind::Hook), 1);

	let count = scheduler.cancel(1, WorkKind::Hook);
	assert_eq!(count, 1, "cancel should return prior pending count");

	let stats = scheduler.drain_budget(DrainBudget::new(Duration::from_millis(200), usize::MAX)).await;

	assert!(!ran.load(Ordering::SeqCst), "cancelled work should not run to completion");
	assert_eq!(scheduler.pending_for_doc(1, WorkKind::Hook), 0);
	assert_eq!(stats.completed, 1);
	assert_eq!(stats.pending, 0);
}

/// Must allow new work after cancel(doc_id, kind) — cancel is not sticky for kind.
///
/// * Enforced in: `WorkScheduler::cancel` (removes kind token, new schedule creates fresh one)
/// * Failure symptom: latest-wins debounce breaks — new work also cancelled
#[tokio::test]
async fn cancel_doc_kind_does_not_cancel_future_schedules() {
	let ran1 = Arc::new(AtomicBool::new(false));
	let ran2 = Arc::new(AtomicBool::new(false));
	let mut scheduler = WorkScheduler::new();

	let r1 = ran1.clone();
	scheduler.schedule(WorkItem {
		future: Box::pin(async move {
			tokio::time::sleep(Duration::from_millis(50)).await;
			r1.store(true, Ordering::SeqCst);
		}),
		kind: WorkKind::Hook,
		priority: HookPriority::Interactive,
		doc_id: Some(1),
	});

	scheduler.cancel(1, WorkKind::Hook);

	let r2 = ran2.clone();
	scheduler.schedule(WorkItem {
		future: Box::pin(async move {
			r2.store(true, Ordering::SeqCst);
		}),
		kind: WorkKind::Hook,
		priority: HookPriority::Interactive,
		doc_id: Some(1),
	});

	scheduler.drain_all().await;

	assert!(!ran1.load(Ordering::SeqCst), "first task should have been cancelled");
	assert!(ran2.load(Ordering::SeqCst), "second task should have run after cancel");
}
