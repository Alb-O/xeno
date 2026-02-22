use super::*;

/// Typing edit on an aligned S-tier full tree performs sync incremental:
/// bumps doc_version, clears pending, clears dirty, rotates tree_id, no BG task.
#[test]
fn sync_incremental_typing_updates_full_tree_without_bg_parse() {
	let loader = Arc::new(LanguageLoader::from_embedded());
	let lang = loader.language_for_name("rust").unwrap();
	let content = Rope::from("fn main() {}");
	let doc_id = DocumentId(10);

	let mut mgr = SyntaxManager::new(Default::default());

	// Bootstrap at v1
	let outcome = mgr.ensure_syntax(EnsureSyntaxContext {
		doc_id,
		doc_version: 1,
		language_id: Some(lang),
		content: &content,
		hotness: SyntaxHotness::Visible,
		loader: &loader,
		viewport: None,
	});
	assert_eq!(outcome.result, SyntaxPollResult::Ready);

	let s1 = mgr.debug_doc_state(doc_id).unwrap();
	let tree_id_v1 = s1.full_tree_id.unwrap();
	let syntax_version_v1 = mgr.syntax_version(doc_id);

	// Typing edit: real one-char insertion (space after "fn")
	let old_rope = content.clone();
	let tx = xeno_primitives::Transaction::change(
		old_rope.slice(..),
		[xeno_primitives::Change {
			start: 2,
			end: 2,
			replacement: Some(" ".into()),
		}],
	);
	let mut new_rope = old_rope.clone();
	tx.apply(&mut new_rope);
	mgr.note_edit_incremental(doc_id, 2, &old_rope, &new_rope, tx.changes(), &loader, EditSource::Typing);

	let s2 = mgr.debug_doc_state(doc_id).unwrap();
	assert_eq!(s2.full_doc_version, Some(2), "sync incremental must bump full tree to v2");
	assert!(s2.pending_base_version.is_none(), "sync incremental must clear pending");
	assert!(!s2.dirty, "sync incremental must clear dirty");
	assert_ne!(s2.full_tree_id.unwrap(), tree_id_v1, "sync incremental must rotate tree_id");
	assert!(mgr.syntax_version(doc_id) > syntax_version_v1, "change_id must bump");

	// Subsequent ensure_syntax should be Ready with no BG work
	let outcome2 = mgr.ensure_syntax(EnsureSyntaxContext {
		doc_id,
		doc_version: 2,
		language_id: Some(lang),
		content: &new_rope,
		hotness: SyntaxHotness::Visible,
		loader: &loader,
		viewport: None,
	});
	assert_eq!(outcome2.result, SyntaxPollResult::Ready);

	let s3 = mgr.debug_doc_state(doc_id).unwrap();
	assert!(!s3.bg_inflight, "no BG task should be spawned after sync incremental");
	assert!(!s3.has_completed, "no completed tasks in queue");
}

// --- Projection ctx safety golden tests ---

/// Projection context is present when pending edits are aligned to resident tree.
#[test]
fn projection_ctx_present_when_pending_aligned() {
	let loader = Arc::new(LanguageLoader::from_embedded());
	let lang = loader.language_for_name("rust").unwrap();
	let content = Rope::from("fn main() {}");
	let doc_id = DocumentId(11);

	let mut mgr = SyntaxManager::new(Default::default());

	// Bootstrap at v1
	mgr.ensure_syntax(EnsureSyntaxContext {
		doc_id,
		doc_version: 1,
		language_id: Some(lang),
		content: &content,
		hotness: SyntaxHotness::Visible,
		loader: &loader,
		viewport: None,
	});
	assert_eq!(mgr.debug_doc_state(doc_id).unwrap().full_doc_version, Some(1));

	// History edit at v2 preserves baseline (doesn't sync incremental)
	let changeset = xeno_primitives::ChangeSet::new(content.slice(..));
	mgr.note_edit_incremental(doc_id, 2, &content, &content, &changeset, &loader, EditSource::History);

	let state = mgr.debug_doc_state(doc_id).unwrap();
	assert_eq!(state.full_doc_version, Some(1), "history edit preserves resident tree version");
	assert_eq!(state.pending_base_version, Some(1), "pending anchored to resident tree");

	// Projection context should be available
	let proj = mgr.highlight_projection_ctx_for(doc_id, 1, 2);
	assert!(proj.is_some(), "projection ctx must be present when pending is aligned");
	let proj = proj.unwrap();
	assert_eq!(proj.tree_doc_version, 1);
	assert_eq!(proj.target_doc_version, 2);
}

/// Projection context is absent when tree and target versions already match.
#[test]
fn projection_ctx_absent_when_versions_match() {
	let loader = Arc::new(LanguageLoader::from_embedded());
	let lang = loader.language_for_name("rust").unwrap();
	let content = Rope::from("fn main() {}");
	let doc_id = DocumentId(12);

	let mut mgr = SyntaxManager::new(Default::default());

	// Bootstrap at v1
	mgr.ensure_syntax(EnsureSyntaxContext {
		doc_id,
		doc_version: 1,
		language_id: Some(lang),
		content: &content,
		hotness: SyntaxHotness::Visible,
		loader: &loader,
		viewport: None,
	});

	// Same version → no projection needed
	let proj = mgr.highlight_projection_ctx_for(doc_id, 1, 1);
	assert!(proj.is_none(), "projection ctx must be None when tree == target version");
}

/// Projection context is absent when pending base doesn't match tree version.
#[test]
fn projection_ctx_absent_when_base_misaligned() {
	let loader = Arc::new(LanguageLoader::from_embedded());
	let lang = loader.language_for_name("rust").unwrap();
	let content = Rope::from("fn main() {}");
	let doc_id = DocumentId(13);

	let mut mgr = SyntaxManager::new(Default::default());

	// Bootstrap at v1
	mgr.ensure_syntax(EnsureSyntaxContext {
		doc_id,
		doc_version: 1,
		language_id: Some(lang),
		content: &content,
		hotness: SyntaxHotness::Visible,
		loader: &loader,
		viewport: None,
	});

	// History edit at v2 → pending anchored to v1
	let changeset = xeno_primitives::ChangeSet::new(content.slice(..));
	mgr.note_edit_incremental(doc_id, 2, &content, &content, &changeset, &loader, EditSource::History);

	// Ask for projection from tree_version=3 (which doesn't match pending base=1)
	let proj = mgr.highlight_projection_ctx_for(doc_id, 3, 4);
	assert!(proj.is_none(), "projection ctx must be None when tree version doesn't match pending base");
}
