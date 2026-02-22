use super::*;

#[tokio::test]
async fn barrier_ignored_after_doc_close() {
	let documents = Arc::new(DocumentStateManager::new());
	let path = Path::new("/barrier_close.rs");
	let uri = documents.register(path, Some("rust")).unwrap();
	documents.mark_opened(&uri, 0);

	// Queue a change and create a barrier that we control.
	let version = documents.queue_change(&uri).unwrap();
	let (barrier_tx, barrier_rx) = oneshot::channel();

	let transport = Arc::new(SimpleStubTransport);
	let registry = Arc::new(Registry::new(transport));
	let sync = DocumentSync::with_registry(registry, documents.clone());

	let completion_rx = sync.wrap_barrier(uri.clone(), version, barrier_rx);

	// Close the document before the barrier resolves.
	documents.unregister(&uri);

	// Resolve the barrier — the ack should be skipped.
	barrier_tx.send(Ok(())).unwrap();
	completion_rx.await.unwrap();

	// Re-register to inspect state: no pending changes should have been acked.
	let uri = documents.register(path, Some("rust")).unwrap();
	assert_eq!(documents.pending_change_count(&uri), 0);
	assert!(!documents.take_force_full_sync_by_uri(&uri));
}

#[tokio::test]
async fn barrier_ignored_after_doc_reopen() {
	let documents = Arc::new(DocumentStateManager::new());
	let path = Path::new("/barrier_reopen.rs");
	let uri = documents.register(path, Some("rust")).unwrap();
	documents.mark_opened(&uri, 0);

	// Queue a change and create a barrier.
	let version = documents.queue_change(&uri).unwrap();
	let (barrier_tx, barrier_rx) = oneshot::channel();

	let transport = Arc::new(SimpleStubTransport);
	let registry = Arc::new(Registry::new(transport));
	let sync = DocumentSync::with_registry(registry, documents.clone());

	let completion_rx = sync.wrap_barrier(uri.clone(), version, barrier_rx);

	// Close and reopen the document — new session, new generation.
	documents.unregister(&uri);
	let uri = documents.register(path, Some("rust")).unwrap();
	documents.mark_opened(&uri, 0);

	// Queue a change in the new session.
	let _new_version = documents.queue_change(&uri).unwrap();

	// Resolve the old barrier — should be silently ignored.
	barrier_tx.send(Ok(())).unwrap();
	completion_rx.await.unwrap();

	// The new session's pending change should still be there (not acked by stale barrier).
	assert_eq!(documents.pending_change_count(&uri), 1, "stale barrier should not ack new session's change");
}

#[tokio::test]
async fn barrier_error_ignored_after_doc_reopen() {
	let documents = Arc::new(DocumentStateManager::new());
	let path = Path::new("/barrier_err_reopen.rs");
	let uri = documents.register(path, Some("rust")).unwrap();
	documents.mark_opened(&uri, 0);

	let version = documents.queue_change(&uri).unwrap();
	let (barrier_tx, barrier_rx) = oneshot::channel();

	let transport = Arc::new(SimpleStubTransport);
	let registry = Arc::new(Registry::new(transport));
	let sync = DocumentSync::with_registry(registry, documents.clone());

	let completion_rx = sync.wrap_barrier(uri.clone(), version, barrier_rx);

	// Close and reopen.
	documents.unregister(&uri);
	let uri = documents.register(path, Some("rust")).unwrap();
	documents.mark_opened(&uri, 0);

	// Resolve with error — should NOT mark force_full_sync on the new session.
	barrier_tx.send(Err(crate::Error::Protocol("test".into()))).unwrap();
	completion_rx.await.unwrap();

	assert!(
		!documents.take_force_full_sync_by_uri(&uri),
		"stale barrier error should not force full sync on new session"
	);
}
