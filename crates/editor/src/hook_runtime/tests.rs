use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use parking_lot::Mutex;
use xeno_registry::HookResult;

use super::*;

#[tokio::test]
async fn test_empty_drain() {
	let mut runtime = HookRuntime::new();
	assert!(!runtime.has_pending());
	runtime.drain_all().await;
	assert!(!runtime.has_pending());
}

#[tokio::test]
async fn test_schedule_and_drain_all() {
	let counter = Arc::new(AtomicUsize::new(0));
	let mut runtime = HookRuntime::new();

	let c1 = counter.clone();
	runtime.schedule(
		Box::pin(async move {
			c1.fetch_add(1, Ordering::SeqCst);
			HookResult::Continue
		}),
		HookPriority::Interactive,
	);

	let c2 = counter.clone();
	runtime.schedule(
		Box::pin(async move {
			c2.fetch_add(10, Ordering::SeqCst);
			HookResult::Continue
		}),
		HookPriority::Background,
	);

	assert!(runtime.has_pending());
	assert_eq!(runtime.pending_count(), 2);
	assert_eq!(runtime.interactive_count(), 1);
	assert_eq!(runtime.background_count(), 1);
	assert_eq!(runtime.scheduled_total(), 2);
	assert_eq!(runtime.completed_total(), 0);

	runtime.drain_all().await;

	assert!(!runtime.has_pending());
	assert_eq!(counter.load(Ordering::SeqCst), 11);
	assert_eq!(runtime.completed_total(), 2);
}

#[tokio::test]
async fn test_drain_budget_completes_fast_hooks() {
	let counter = Arc::new(AtomicUsize::new(0));
	let mut runtime = HookRuntime::new();

	for _ in 0..2 {
		let c = counter.clone();
		runtime.schedule(
			Box::pin(async move {
				c.fetch_add(1, Ordering::SeqCst);
				HookResult::Continue
			}),
			HookPriority::Interactive,
		);
	}

	assert_eq!(runtime.pending_count(), 2);
	runtime.drain_budget(Duration::from_millis(100)).await;

	assert!(!runtime.has_pending());
	assert_eq!(counter.load(Ordering::SeqCst), 2);
}

#[tokio::test]
async fn test_drain_budget_returns_on_empty() {
	let mut runtime = HookRuntime::new();
	let start = Instant::now();
	runtime.drain_budget(Duration::from_secs(10)).await;
	assert!(start.elapsed() < Duration::from_millis(100));
}

#[tokio::test]
async fn test_drain_budget_respects_timeout() {
	let mut runtime = HookRuntime::new();

	runtime.schedule(
		Box::pin(async {
			tokio::time::sleep(Duration::from_secs(10)).await;
			HookResult::Continue
		}),
		HookPriority::Interactive,
	);

	let start = Instant::now();
	runtime.drain_budget(Duration::from_millis(10)).await;

	assert!(start.elapsed() < Duration::from_millis(100));
	assert!(runtime.has_pending());
}

#[tokio::test]
async fn test_concurrent_execution() {
	let order = Arc::new(Mutex::new(Vec::new()));
	let mut runtime = HookRuntime::new();

	for i in 0..3 {
		let o = order.clone();
		runtime.schedule(
			Box::pin(async move {
				o.lock().push(i);
				HookResult::Continue
			}),
			HookPriority::Interactive,
		);
	}

	runtime.drain_all().await;

	let completed = order.lock();
	assert_eq!(completed.len(), 3);
	assert!(completed.contains(&0));
	assert!(completed.contains(&1));
	assert!(completed.contains(&2));
}

#[tokio::test]
async fn test_interactive_hooks_run_before_background() {
	let order = Arc::new(Mutex::new(Vec::new()));
	let mut runtime = HookRuntime::new();

	let o1 = order.clone();
	runtime.schedule(
		Box::pin(async move {
			o1.lock().push("background");
			HookResult::Continue
		}),
		HookPriority::Background,
	);

	let o2 = order.clone();
	runtime.schedule(
		Box::pin(async move {
			o2.lock().push("interactive");
			HookResult::Continue
		}),
		HookPriority::Interactive,
	);

	runtime.drain_all().await;

	let completed = order.lock();
	assert_eq!(completed.len(), 2);
	assert_eq!(completed[0], "interactive");
	assert_eq!(completed[1], "background");
}

#[tokio::test]
async fn test_background_hooks_dropped_under_backlog() {
	let mut runtime = HookRuntime::new();

	for _ in 0..BACKGROUND_DROP_THRESHOLD {
		runtime.schedule(
			Box::pin(async { HookResult::Continue }),
			HookPriority::Background,
		);
	}
	assert_eq!(runtime.background_count(), BACKGROUND_DROP_THRESHOLD);
	assert_eq!(runtime.dropped_total(), 0);

	runtime.schedule(
		Box::pin(async { HookResult::Continue }),
		HookPriority::Background,
	);
	assert_eq!(runtime.background_count(), BACKGROUND_DROP_THRESHOLD);
	assert_eq!(runtime.dropped_total(), 1);

	runtime.schedule(
		Box::pin(async { HookResult::Continue }),
		HookPriority::Interactive,
	);
	assert_eq!(runtime.interactive_count(), 1);
}

#[tokio::test]
async fn test_drop_background() {
	let mut runtime = HookRuntime::new();

	for _ in 0..10 {
		runtime.schedule(
			Box::pin(async { HookResult::Continue }),
			HookPriority::Background,
		);
	}
	runtime.schedule(
		Box::pin(async { HookResult::Continue }),
		HookPriority::Interactive,
	);

	assert_eq!(runtime.background_count(), 10);
	assert_eq!(runtime.interactive_count(), 1);

	runtime.drop_background();

	assert_eq!(runtime.background_count(), 0);
	assert_eq!(runtime.interactive_count(), 1);
	assert_eq!(runtime.dropped_total(), 10);
}

#[tokio::test]
async fn test_drain_budget_does_not_complete_slow_hooks() {
	let mut runtime = HookRuntime::new();

	runtime.schedule(
		Box::pin(async {
			tokio::time::sleep(Duration::from_millis(100)).await;
			HookResult::Continue
		}),
		HookPriority::Interactive,
	);

	runtime.drain_budget(Duration::from_millis(10)).await;
	assert!(runtime.has_pending());
	assert_eq!(runtime.completed_total(), 0);

	runtime.drain_budget(Duration::from_millis(200)).await;
	assert!(!runtime.has_pending());
	assert_eq!(runtime.completed_total(), 1);
}

#[tokio::test]
async fn test_backlog_threshold_exceeded() {
	let mut runtime = HookRuntime::new();

	for _ in 0..HOOK_BACKLOG_HIGH_WATER + 1 {
		runtime.schedule(
			Box::pin(async { HookResult::Continue }),
			HookPriority::Interactive,
		);
	}

	assert!(runtime.pending_count() > HOOK_BACKLOG_HIGH_WATER);
	runtime.drain_all().await;
	assert!(!runtime.has_pending());
}
