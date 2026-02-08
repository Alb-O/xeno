use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use parking_lot::Mutex;
use xeno_registry::hooks::HookPriority;

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
