use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use super::*;

#[derive(Default)]
struct CountingActor {
	seen: usize,
}

#[async_trait]
impl WorkerActor for CountingActor {
	type Cmd = usize;
	type Evt = usize;

	async fn handle(&mut self, cmd: Self::Cmd, ctx: &mut ActorContext<Self::Evt>) -> Result<ActorFlow, String> {
		self.seen = self.seen.wrapping_add(1);
		ctx.emit(cmd);
		if cmd == 99 { Ok(ActorFlow::Stop) } else { Ok(ActorFlow::Continue) }
	}
}

#[tokio::test]
async fn actor_emits_events_and_stops() {
	let handle = spawn_supervised_actor(ActorSpec::new("counting", TaskClass::Interactive, CountingActor::default));
	let mut events = handle.subscribe();
	let _ = handle.send(1).await;
	let _ = handle.send(99).await;

	assert_eq!(events.recv().await.ok(), Some(1));
	assert_eq!(events.recv().await.ok(), Some(99));

	let report = handle
		.shutdown(ActorShutdownMode::Graceful {
			timeout: Duration::from_secs(1),
		})
		.await;
	assert!(report.completed());
}

struct FailingActor {
	start_counter: Arc<AtomicUsize>,
}

#[async_trait]
impl WorkerActor for FailingActor {
	type Cmd = ();
	type Evt = ();

	async fn on_start(&mut self, _ctx: &mut ActorContext<Self::Evt>) -> Result<(), String> {
		self.start_counter.fetch_add(1, Ordering::SeqCst);
		Ok(())
	}

	async fn handle(&mut self, _cmd: Self::Cmd, _ctx: &mut ActorContext<Self::Evt>) -> Result<ActorFlow, String> {
		Err("boom".to_string())
	}
}

#[tokio::test]
async fn supervisor_restarts_on_handler_failure() {
	let starts = Arc::new(AtomicUsize::new(0));
	let starts_clone = Arc::clone(&starts);
	let spec = ActorSpec::new("failing", TaskClass::Background, move || FailingActor {
		start_counter: Arc::clone(&starts_clone),
	})
	.supervisor(ActorSupervisorSpec {
		restart: ActorRestartPolicy::OnFailure {
			max_restarts: 2,
			backoff: Duration::from_millis(1),
		},
		event_buffer: 8,
	});

	let handle = spawn_supervised_actor(spec);
	let _ = handle.send(()).await;
	tokio::time::sleep(Duration::from_millis(20)).await;
	handle.cancel();
	let _ = handle.shutdown(ActorShutdownMode::Immediate).await;

	assert!(starts.load(Ordering::SeqCst) >= 2, "actor should restart after failure");
}

struct SlowActor;

#[async_trait]
impl WorkerActor for SlowActor {
	type Cmd = ();
	type Evt = &'static str;

	async fn handle(&mut self, _cmd: Self::Cmd, ctx: &mut ActorContext<Self::Evt>) -> Result<ActorFlow, String> {
		ctx.emit("entered");
		tokio::time::sleep(Duration::from_secs(60)).await;
		Ok(ActorFlow::Continue)
	}
}

#[tokio::test]
async fn immediate_shutdown_preempts_slow_handler() {
	let handle = spawn_supervised_actor(ActorSpec::new("slow", TaskClass::Background, || SlowActor).supervisor(ActorSupervisorSpec {
		restart: ActorRestartPolicy::Never,
		event_buffer: 8,
	}));
	let mut events = handle.subscribe();

	let _ = handle.send(()).await;
	// Wait until handle() is entered (event emitted before the long sleep).
	let got = tokio::time::timeout(Duration::from_secs(2), events.recv()).await;
	assert_eq!(got.ok().and_then(|r| r.ok()), Some("entered"), "actor should enter handle()");

	// Immediate shutdown must complete quickly despite the 60s sleep.
	let report = tokio::time::timeout(Duration::from_millis(500), handle.shutdown(ActorShutdownMode::Immediate))
		.await
		.expect("shutdown should not hang");
	assert!(report.completed());
	assert_eq!(report.last_exit().map(|e| e.kind()), Some(ActorExitKind::Cancelled));
}

