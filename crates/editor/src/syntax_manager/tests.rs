use std::sync::Mutex;

use tokio::sync::oneshot;

use super::*;

struct MockEngine {
	gate: Mutex<Option<oneshot::Receiver<()>>>,
	result: Mutex<Option<Result<Syntax, SyntaxError>>>,
}

impl MockEngine {
	fn new() -> Self {
		Self {
			gate: Mutex::new(None),
			result: Mutex::new(None),
		}
	}

	fn set_gate(&self, rx: oneshot::Receiver<()>) {
		*self.gate.lock().unwrap() = Some(rx);
	}

	fn set_result(&self, res: Result<Syntax, SyntaxError>) {
		*self.result.lock().unwrap() = Some(res);
	}
}

impl SyntaxEngine for MockEngine {
	fn parse(
		&self,
		_content: ropey::RopeSlice<'_>,
		_lang: LanguageId,
		_loader: &LanguageLoader,
		_opts: SyntaxOptions,
	) -> Result<Syntax, SyntaxError> {
		let gate = self.gate.lock().unwrap().take();
		if let Some(rx) = gate {
			let _ = rx.blocking_recv();
		}

		if let Some(res) = self.result.lock().unwrap().take() {
			res
		} else {
			Err(SyntaxError::Timeout)
		}
	}
}

#[tokio::test]
async fn test_inflight_drained_even_if_doc_marked_clean() {
	let engine = Arc::new(MockEngine::new());
	let (tx, rx) = oneshot::channel();
	engine.set_gate(rx);

	let mut mgr = SyntaxManager::new_with_engine(1, engine.clone());
	let mut policy = TieredSyntaxPolicy::default();
	policy.s.debounce = Duration::from_millis(0);
	policy.s.cooldown_on_timeout = Duration::from_millis(0);
	mgr.set_policy(policy);

	let doc_id = DocumentId(1);
	let loader = Arc::new(LanguageLoader::from_embedded());
	let lang_id = loader
		.language_for_name("rust")
		.expect("rust should be available in embedded loader");
	let content = Rope::from("test");

	let mut current = None;
	let mut dirty = true;
	let mut updated = false;
	let poll = mgr.ensure_syntax(
		EnsureSyntaxContext {
			doc_id,
			doc_version: 1,
			language_id: Some(lang_id),
			content: &content,
			hotness: SyntaxHotness::Visible,
			loader: &loader,
		},
		SyntaxSlot {
			current: &mut current,
			dirty: &mut dirty,
			updated: &mut updated,
		},
	);
	assert_eq!(poll, SyntaxPollResult::Kicked);
	assert!(mgr.has_pending(doc_id));

	dirty = false;
	let _ = tx.send(());
	tokio::time::sleep(Duration::from_millis(50)).await;

	let poll = mgr.ensure_syntax(
		EnsureSyntaxContext {
			doc_id,
			doc_version: 1,
			language_id: Some(lang_id),
			content: &content,
			hotness: SyntaxHotness::Visible,
			loader: &loader,
		},
		SyntaxSlot {
			current: &mut current,
			dirty: &mut dirty,
			updated: &mut updated,
		},
	);

	assert!(!mgr.has_pending(doc_id));
	assert!(matches!(
		poll,
		SyntaxPollResult::Ready | SyntaxPollResult::CoolingDown
	));
}

