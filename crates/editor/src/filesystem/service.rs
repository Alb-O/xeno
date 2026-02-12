use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering as AtomicOrdering};
use std::sync::mpsc::{Receiver, Sender, TryRecvError};
use std::time::Instant;

use super::types::{IndexDelta, IndexMsg, ProgressSnapshot, PumpBudget, SearchCmd, SearchData, SearchMsg, SearchRow};

#[derive(Clone, Debug, PartialEq, Eq)]
struct IndexSpec {
	root: PathBuf,
	options: super::FilesystemOptions,
}

pub struct FsService {
	generation: u64,
	index_rx: Option<Receiver<IndexMsg>>,
	search_tx: Option<Sender<SearchCmd>>,
	search_rx: Option<Receiver<SearchMsg>>,
	search_latest_query_id: Option<Arc<AtomicU64>>,
	next_query_id: u64,
	index_spec: Option<IndexSpec>,
	data: SearchData,
	progress: ProgressSnapshot,
	result_query: String,
	result_id: Option<u64>,
	results: Arc<[SearchRow]>,
}

impl Default for FsService {
	fn default() -> Self {
		Self {
			generation: 0,
			index_rx: None,
			search_tx: None,
			search_rx: None,
			search_latest_query_id: None,
			next_query_id: 0,
			index_spec: None,
			data: SearchData::default(),
			progress: ProgressSnapshot::default(),
			result_query: String::new(),
			result_id: None,
			results: Arc::from(Vec::<SearchRow>::new()),
		}
	}
}

impl FsService {
	pub fn new() -> Self {
		Self::default()
	}

	pub fn ensure_index(&mut self, root: PathBuf, options: super::FilesystemOptions) -> bool {
		let requested = IndexSpec {
			root: root.clone(),
			options: options.clone(),
		};

		if self.index_spec.as_ref().is_some_and(|active| active == &requested) && self.index_rx.is_some() {
			return false;
		}

		self.stop_index();
		let generation = self.begin_new_generation();
		self.data.root = Some(root.clone());
		let rx = super::spawn_filesystem_index(generation, root.clone(), options);
		self.set_index_receiver(rx);

		let (search_tx, search_rx, latest_query_id) = super::spawn_search_worker(
			generation,
			SearchData {
				root: Some(root),
				files: Vec::new(),
			},
		);
		self.search_tx = Some(search_tx);
		self.search_latest_query_id = Some(latest_query_id);
		self.set_search_receiver(search_rx);

		self.index_spec = Some(requested);
		true
	}

	pub fn stop_index(&mut self) {
		if let Some(search_tx) = self.search_tx.take() {
			let _ = search_tx.send(SearchCmd::Shutdown { generation: self.generation });
		}
		self.index_rx = None;
		self.search_rx = None;
		self.search_latest_query_id = None;
		self.index_spec = None;
	}

	pub fn generation(&self) -> u64 {
		self.generation
	}

	pub fn begin_new_generation(&mut self) -> u64 {
		self.generation = self.generation.saturating_add(1);
		self.progress = ProgressSnapshot::default();
		self.search_tx = None;
		self.search_rx = None;
		self.search_latest_query_id = None;
		self.next_query_id = 0;
		self.data.files.clear();
		self.result_query.clear();
		self.result_id = None;
		self.results = Arc::from(Vec::<SearchRow>::new());
		self.generation
	}

	pub fn set_index_receiver(&mut self, rx: Receiver<IndexMsg>) {
		self.index_rx = Some(rx);
	}

	pub fn set_search_receiver(&mut self, rx: Receiver<SearchMsg>) {
		self.search_rx = Some(rx);
	}

	pub fn progress(&self) -> ProgressSnapshot {
		self.progress
	}

	pub fn data(&self) -> &SearchData {
		&self.data
	}

	pub fn result_query(&self) -> &str {
		&self.result_query
	}

	pub fn results(&self) -> Arc<[SearchRow]> {
		Arc::clone(&self.results)
	}

	pub fn query(&mut self, query: impl Into<String>, limit: usize) -> Option<u64> {
		let tx = self.search_tx.as_ref()?;
		self.next_query_id = self.next_query_id.wrapping_add(1);
		let id = self.next_query_id;

		if let Some(latest_query_id) = &self.search_latest_query_id {
			latest_query_id.store(id, AtomicOrdering::Release);
		}

		let _ = tx.send(SearchCmd::Query {
			generation: self.generation,
			id,
			query: query.into(),
			limit,
		});

		Some(id)
	}

	pub fn pump(&mut self, budget: PumpBudget) -> bool {
		let start = Instant::now();
		let mut changed = false;

		let mut index_processed = 0usize;
		while index_processed < budget.max_index_msgs && start.elapsed() < budget.max_time {
			let Some(rx) = self.index_rx.as_ref() else {
				break;
			};
			match rx.try_recv() {
				Ok(msg) => {
					changed |= self.apply_index_msg(msg);
					index_processed += 1;
				}
				Err(TryRecvError::Empty) => break,
				Err(TryRecvError::Disconnected) => {
					self.index_rx = None;
					break;
				}
			}
		}

		let mut search_processed = 0usize;
		while search_processed < budget.max_search_msgs && start.elapsed() < budget.max_time {
			let Some(rx) = self.search_rx.as_ref() else {
				break;
			};
			match rx.try_recv() {
				Ok(msg) => {
					changed |= self.apply_search_msg(msg);
					search_processed += 1;
				}
				Err(TryRecvError::Empty) => break,
				Err(TryRecvError::Disconnected) => {
					self.search_rx = None;
					break;
				}
			}
		}

		changed
	}

