use std::sync::Mutex;

use tokio::sync::oneshot;

use super::*;

struct MockEngine {
	gate: Mutex<Option<oneshot::Receiver<()>>>,
}

impl MockEngine {
	fn new() -> Self {
		Self {
			gate: Mutex::new(None),
		}
	}

	fn set_gate(&self, rx: oneshot::Receiver<()>) {
		*self.gate.lock().unwrap() = Some(rx);
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

		Err(SyntaxError::Timeout)
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
	let loader = Arc::new(LanguageLoader::new());
	let lang_id = loader
		.language_for_name("rust")
		.unwrap_or_else(|| LanguageId::new(0u32));
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
	let loader = Arc::new(LanguageLoader::new());
	let lang_id_old = loader
		.language_for_name("rust")
		.unwrap_or_else(|| LanguageId::new(1u32));
	let lang_id_new = loader
		.language_for_name("python")
		.unwrap_or_else(|| LanguageId::new(2u32));
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
	let loader = Arc::new(LanguageLoader::new());
	let lang_id = loader
		.language_for_name("rust")
		.unwrap_or_else(|| LanguageId::new(1u32));
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
