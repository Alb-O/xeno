use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering as AtomicOrdering};
use std::sync::mpsc::{Receiver, Sender, TryRecvError};
use std::time::Instant;

use crate::filesystem::types::{IndexDelta, IndexMsg, ProgressSnapshot, PumpBudget, SearchCmd, SearchData, SearchMsg, SearchRow};
use crate::filesystem::{FilesystemOptions, spawn_filesystem_index, spawn_search_worker};

#[derive(Clone, Debug, PartialEq, Eq)]
struct IndexSpec {
	root: PathBuf,
	options: FilesystemOptions,
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

	pub fn ensure_index(&mut self, root: PathBuf, options: FilesystemOptions) -> bool {
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
		let rx = spawn_filesystem_index(generation, root.clone(), options);
		self.set_index_receiver(rx);

		let (search_tx, search_rx, latest_query_id) = spawn_search_worker(
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

	#[cfg(test)]
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
				let kind = update.kind;

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
				tracing::trace!(
					generation = update.generation,
					kind = ?kind,
					reset = update.reset,
					files = update.files.len(),
					has_cached_data = update.cached_data.is_some(),
					"filesystem index update"
				);

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