#[tokio::test]
async fn test_language_switch_discards_old_parse() {
	let engine = Arc::new(MockEngine::new());
	let (tx, rx) = oneshot::channel();
	engine.set_gate(rx);

	let mut mgr = SyntaxManager::new_with_engine(2, engine.clone());
	let mut policy = TieredSyntaxPolicy::default();
	policy.s.debounce = Duration::from_millis(0);
	policy.s.cooldown_on_timeout = Duration::from_millis(0);
	mgr.set_policy(policy);

	let doc_id = DocumentId(1);
	let loader = Arc::new(LanguageLoader::from_embedded());
	let lang_id_old = loader
		.language_for_name("rust")
		.expect("rust should be available");
	let lang_id_new = loader
		.language_for_name("python")
		.expect("python should be available");
	let content = Rope::from("test");

	let mut current = None;
	let mut dirty = true;
	let mut updated = false;
	mgr.ensure_syntax(
		EnsureSyntaxContext {
			doc_id,
			doc_version: 1,
			language_id: Some(lang_id_old),
			content: &content,
			hotness: SyntaxHotness::Visible,
			loader: &loader,
		},
		SyntaxSlot {
			current: &mut current,
			dirty: &mut dirty,
			updated: &mut updated,
		},
	);

	let _ = tx.send(());
	tokio::time::sleep(Duration::from_millis(50)).await;

	let poll = mgr.ensure_syntax(
		EnsureSyntaxContext {
			doc_id,
			doc_version: 1,
			language_id: Some(lang_id_new),
			content: &content,
			hotness: SyntaxHotness::Visible,
			loader: &loader,
		},
		SyntaxSlot {
			current: &mut current,
			dirty: &mut dirty,
			updated: &mut updated,
		},
	);

	assert!(current.is_none());
	let poll = if poll == SyntaxPollResult::CoolingDown {
		mgr.ensure_syntax(
			EnsureSyntaxContext {
				doc_id,
				doc_version: 1,
				language_id: Some(lang_id_new),
				content: &content,
				hotness: SyntaxHotness::Visible,
				loader: &loader,
			},
			SyntaxSlot {
				current: &mut current,
				dirty: &mut dirty,
				updated: &mut updated,
			},
		)
	} else {
		poll
	};
	assert_eq!(poll, SyntaxPollResult::Kicked);
}

#[tokio::test]
async fn test_dropwhenhidden_discards_completed_parse() {
	let engine = Arc::new(MockEngine::new());
	let (tx, rx) = oneshot::channel();
	engine.set_gate(rx);

	let mut mgr = SyntaxManager::new_with_engine(1, engine.clone());
	let mut policy = TieredSyntaxPolicy::default();
	policy.l.retention_hidden = RetentionPolicy::DropWhenHidden;
	policy.l.debounce = Duration::from_millis(0);
	policy.l.cooldown_on_timeout = Duration::from_millis(0);
	mgr.set_policy(policy);

	let doc_id = DocumentId(1);
	let loader = Arc::new(LanguageLoader::from_embedded());
	let lang_id = loader
		.language_for_name("rust")
		.expect("rust should be available");
	let content = Rope::from(" ".repeat(2 * 1024 * 1024));

	let mut current = None;
	let mut dirty = true;
	let mut updated = false;
	mgr.ensure_syntax(
		EnsureSyntaxContext {
			doc_id,
			doc_version: 1,
			language_id: Some(lang_id),
			content: &content,
			hotness: SyntaxHotness::Visible,
			loader: &loader,
		},
		SyntaxSlot {
			current: &mut current,
			dirty: &mut dirty,
			updated: &mut updated,
		},
	);

	let _ = tx.send(());
	tokio::time::sleep(Duration::from_millis(50)).await;

	mgr.ensure_syntax(
		EnsureSyntaxContext {
			doc_id,
			doc_version: 1,
			language_id: Some(lang_id),
			content: &content,
			hotness: SyntaxHotness::Cold,
			loader: &loader,
		},
		SyntaxSlot {
			current: &mut current,
			dirty: &mut dirty,
			updated: &mut updated,
		},
	);

	assert!(current.is_none());
	assert!(dirty);
}

/// Exhaustive truth table for the stale-inflight install guard.
///
/// The critical regression case is `(version_match=false, dirty=false,
/// has_current=true)` — a clean incremental tree at V1 must NOT be
/// overwritten by a stale background parse from V0.
#[test]
fn test_stale_parse_does_not_overwrite_clean_incremental() {
	use super::should_install_completed_parse;

	// (version_match, slot_dirty, has_current) -> expected
	let cases = [
		// The regression case: clean tree + stale result → MUST NOT install.
		(false, false, true, false),
		// Exact version match → always install.
		(true, false, true, true),
		(true, true, true, true),
		(true, false, false, true),
		(true, true, false, true),
		// Dirty slot → install stale for catch-up continuity.
		(false, true, true, true),
		(false, true, false, true),
		// No current syntax → install stale for bootstrap.
		(false, false, false, true),
	];

	for (version_match, dirty, has_current, expected) in cases {
		let result = should_install_completed_parse(version_match, dirty, has_current);
		assert_eq!(
			result, expected,
			"should_install_completed_parse(version_match={version_match}, dirty={dirty}, has_current={has_current}) = {result}, expected {expected}"
		);
	}
}