struct SlowStopActor {
	stopped: Arc<std::sync::atomic::AtomicBool>,
}

#[async_trait]
impl WorkerActor for SlowStopActor {
	type Cmd = ();
	type Evt = &'static str;

	async fn handle(&mut self, _cmd: Self::Cmd, ctx: &mut ActorContext<Self::Evt>) -> Result<ActorFlow, String> {
		ctx.emit("entered");
		tokio::time::sleep(Duration::from_secs(60)).await;
		Ok(ActorFlow::Continue)
	}

	async fn on_stop(&mut self, _ctx: &mut ActorContext<Self::Evt>) {
		tokio::time::sleep(Duration::from_millis(200)).await;
		self.stopped.store(true, Ordering::SeqCst);
	}
}

#[tokio::test]
async fn graceful_timeout_retains_handle_for_immediate_followup() {
	let stopped = Arc::new(std::sync::atomic::AtomicBool::new(false));
	let stopped_clone = Arc::clone(&stopped);
	let handle = spawn_supervised_actor(
		ActorSpec::new("slow-stop", TaskClass::Background, move || SlowStopActor {
			stopped: Arc::clone(&stopped_clone),
		})
		.supervisor(ActorSupervisorSpec {
			restart: ActorRestartPolicy::Never,
			event_buffer: 8,
		}),
	);
	let mut events = handle.subscribe();

	let _ = handle.send(()).await;
	let got = tokio::time::timeout(Duration::from_secs(2), events.recv()).await;
	assert_eq!(got.ok().and_then(|r| r.ok()), Some("entered"));

	// Graceful with very short timeout — will time out while handle() sleeps.
	let report = handle
		.shutdown(ActorShutdownMode::Graceful {
			timeout: Duration::from_millis(10),
		})
		.await;
	assert!(report.timed_out());
	assert!(!report.completed());
	// on_stop hasn't run yet (cancel just fired, actor still tearing down).
	assert!(!stopped.load(Ordering::SeqCst));

	// Follow-up Immediate must join the supervisor and wait for on_stop to finish.
	let report = tokio::time::timeout(Duration::from_secs(2), handle.shutdown(ActorShutdownMode::Immediate))
		.await
		.expect("immediate after graceful should not hang");
	assert!(report.completed());
	assert!(stopped.load(Ordering::SeqCst), "on_stop should have completed");
}

#[derive(Default)]
struct NoopActor;

#[async_trait]
impl WorkerActor for NoopActor {
	type Cmd = ();
	type Evt = ();

	async fn handle(&mut self, _cmd: Self::Cmd, _ctx: &mut ActorContext<Self::Evt>) -> Result<ActorFlow, String> {
		Ok(ActorFlow::Continue)
	}
}

#[tokio::test]
async fn graceful_shutdown_terminates_with_restart_on_failure() {
	let handle = spawn_supervised_actor(
		ActorSpec::new("restart-shutdown", TaskClass::Background, NoopActor::default).supervisor(ActorSupervisorSpec {
			restart: ActorRestartPolicy::OnFailure {
				max_restarts: 5,
				backoff: Duration::from_millis(1),
			},
			event_buffer: 8,
		}),
	);

	// Don't send anything — just graceful-shutdown immediately.
	let report = handle
		.shutdown(ActorShutdownMode::Graceful {
			timeout: Duration::from_millis(200),
		})
		.await;
	assert!(report.completed(), "graceful shutdown should complete promptly");
	assert!(!report.timed_out());
}