	fn apply_index_msg(&mut self, msg: IndexMsg) -> bool {
		match msg {
			IndexMsg::Update(update) => {
				if update.generation != self.generation {
					return false;
				}

				if let Some(search_tx) = self.search_tx.as_ref() {
					if let Some(cached_data) = update.cached_data.clone() {
						let _ = search_tx.send(SearchCmd::Update {
							generation: self.generation,
							delta: IndexDelta::Replace(cached_data),
						});
					} else {
						if update.reset {
							let _ = search_tx.send(SearchCmd::Update {
								generation: self.generation,
								delta: IndexDelta::Reset,
							});
						}
						if !update.files.is_empty() {
							let _ = search_tx.send(SearchCmd::Update {
								generation: self.generation,
								delta: IndexDelta::Append(Arc::clone(&update.files)),
							});
						}
					}
				}

				let mut changed = false;

				if let Some(cached_data) = update.cached_data {
					self.data = cached_data;
					changed = true;
				} else {
					if update.reset {
						self.data.files.clear();
						changed = true;
					}
					if !update.files.is_empty() {
						self.data.files.extend(update.files.iter().cloned());
						changed = true;
					}
				}

				if self.progress.indexed_files != update.progress.indexed_files
					|| self.progress.total_files != update.progress.total_files
					|| self.progress.complete != update.progress.complete
				{
					self.progress = update.progress;
					changed = true;
				}

				changed
			}
			IndexMsg::Error { generation, message } => {
				if generation != self.generation {
					return false;
				}
				tracing::warn!(generation, message = %message, "filesystem indexer error");
				false
			}
			IndexMsg::Complete {
				generation,
				indexed_files,
				elapsed_ms,
			} => {
				if generation != self.generation {
					return false;
				}

				tracing::debug!(generation, indexed_files, elapsed_ms, "filesystem indexing complete");
				if self.progress.complete && self.progress.indexed_files == indexed_files {
					return false;
				}

				self.progress.indexed_files = indexed_files;
				self.progress.total_files = Some(indexed_files);
				self.progress.complete = true;
				true
			}
		}
	}

	fn apply_search_msg(&mut self, msg: SearchMsg) -> bool {
		match msg {
			SearchMsg::Result {
				generation,
				id,
				query,
				rows,
				scanned,
				elapsed_ms,
			} => {
				if generation != self.generation {
					return false;
				}

				tracing::trace!(generation, id, scanned, elapsed_ms, "filesystem search result");
				let query_changed = self.result_query != query;
				let id_changed = self.result_id != Some(id);
				let rows_changed = !Arc::ptr_eq(&self.results, &rows);

				self.result_query = query;
				self.result_id = Some(id);
				self.results = rows;

				query_changed || id_changed || rows_changed
			}
		}
	}
}

#[cfg(test)]
mod tests {
	use std::sync::{Arc, mpsc};
	use std::time::Duration;

	use super::FsService;
	use crate::filesystem::{FileRow, IndexKind, IndexMsg, IndexUpdate, ProgressSnapshot, PumpBudget, SearchMsg, SearchRow};

	fn budget() -> PumpBudget {
		PumpBudget {
			max_index_msgs: 32,
			max_search_msgs: 8,
			max_time: Duration::from_millis(10),
		}
	}

	#[test]
	fn pump_applies_current_generation_index_updates() {
		let (tx, rx) = mpsc::channel();
		let mut service = FsService::new();
		service.set_index_receiver(rx);

		let files: Arc<[FileRow]> = vec![FileRow::new(Arc::<str>::from("src/main.rs"))].into();
		tx.send(IndexMsg::Update(IndexUpdate {
			generation: service.generation(),
			kind: IndexKind::Live,
			reset: false,
			files,
			progress: ProgressSnapshot {
				indexed_files: 1,
				total_files: Some(2),
				complete: false,
			},
			cached_data: None,
		}))
		.unwrap();

		assert!(service.pump(budget()));
		assert_eq!(service.data().files.len(), 1);
		assert_eq!(service.progress().indexed_files, 1);
	}

	#[test]
	fn pump_ignores_stale_generation_messages() {
		let (tx, rx) = mpsc::channel();
		let mut service = FsService::new();
		service.set_index_receiver(rx);
		let stale_generation = service.generation();
		service.begin_new_generation();

		tx.send(IndexMsg::Update(IndexUpdate {
			generation: stale_generation,
			kind: IndexKind::Live,
			reset: false,
			files: vec![FileRow::new(Arc::<str>::from("src/lib.rs"))].into(),
			progress: ProgressSnapshot {
				indexed_files: 1,
				total_files: Some(1),
				complete: false,
			},
			cached_data: None,
		}))
		.unwrap();

		assert!(!service.pump(budget()));
		assert!(service.data().files.is_empty());
	}

	#[test]
	fn pump_applies_current_generation_search_results() {
		let (tx, rx) = mpsc::channel();
		let mut service = FsService::new();
		service.set_search_receiver(rx);

		tx.send(SearchMsg::Result {
			generation: service.generation(),
			id: 7,
			query: "main".to_string(),
			rows: vec![SearchRow {
				path: Arc::<str>::from("src/main.rs"),
				score: 42,
				match_indices: Some(vec![0, 1]),
			}]
			.into(),
			scanned: 100,
			elapsed_ms: 3,
		})
		.unwrap();

		assert!(service.pump(budget()));
		assert_eq!(service.result_query(), "main");
		assert_eq!(service.results().len(), 1);
	}
}