#[tokio::test]
async fn test_stale_install_continuity() {
	let engine = Arc::new(MockEngine::new());
	let (tx, rx) = oneshot::channel();
	engine.set_gate(rx);

	let mut mgr = SyntaxManager::new_with_engine(1, engine.clone());
	let mut policy = TieredSyntaxPolicy::default();
	policy.s.debounce = Duration::from_millis(0);
	policy.s.cooldown_on_timeout = Duration::from_millis(0);
	mgr.set_policy(policy);

	let doc_id = DocumentId(1);
	let loader = Arc::new(LanguageLoader::from_embedded());
	let lang_id = loader
		.language_for_name("rust")
		.expect("rust should be available in embedded loader");
	let content = Rope::from("test");

	// Setup a dummy syntax to return
	let dummy_syntax = Syntax::new(
		content.slice(..),
		lang_id,
		&loader,
		xeno_runtime_language::SyntaxOptions::default(),
	)
	.unwrap();
	engine.set_result(Ok(dummy_syntax));

	let mut current = None;
	let mut dirty = true;
	let mut updated = false;

	// 1. Kick off parse for V1
	mgr.ensure_syntax(
		EnsureSyntaxContext {
			doc_id,
			doc_version: 1,
			language_id: Some(lang_id),
			content: &content,
			hotness: SyntaxHotness::Visible,
			loader: &loader,
		},
		SyntaxSlot {
			current: &mut current,
			dirty: &mut dirty,
			updated: &mut updated,
		},
	);

	// 2. Doc version moves to V2 before V1 completes
	// 3. Complete V1 parse
	let _ = tx.send(());
	tokio::time::sleep(Duration::from_millis(50)).await;

	// 4. Poll with V2. Slot is still dirty, so V1 result SHOULD be installed (continuity).
	mgr.ensure_syntax(
		EnsureSyntaxContext {
			doc_id,
			doc_version: 2,
			language_id: Some(lang_id),
			content: &content,
			hotness: SyntaxHotness::Visible,
			loader: &loader,
		},
		SyntaxSlot {
			current: &mut current,
			dirty: &mut dirty,
			updated: &mut updated,
		},
	);

	assert!(current.is_some());
	assert!(updated);
	assert!(dirty); // dirty stays true because version mismatch (V1 != V2)
}

/// Bootstrap parse (no existing syntax tree) MUST skip the debounce gate
/// so that newly opened documents get highlighted immediately instead of
/// waiting for the debounce timeout to elapse.
#[tokio::test]
async fn test_bootstrap_parse_skips_debounce() {
	let engine = Arc::new(MockEngine::new());

	let mut mgr = SyntaxManager::new_with_engine(1, engine.clone());
	// Keep the default 80ms debounce — the point is to prove it's skipped.

	let doc_id = DocumentId(1);
	let loader = Arc::new(LanguageLoader::from_embedded());
	let lang_id = loader
		.language_for_name("rust")
		.expect("rust should be available in embedded loader");
	let content = Rope::from("fn main() {}");

	let mut current = None;
	let mut dirty = true;
	let mut updated = false;

	// First call with no existing syntax tree must kick immediately.
	let poll = mgr.ensure_syntax(
		EnsureSyntaxContext {
			doc_id,
			doc_version: 1,
			language_id: Some(lang_id),
			content: &content,
			hotness: SyntaxHotness::Visible,
			loader: &loader,
		},
		SyntaxSlot {
			current: &mut current,
			dirty: &mut dirty,
			updated: &mut updated,
		},
	);
	assert_eq!(
		poll,
		SyntaxPollResult::Kicked,
		"bootstrap parse must skip debounce and kick immediately"
	);
	assert!(mgr.has_pending(doc_id));
}