#[tokio::test]
async fn shutdown_graceful_or_force_completes_with_slow_stop() {
	let stopped = Arc::new(std::sync::atomic::AtomicBool::new(false));
	let stopped_clone = Arc::clone(&stopped);
	let handle = spawn_supervised_actor(
		ActorSpec::new("force-test", TaskClass::Background, move || SlowStopActor {
			stopped: Arc::clone(&stopped_clone),
		})
		.supervisor(ActorSupervisorSpec {
			restart: ActorRestartPolicy::Never,
			event_buffer: 8,
		}),
	);
	let mut events = handle.subscribe();

	let _ = handle.send(()).await;
	let got = tokio::time::timeout(Duration::from_secs(2), events.recv()).await;
	assert_eq!(got.ok().and_then(|r| r.ok()), Some("entered"));

	// Graceful with tiny timeout will time out (handler sleeps 60s),
	// then force immediate which cancels the handler + runs on_stop.
	let report = tokio::time::timeout(Duration::from_secs(2), handle.shutdown_graceful_or_force(Duration::from_millis(10)))
		.await
		.expect("shutdown_graceful_or_force should not hang");
	assert!(report.completed());
	assert!(stopped.load(Ordering::SeqCst), "on_stop should have completed via forced immediate");
}

#[tokio::test]
async fn cancel_closes_mailbox_and_send_fails_fast() {
	let handle =
		spawn_supervised_actor(ActorSpec::new("cancel-close", TaskClass::Background, CountingActor::default).mailbox(ActorMailboxSpec { capacity: 1 }));

	handle.cancel();

	// send() must fail fast (not block on backpressure) since mailbox is closed.
	let result = tokio::time::timeout(Duration::from_millis(50), handle.send(1)).await;
	assert!(result.is_ok(), "send should not block after cancel");
	assert!(result.unwrap().is_err(), "send should return error on closed mailbox");

	let report = handle.shutdown(ActorShutdownMode::Immediate).await;
	assert!(report.completed());
}

struct ConcurrentStopActor {
	started_stop: Arc<std::sync::atomic::AtomicBool>,
	done_stop: Arc<std::sync::atomic::AtomicBool>,
}

#[async_trait]
impl WorkerActor for ConcurrentStopActor {
	type Cmd = ();
	type Evt = &'static str;

	async fn handle(&mut self, _cmd: Self::Cmd, ctx: &mut ActorContext<Self::Evt>) -> Result<ActorFlow, String> {
		ctx.emit("entered");
		tokio::time::sleep(Duration::from_secs(60)).await;
		Ok(ActorFlow::Continue)
	}

	async fn on_stop(&mut self, _ctx: &mut ActorContext<Self::Evt>) {
		self.started_stop.store(true, Ordering::SeqCst);
		tokio::time::sleep(Duration::from_millis(200)).await;
		self.done_stop.store(true, Ordering::SeqCst);
	}
}

#[tokio::test]
async fn concurrent_shutdown_waits_for_in_progress_join() {
	let started_stop = Arc::new(std::sync::atomic::AtomicBool::new(false));
	let done_stop = Arc::new(std::sync::atomic::AtomicBool::new(false));
	let ss = Arc::clone(&started_stop);
	let ds = Arc::clone(&done_stop);
	let handle = Arc::new(spawn_supervised_actor(
		ActorSpec::new("concurrent-shutdown", TaskClass::Background, move || ConcurrentStopActor {
			started_stop: Arc::clone(&ss),
			done_stop: Arc::clone(&ds),
		})
		.supervisor(ActorSupervisorSpec {
			restart: ActorRestartPolicy::Never,
			event_buffer: 8,
		}),
	));
	let mut events = handle.subscribe();

	let _ = handle.send(()).await;
	let got = tokio::time::timeout(Duration::from_secs(2), events.recv()).await;
	assert_eq!(got.ok().and_then(|r| r.ok()), Some("entered"));

	// Caller A starts Immediate shutdown.
	let handle_a = Arc::clone(&handle);
	let task_a = tokio::spawn(async move { handle_a.shutdown(ActorShutdownMode::Immediate).await });

	// Wait until on_stop has started (proves A is the leader, joining).
	while !started_stop.load(Ordering::SeqCst) {
		tokio::task::yield_now().await;
	}
	// on_stop started but not done yet.
	assert!(!done_stop.load(Ordering::SeqCst));

	// Caller B also calls Immediate shutdown concurrently.
	let report_b = tokio::time::timeout(Duration::from_secs(2), handle.shutdown(ActorShutdownMode::Immediate))
		.await
		.expect("concurrent shutdown B should not hang");
	// B must wait until on_stop finishes (not return early).
	assert!(report_b.completed);
	assert!(done_stop.load(Ordering::SeqCst), "concurrent caller must see on_stop completed");

	let report_a = task_a.await.unwrap();
	assert!(report_a.completed);
}

