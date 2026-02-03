//! Workspace intelligence and search service.

use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use tokio::sync::{mpsc, oneshot};
use xeno_broker_proto::types::{ErrorCode, KnowledgeHit};

use crate::core::knowledge;

/// Commands for the knowledge service actor.
#[derive(Debug)]
pub enum KnowledgeCmd {
	/// Search the BM25 index.
	Search {
		/// Search query string.
		query: String,
		/// Maximum number of hits.
		limit: u32,
		/// Reply channel for ranked hits.
		reply: oneshot::Sender<Result<Vec<KnowledgeHit>, ErrorCode>>,
	},
	/// Mark a document as dirty for background re-indexing.
	DocDirty {
		/// Canonical document URI.
		uri: String,
	},
	/// Start a background filesystem crawl for a project root.
	SpawnProjectCrawl {
		/// Project filesystem path.
		root: PathBuf,
	},
}

/// Handle for communicating with the `KnowledgeService`.
#[derive(Clone, Debug)]
pub struct KnowledgeHandle {
	tx: mpsc::Sender<KnowledgeCmd>,
}

impl KnowledgeHandle {
	/// Wraps a command sender in a typed handle.
	pub fn new(tx: mpsc::Sender<KnowledgeCmd>) -> Self {
		Self { tx }
	}

	/// Executes a BM25 ranked search against the workspace index.
	pub async fn search(&self, query: &str, limit: u32) -> Result<Vec<KnowledgeHit>, ErrorCode> {
		let (reply, rx) = oneshot::channel();
		self.tx
			.send(KnowledgeCmd::Search {
				query: query.to_string(),
				limit,
				reply,
			})
			.await
			.map_err(|_| ErrorCode::Internal)?;
		rx.await.map_err(|_| ErrorCode::Internal)?
	}

	/// Signals that a document has changed and needs indexing.
	///
	/// This is non-blocking and spawns a background task to ensure high-frequency
	/// editor deltas are not blocked by the knowledge service's channel capacity.
	pub fn doc_dirty(&self, uri: String) -> Result<(), ErrorCode> {
		let tx = self.tx.clone();
		tokio::spawn(async move {
			let _ = tx.send(KnowledgeCmd::DocDirty { uri }).await;
		});
		Ok(())
	}

	/// Triggers a project-wide crawl for unopened files.
	pub fn spawn_project_crawl(&self, root: PathBuf) {
		let tx = self.tx.clone();
		tokio::spawn(async move {
			let _ = tx.send(KnowledgeCmd::SpawnProjectCrawl { root }).await;
		});
	}
}

/// Actor service managing the workspace search index.
///
/// Wraps `KnowledgeCore` and manages background workers for crawling the
/// filesystem and re-indexing open documents. Depends on `SharedStateHandle`
/// for pulling consistent snapshots of live editor state.
pub struct KnowledgeService {
	rx: mpsc::Receiver<KnowledgeCmd>,
	core: Option<Arc<knowledge::KnowledgeCore>>,
	open_docs: Arc<Mutex<HashSet<String>>>,
}

impl KnowledgeService {
	/// Spawns the knowledge service actor.
	pub fn start(
		shared_handle: super::shared_state::SharedStateHandle,
		open_docs: Arc<Mutex<HashSet<String>>>,
	) -> KnowledgeHandle {
		let (tx, rx) = mpsc::channel(256);

		let core = match knowledge::default_db_path()
			.and_then(|path| knowledge::KnowledgeCore::open(path).map(Arc::new))
		{
			Ok(core) => {
				let source: Arc<dyn knowledge::DocSnapshotSource> = Arc::new(AsyncSnapshotSource {
					handle: shared_handle.clone(),
				});
				core.start_worker(Arc::downgrade(&source));
				Some(core)
			}
			Err(err) => {
				tracing::warn!(error = %err, "KnowledgeCore disabled");
				None
			}
		};

		let service = Self {
			rx,
			core,
			open_docs,
		};
		tokio::spawn(service.run());

		KnowledgeHandle::new(tx)
	}

	async fn run(mut self) {
		while let Some(cmd) = self.rx.recv().await {
			match cmd {
				KnowledgeCmd::Search {
					query,
					limit,
					reply,
				} => {
					let res = self
						.core
						.as_ref()
						.ok_or(ErrorCode::NotImplemented)
						.and_then(|c| c.search(&query, limit).map_err(|_| ErrorCode::Internal));
					let _ = reply.send(res);
				}
				KnowledgeCmd::DocDirty { uri } => {
					if let Some(core) = &self.core {
						core.mark_dirty(uri);
					}
				}
				KnowledgeCmd::SpawnProjectCrawl { root } => {
					if let Some(core) = &self.core {
						knowledge::crawler::ProjectCrawler::spawn(
							core.clone(),
							self.open_docs.clone(),
							root,
						);
					}
				}
			}
		}
	}
}

struct AsyncSnapshotSource {
	handle: super::shared_state::SharedStateHandle,
}

impl knowledge::DocSnapshotSource for AsyncSnapshotSource {
	fn snapshot_sync_doc(
		&self,
		uri: &str,
	) -> std::pin::Pin<
		Box<
			dyn std::future::Future<
					Output = Option<(
						xeno_broker_proto::types::SyncEpoch,
						xeno_broker_proto::types::SyncSeq,
						ropey::Rope,
					)>,
				> + Send,
		>,
	> {
		let handle = self.handle.clone();
		let uri = uri.to_string();
		Box::pin(async move { handle.snapshot(uri).await })
	}

	fn is_sync_doc_open(
		&self,
		uri: &str,
	) -> std::pin::Pin<Box<dyn std::future::Future<Output = bool> + Send>> {
		let handle = self.handle.clone();
		let uri = uri.to_string();
		Box::pin(async move { handle.is_open(uri).await })
	}
}
