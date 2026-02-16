use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

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