// ── Restart + cancellation invariant tests ──

/// Actor that tracks generation token cancellation across restarts.
///
/// On start, spawns a background task scoped to the generation token.
/// Each tick increments `active_tickers`. On cancel, decrements it.
/// If the supervisor properly cancels old generations on restart,
/// `active_tickers` should never exceed 1.
struct ZombieDetectorActor {
	active_tickers: Arc<AtomicUsize>,
	peak_tickers: Arc<AtomicUsize>,
	starts: Arc<AtomicUsize>,
	fail_first_n: Arc<AtomicUsize>,
}

#[async_trait]
impl WorkerActor for ZombieDetectorActor {
	type Cmd = ();
	type Evt = ();

	async fn on_start(&mut self, ctx: &mut ActorContext<Self::Evt>) -> Result<(), String> {
		let generation_id = ctx.generation();
		let count = self.starts.fetch_add(1, Ordering::SeqCst) + 1;
		let remaining_failures = self.fail_first_n.load(Ordering::SeqCst);

		// Spawn a background "ticker" scoped to this generation's token.
		let active = Arc::clone(&self.active_tickers);
		let peak = Arc::clone(&self.peak_tickers);
		let token = ctx.token.clone();
		tokio::spawn(async move {
			let cur = active.fetch_add(1, Ordering::SeqCst) + 1;
			// Track peak concurrent tickers.
			peak.fetch_max(cur, Ordering::SeqCst);

			// Keep ticking until cancelled.
			loop {
				tokio::select! {
					biased;
					_ = token.cancelled() => break,
					_ = tokio::time::sleep(Duration::from_millis(1)) => {}
				}
			}
			active.fetch_sub(1, Ordering::SeqCst);
			let _ = generation_id;
		});

		if count <= remaining_failures {
			Err(format!("deliberate startup failure #{count}"))
		} else {
			Ok(())
		}
	}

	async fn handle(&mut self, _cmd: Self::Cmd, _ctx: &mut ActorContext<Self::Evt>) -> Result<ActorFlow, String> {
		Ok(ActorFlow::Continue)
	}
}

#[tokio::test]
async fn no_zombie_tickers_across_restarts() {
	let active_tickers = Arc::new(AtomicUsize::new(0));
	let peak_tickers = Arc::new(AtomicUsize::new(0));
	let starts = Arc::new(AtomicUsize::new(0));
	let fail_first_n = Arc::new(AtomicUsize::new(3)); // fail first 3 starts

	let at = Arc::clone(&active_tickers);
	let pt = Arc::clone(&peak_tickers);
	let st = Arc::clone(&starts);
	let ff = Arc::clone(&fail_first_n);

	let handle = spawn_supervised_actor(
		ActorSpec::new("zombie-detector", TaskClass::Background, move || ZombieDetectorActor {
			active_tickers: Arc::clone(&at),
			peak_tickers: Arc::clone(&pt),
			starts: Arc::clone(&st),
			fail_first_n: Arc::clone(&ff),
		})
		.supervisor(ActorSupervisorSpec {
			restart: ActorRestartPolicy::OnFailure {
				max_restarts: 5,
				backoff: Duration::from_millis(1),
			},
			event_buffer: 8,
		}),
	);

	// Wait for restarts to settle (3 failures + 1 success = 4 starts).
	tokio::time::sleep(Duration::from_millis(100)).await;
	assert_eq!(starts.load(Ordering::SeqCst), 4, "should start 4 times (3 failures + 1 success)");

	// After settling, exactly one ticker should be active.
	assert_eq!(active_tickers.load(Ordering::SeqCst), 1, "exactly one ticker should be active after restarts");

	// Shutdown and verify all tickers stop.
	handle.cancel();
	let report = handle.shutdown(ActorShutdownMode::Immediate).await;
	assert!(report.completed());

	// Give ticker tasks a moment to observe cancellation.
	tokio::time::sleep(Duration::from_millis(20)).await;
	assert_eq!(active_tickers.load(Ordering::SeqCst), 0, "all tickers should stop after shutdown");

	// Peak should reflect zombie accumulation if cancellation is broken.
	// With correct per-generation cancellation, peak should be 1.
	// With broken cancellation (all share parent token), peak could be up to 4.
	let peak = peak_tickers.load(Ordering::SeqCst);
	assert_eq!(peak, 1, "peak concurrent tickers should be 1 (no zombies); got {peak}");
}