#[test]
fn test_note_edit_updates_timestamp() {
	let mut mgr = SyntaxManager::default();
	let doc_id = DocumentId(1);

	mgr.note_edit(doc_id);
	let t1 = mgr.docs.get(&doc_id).unwrap().last_edit_at;

	std::thread::sleep(Duration::from_millis(1));
	mgr.note_edit(doc_id);
	let t2 = mgr.docs.get(&doc_id).unwrap().last_edit_at;

	assert!(t2 > t1);
}

#[tokio::test]
async fn test_idle_tick_polls_inflight_parse() {
	let engine = Arc::new(MockEngine::new());
	let (tx, rx) = oneshot::channel();
	engine.set_gate(rx);

	let mut mgr = SyntaxManager::new_with_engine(1, engine.clone());
	let mut policy = TieredSyntaxPolicy::default();
	policy.s.debounce = Duration::from_millis(0);
	mgr.set_policy(policy);

	let doc_id = DocumentId(1);
	let loader = Arc::new(LanguageLoader::from_embedded());
	let lang_id = loader.language_for_name("rust").unwrap();
	let content = Rope::from("test");

	let mut current = None;
	let mut dirty = true;
	let mut updated = false;

	// 1. Kick off parse
	mgr.ensure_syntax(
		EnsureSyntaxContext {
			doc_id,
			doc_version: 1,
			language_id: Some(lang_id),
			content: &content,
			hotness: SyntaxHotness::Visible,
			loader: &loader,
		},
		SyntaxSlot {
			current: &mut current,
			dirty: &mut dirty,
			updated: &mut updated,
		},
	);

	assert!(mgr.has_pending(doc_id));
	assert!(!mgr.any_task_finished());

	// 2. Complete parse
	let _ = tx.send(());
	tokio::time::sleep(Duration::from_millis(50)).await;

	// 3. Check any_task_finished (simulating what Editor::tick does)
	assert!(mgr.any_task_finished());
}

#[tokio::test]
async fn test_single_flight_per_doc() {
	let engine = Arc::new(MockEngine::new());
	let (_tx, rx) = oneshot::channel();
	engine.set_gate(rx); // Parse will block

	let mut mgr = SyntaxManager::new_with_engine(2, engine.clone());
	let mut policy = TieredSyntaxPolicy::default();
	policy.s.debounce = Duration::from_millis(0);
	mgr.set_policy(policy);

	let doc_id = DocumentId(1);
	let loader = Arc::new(LanguageLoader::from_embedded());
	let lang_id = loader.language_for_name("rust").unwrap();
	let content = Rope::from("test");

	let mut current = None;
	let mut dirty = true;
	let mut updated = false;

	// 1. Kick off first parse
	let poll1 = mgr.ensure_syntax(
		EnsureSyntaxContext {
			doc_id,
			doc_version: 1,
			language_id: Some(lang_id),
			content: &content,
			hotness: SyntaxHotness::Visible,
			loader: &loader,
		},
		SyntaxSlot {
			current: &mut current,
			dirty: &mut dirty,
			updated: &mut updated,
		},
	);
	assert_eq!(poll1, SyntaxPollResult::Kicked);
	assert!(mgr.has_pending(doc_id));

	// 2. Try to kick off another parse for same doc while first is running
	let poll2 = mgr.ensure_syntax(
		EnsureSyntaxContext {
			doc_id,
			doc_version: 2,
			language_id: Some(lang_id),
			content: &content,
			hotness: SyntaxHotness::Visible,
			loader: &loader,
		},
		SyntaxSlot {
			current: &mut current,
			dirty: &mut dirty,
			updated: &mut updated,
		},
	);

	assert_eq!(poll2, SyntaxPollResult::Pending);
	assert_eq!(
		mgr.pending_count(),
		1,
		"Should only have one task for this doc"
	);
}