#[tokio::test]
async fn shutdown_during_backoff_completes_promptly() {
	let starts = Arc::new(AtomicUsize::new(0));
	let starts_clone = Arc::clone(&starts);

	// Actor that fails on first message, triggering OnFailure restart + backoff.
	struct FailOnceActor;
	#[async_trait]
	impl WorkerActor for FailOnceActor {
		type Cmd = ();
		type Evt = ();
		async fn handle(&mut self, _cmd: (), _ctx: &mut ActorContext<()>) -> Result<ActorFlow, String> {
			Err("deliberate failure".into())
		}
	}

	let handle = spawn_supervised_actor(
		ActorSpec::new("backoff-shutdown", TaskClass::Background, move || {
			let s = Arc::clone(&starts_clone);
			s.fetch_add(1, Ordering::SeqCst);
			FailOnceActor
		})
		.supervisor(ActorSupervisorSpec {
			restart: ActorRestartPolicy::OnFailure {
				max_restarts: 5,
				backoff: Duration::from_secs(60), // very long backoff
			},
			event_buffer: 8,
		}),
	);

	// Trigger failure → supervisor enters 60s backoff sleep.
	let _ = handle.send(()).await;
	tokio::time::sleep(Duration::from_millis(20)).await;

	let starts_before = starts.load(Ordering::SeqCst);
	assert_eq!(starts_before, 1, "only one start so far");

	// Shutdown must complete promptly despite the 60s backoff.
	let report = tokio::time::timeout(Duration::from_millis(500), handle.shutdown(ActorShutdownMode::Immediate))
		.await
		.expect("shutdown should not hang during backoff");
	assert!(report.completed());

	// No additional restart should have occurred.
	assert_eq!(starts.load(Ordering::SeqCst), 1, "no restart after shutdown during backoff");
}

#[tokio::test]
async fn panic_path_triggers_restart_same_as_error() {
	let starts = Arc::new(AtomicUsize::new(0));

	struct PanicOnStartActor {
		starts: Arc<AtomicUsize>,
	}

	#[async_trait]
	impl WorkerActor for PanicOnStartActor {
		type Cmd = ();
		type Evt = ();

		async fn on_start(&mut self, _ctx: &mut ActorContext<Self::Evt>) -> Result<(), String> {
			self.starts.fetch_add(1, Ordering::SeqCst);
			panic!("deliberate startup panic");
		}

		async fn handle(&mut self, _cmd: Self::Cmd, _ctx: &mut ActorContext<Self::Evt>) -> Result<ActorFlow, String> {
			unreachable!();
		}
	}

	let starts_clone = Arc::clone(&starts);
	let handle = spawn_supervised_actor(
		ActorSpec::new("panic-restart", TaskClass::Background, move || PanicOnStartActor {
			starts: Arc::clone(&starts_clone),
		})
		.supervisor(ActorSupervisorSpec {
			restart: ActorRestartPolicy::OnFailure {
				max_restarts: 2,
				backoff: Duration::from_millis(1),
			},
			event_buffer: 8,
		}),
	);

	tokio::time::sleep(Duration::from_millis(100)).await;

	// Should have started 3 times (initial + 2 restarts).
	let total_starts = starts.load(Ordering::SeqCst);
	assert_eq!(total_starts, 3, "panic should trigger same restart logic as error");

	// Final exit should be Panicked.
	let last_exit = handle.last_exit().await;
	assert_eq!(last_exit.as_ref().map(|e| e.kind()), Some(ActorExitKind::Panicked));

	handle.cancel();
	let report = handle.shutdown(ActorShutdownMode::Immediate).await;
	assert!(report.completed());
}

#[tokio::test]
async fn max_restarts_honored_then_stops() {
	let starts = Arc::new(AtomicUsize::new(0));

	struct AlwaysFailActor {
		starts: Arc<AtomicUsize>,
	}

	#[async_trait]
	impl WorkerActor for AlwaysFailActor {
		type Cmd = ();
		type Evt = ();

		async fn on_start(&mut self, _ctx: &mut ActorContext<Self::Evt>) -> Result<(), String> {
			let count = self.starts.fetch_add(1, Ordering::SeqCst) + 1;
			Err(format!("fail #{count}"))
		}

		async fn handle(&mut self, _cmd: Self::Cmd, _ctx: &mut ActorContext<Self::Evt>) -> Result<ActorFlow, String> {
			unreachable!("on_start always fails");
		}
	}

	let starts_clone = Arc::clone(&starts);
	let handle = spawn_supervised_actor(
		ActorSpec::new("max-restarts", TaskClass::Background, move || AlwaysFailActor {
			starts: Arc::clone(&starts_clone),
		})
		.supervisor(ActorSupervisorSpec {
			restart: ActorRestartPolicy::OnFailure {
				max_restarts: 3,
				backoff: Duration::from_millis(1),
			},
			event_buffer: 8,
		}),
	);

	// Wait for all restarts to exhaust.
	tokio::time::sleep(Duration::from_millis(100)).await;

	// initial (1) + 3 restarts = 4 total starts.
	let total = starts.load(Ordering::SeqCst);
	assert_eq!(total, 4, "should start exactly 1 + max_restarts times");

	let last_exit = handle.last_exit().await;
	assert!(
		last_exit.as_ref().map(|e| e.kind()) == Some(ActorExitKind::StartupFailed),
		"final exit should be StartupFailed, got {last_exit:?}"
	);

	// Supervisor should have already exited (no more restarts).
	let report = tokio::time::timeout(Duration::from_millis(100), handle.shutdown(ActorShutdownMode::Immediate))
		.await
		.expect("shutdown should complete quickly when supervisor already exited");
	assert!(report.completed());
}

#[tokio::test]
async fn generation_advances_on_each_restart() {
	let generations = Arc::new(Mutex::new(Vec::<u64>::new()));

	struct GenTrackingActor {
		generations: Arc<Mutex<Vec<u64>>>,
	}

	#[async_trait]
	impl WorkerActor for GenTrackingActor {
		type Cmd = ();
		type Evt = ();

		async fn on_start(&mut self, ctx: &mut ActorContext<Self::Evt>) -> Result<(), String> {
			self.generations.lock().await.push(ctx.generation());
			Err("fail".to_string())
		}

		async fn handle(&mut self, _cmd: Self::Cmd, _ctx: &mut ActorContext<Self::Evt>) -> Result<ActorFlow, String> {
			unreachable!();
		}
	}

	let gens = Arc::clone(&generations);
	let handle = spawn_supervised_actor(
		ActorSpec::new("gen-tracking", TaskClass::Background, move || GenTrackingActor {
			generations: Arc::clone(&gens),
		})
		.supervisor(ActorSupervisorSpec {
			restart: ActorRestartPolicy::OnFailure {
				max_restarts: 3,
				backoff: Duration::from_millis(1),
			},
			event_buffer: 8,
		}),
	);

	tokio::time::sleep(Duration::from_millis(100)).await;
	handle.cancel();
	let _ = handle.shutdown(ActorShutdownMode::Immediate).await;

	let gens = generations.lock().await;
	assert_eq!(gens.len(), 4, "4 starts = 4 generations");

	// Generations must be strictly monotonically increasing.
	for window in gens.windows(2) {
		assert!(window[1] > window[0], "generations must be strictly increasing: {gens:?}");
	}

	// Handle's generation() should match the last one.
	assert_eq!(handle.generation(), *gens.last().unwrap());
}
