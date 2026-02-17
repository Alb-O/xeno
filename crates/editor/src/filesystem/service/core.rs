use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering as AtomicOrdering};
use std::time::Duration;

use parking_lot::RwLock;
use tokio::sync::mpsc;

use crate::filesystem::types::{IndexDelta, IndexMsg, ProgressSnapshot, PumpBudget, SearchData, SearchMsg, SearchRow};
use crate::filesystem::{FilesystemOptions, apply_search_delta, run_filesystem_index, run_search_query};

#[derive(Clone, Debug, PartialEq, Eq)]
struct IndexSpec {
	root: PathBuf,
	options: FilesystemOptions,
}

#[derive(Clone)]
struct FsSharedState {
	generation: u64,
	index_spec: Option<IndexSpec>,
	search_active: bool,
	data: SearchData,
	progress: ProgressSnapshot,
	result_query: String,
	result_id: Option<u64>,
	results: Arc<[SearchRow]>,
}

impl Default for FsSharedState {
	fn default() -> Self {
		Self {
			generation: 0,
			index_spec: None,
			search_active: false,
			data: SearchData::default(),
			progress: ProgressSnapshot::default(),
			result_query: String::new(),
			result_id: None,
			results: Arc::from(Vec::<SearchRow>::new()),
		}
	}
}

/// Command protocol for the filesystem service actor.
#[allow(dead_code)]
#[derive(Debug)]
pub enum FsServiceCmd {
	EnsureIndex { root: PathBuf, options: FilesystemOptions },
	Query { query: String, limit: usize },
	StopIndex,
	Indexer(FsIndexerEvt),
	Search(FsSearchEvt),
}

/// Event protocol emitted by the filesystem service actor.
#[allow(dead_code)]
#[derive(Debug, Clone)]
pub enum FsServiceEvt {
	SnapshotChanged { generation: u64 },
}

/// Command protocol for the indexer worker actor.
#[derive(Debug)]
pub enum FsIndexerCmd {
	Start {
		generation: u64,
		root: PathBuf,
		options: FilesystemOptions,
	},
	Stop,
}

/// Event protocol emitted by the indexer worker actor.
#[derive(Debug)]
pub enum FsIndexerEvt {
	Message(IndexMsg),
}

/// Command protocol for the search worker actor.
#[derive(Debug)]
pub enum FsSearchCmd {
	Start { generation: u64, data: SearchData },
	UpdateDelta { generation: u64, delta: IndexDelta },
	RunQuery { generation: u64, id: u64, query: String, limit: usize },
	Stop,
}

/// Event protocol emitted by the search worker actor.
#[derive(Debug)]
pub enum FsSearchEvt {
	Message(SearchMsg),
}

struct FsIndexerActor {
	worker_runtime: xeno_worker::WorkerRuntime,
	event_tx: mpsc::UnboundedSender<FsServiceCmd>,
}

#[async_trait::async_trait]
impl xeno_worker::WorkerActor for FsIndexerActor {
	type Cmd = FsIndexerCmd;
	type Evt = ();

	async fn handle(&mut self, cmd: Self::Cmd, _ctx: &mut xeno_worker::ActorContext<Self::Evt>) -> Result<xeno_worker::ActorFlow, String> {
		match cmd {
			FsIndexerCmd::Start { generation, root, options } => {
				let event_tx = self.event_tx.clone();
				let runtime = self.worker_runtime.clone();
				self.worker_runtime.spawn_thread(xeno_worker::TaskClass::IoBlocking, move || {
					run_filesystem_index(
						runtime,
						generation,
						root,
						options,
						Arc::new(move |msg| event_tx.send(FsServiceCmd::Indexer(FsIndexerEvt::Message(msg))).is_ok()),
					);
				});
			}
			FsIndexerCmd::Stop => {}
		}
		Ok(xeno_worker::ActorFlow::Continue)
	}
}

struct FsSearchActor {
	worker_runtime: xeno_worker::WorkerRuntime,
	event_tx: mpsc::UnboundedSender<FsServiceCmd>,
	data: SearchData,
	generation: Option<u64>,
	latest_query_id: Option<Arc<AtomicU64>>,
}

impl FsSearchActor {
	fn stop_generation(&mut self) {
		self.generation = None;
		self.data = SearchData::default();
		self.latest_query_id = None;
	}
}

#[async_trait::async_trait]
impl xeno_worker::WorkerActor for FsSearchActor {
	type Cmd = FsSearchCmd;
	type Evt = ();

	async fn on_stop(&mut self, _ctx: &mut xeno_worker::ActorContext<Self::Evt>) {
		self.stop_generation();
	}

	async fn handle(&mut self, cmd: Self::Cmd, _ctx: &mut xeno_worker::ActorContext<Self::Evt>) -> Result<xeno_worker::ActorFlow, String> {
		match cmd {
			FsSearchCmd::Start { generation, data } => {
				self.stop_generation();
				self.generation = Some(generation);
				self.data = data;
				self.latest_query_id = Some(Arc::new(AtomicU64::new(0)));
			}
			FsSearchCmd::UpdateDelta { generation, delta } => {
				if self.generation == Some(generation) {
					apply_search_delta(&mut self.data, delta);
				}
			}
			FsSearchCmd::RunQuery { generation, id, query, limit } => {
				if self.generation == Some(generation)
					&& let Some(latest_query_id) = self.latest_query_id.as_ref()
				{
					latest_query_id.store(id, AtomicOrdering::Release);
					let latest_query_id = Arc::clone(latest_query_id);
					let data = self.data.clone();
					let event_tx = self.event_tx.clone();
					let runtime = self.worker_runtime.clone();
					self.worker_runtime.spawn(xeno_worker::TaskClass::Background, async move {
						let result = runtime
							.spawn_blocking(xeno_worker::TaskClass::CpuBlocking, move || {
								run_search_query(generation, id, &query, limit, &data, latest_query_id.as_ref())
							})
							.await
							.ok()
							.flatten();
						if let Some(msg) = result {
							let _ = event_tx.send(FsServiceCmd::Search(FsSearchEvt::Message(msg)));
						}
					});
				}
			}
			FsSearchCmd::Stop => self.stop_generation(),
		}
		Ok(xeno_worker::ActorFlow::Continue)
	}
}

struct FsServiceActor {
	generation: u64,
	index_spec: Option<IndexSpec>,
	next_query_id: u64,
	data: SearchData,
	progress: ProgressSnapshot,
	result_query: String,
	result_id: Option<u64>,
	results: Arc<[SearchRow]>,
	search_active: bool,
	indexer: Arc<xeno_worker::ActorHandle<FsIndexerCmd, ()>>,
	search: Arc<xeno_worker::ActorHandle<FsSearchCmd, ()>>,
	shared: Arc<RwLock<FsSharedState>>,
	changed: Arc<AtomicBool>,
}

impl FsServiceActor {
	fn sync_shared(&self) {
		let mut shared = self.shared.write();
		shared.generation = self.generation;
		shared.index_spec = self.index_spec.clone();
		shared.search_active = self.search_active;
		shared.data = self.data.clone();
		shared.progress = self.progress;
		shared.result_query = self.result_query.clone();
		shared.result_id = self.result_id;
		shared.results = Arc::clone(&self.results);
	}

	fn begin_new_generation(&mut self) {
		self.generation = self.generation.saturating_add(1);
		self.next_query_id = 0;
		self.data.files.clear();
		self.progress = ProgressSnapshot::default();
		self.result_query.clear();
		self.result_id = None;
		self.results = Arc::from(Vec::<SearchRow>::new());
	}

	async fn stop_workers(&mut self) {
		let _ = self.indexer.send(FsIndexerCmd::Stop).await;
		let _ = self.search.send(FsSearchCmd::Stop).await;
		self.index_spec = None;
		self.search_active = false;
	}

	async fn apply_index_msg(&mut self, msg: IndexMsg) -> bool {
		match msg {
			IndexMsg::Update(update) => {
				if update.generation != self.generation {
					return false;
				}

				if let Some(cached_data) = update.cached_data.clone() {
					let _ = self
						.search
						.send(FsSearchCmd::UpdateDelta {
							generation: self.generation,
							delta: IndexDelta::Replace(cached_data),
						})
						.await;
				} else {
					if update.reset {
						let _ = self
							.search
							.send(FsSearchCmd::UpdateDelta {
								generation: self.generation,
								delta: IndexDelta::Reset,
							})
							.await;
					}
					if !update.files.is_empty() {
						let _ = self
							.search
							.send(FsSearchCmd::UpdateDelta {
								generation: self.generation,
								delta: IndexDelta::Append(Arc::clone(&update.files)),
							})
							.await;
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

#[async_trait::async_trait]
impl xeno_worker::WorkerActor for FsServiceActor {
	type Cmd = FsServiceCmd;
	type Evt = FsServiceEvt;

	async fn handle(&mut self, cmd: Self::Cmd, ctx: &mut xeno_worker::ActorContext<Self::Evt>) -> Result<xeno_worker::ActorFlow, String> {
		let mut changed = false;
		match cmd {
			FsServiceCmd::EnsureIndex { root, options } => {
				let requested = IndexSpec {
					root: root.clone(),
					options: options.clone(),
				};

				if self.index_spec.as_ref().is_some_and(|active| active == &requested) && self.search_active {
					return Ok(xeno_worker::ActorFlow::Continue);
				}

				self.stop_workers().await;
				self.begin_new_generation();
				self.data.root = Some(root.clone());

				let _ = self
					.indexer
					.send(FsIndexerCmd::Start {
						generation: self.generation,
						root,
						options,
					})
					.await;
				let _ = self
					.search
					.send(FsSearchCmd::Start {
						generation: self.generation,
						data: SearchData {
							root: self.data.root.clone(),
							files: Vec::new(),
						},
					})
					.await;

				self.search_active = true;
				self.index_spec = Some(requested);
				changed = true;
			}
			FsServiceCmd::Query { query, limit } => {
				if !self.search_active {
					return Ok(xeno_worker::ActorFlow::Continue);
				}

				self.next_query_id = self.next_query_id.wrapping_add(1);
				let id = self.next_query_id;
				let _ = self
					.search
					.send(FsSearchCmd::RunQuery {
						generation: self.generation,
						id,
						query,
						limit,
					})
					.await;
			}
			FsServiceCmd::StopIndex => {
				self.stop_workers().await;
				self.generation = self.generation.saturating_add(1);
				self.next_query_id = 0;
				changed = true;
			}
			FsServiceCmd::Indexer(FsIndexerEvt::Message(msg)) => {
				changed = self.apply_index_msg(msg).await;
			}
			FsServiceCmd::Search(FsSearchEvt::Message(msg)) => {
				changed = self.apply_search_msg(msg);
			}
		}

		if changed {
			self.sync_shared();
			self.changed.store(true, AtomicOrdering::Release);
			ctx.emit(FsServiceEvt::SnapshotChanged { generation: self.generation });
		}

		Ok(xeno_worker::ActorFlow::Continue)
	}
}

pub struct FsService {
	state: Arc<RwLock<FsSharedState>>,
	query_state: Arc<RwLock<(u64, u64)>>,
	changed: Arc<AtomicBool>,
	command_tx: mpsc::UnboundedSender<FsServiceCmd>,
	_service_actor: Arc<xeno_worker::ActorHandle<FsServiceCmd, FsServiceEvt>>,
	_dispatch_task: tokio::task::JoinHandle<()>,
}

impl FsService {
	#[cfg(test)]
	pub fn new() -> Self {
		Self::default()
	}

	pub fn new_with_runtime(worker_runtime: xeno_worker::WorkerRuntime) -> Self {
		let state = Arc::new(RwLock::new(FsSharedState::default()));
		let query_state = Arc::new(RwLock::new((0, 0)));
		let changed = Arc::new(AtomicBool::new(false));
		let (command_tx, mut command_rx) = mpsc::unbounded_channel::<FsServiceCmd>();
		let (event_tx, mut event_rx) = mpsc::unbounded_channel::<FsServiceCmd>();

		let indexer = Arc::new(
			worker_runtime.actor(
				xeno_worker::ActorSpec::new("fs.indexer", xeno_worker::TaskClass::IoBlocking, {
					let worker_runtime = worker_runtime.clone();
					let service_tx = event_tx.clone();
					move || FsIndexerActor {
						worker_runtime: worker_runtime.clone(),
						event_tx: service_tx.clone(),
					}
				})
				.supervisor(xeno_worker::SupervisorSpec {
					restart: xeno_worker::RestartPolicy::OnFailure {
						max_restarts: 3,
						backoff: Duration::from_millis(50),
					},
					event_buffer: 16,
				}),
			),
		);

		let search = Arc::new(
			worker_runtime.actor(
				xeno_worker::ActorSpec::new("fs.search", xeno_worker::TaskClass::CpuBlocking, {
					let worker_runtime = worker_runtime.clone();
					let service_tx = event_tx.clone();
					move || FsSearchActor {
						worker_runtime: worker_runtime.clone(),
						event_tx: service_tx.clone(),
						data: SearchData::default(),
						generation: None,
						latest_query_id: None,
					}
				})
				.mailbox(xeno_worker::MailboxSpec {
					capacity: 128,
					policy: xeno_worker::MailboxPolicy::CoalesceByKey,
				})
				.coalesce_by_key(|cmd: &FsSearchCmd| match cmd {
					FsSearchCmd::RunQuery { generation, id, .. } => format!("q:{generation}:{id}"),
					FsSearchCmd::UpdateDelta { generation, .. } => format!("u:{generation}"),
					FsSearchCmd::Start { generation, .. } => format!("s:{generation}"),
					FsSearchCmd::Stop => "stop".to_string(),
				}),
			),
		);

		let service_actor = Arc::new(
			worker_runtime.actor(xeno_worker::ActorSpec::new("fs.service", xeno_worker::TaskClass::Interactive, {
				let state = Arc::clone(&state);
				let changed = Arc::clone(&changed);
				let indexer = Arc::clone(&indexer);
				let search = Arc::clone(&search);
				move || FsServiceActor {
					generation: 0,
					index_spec: None,
					next_query_id: 0,
					data: SearchData::default(),
					progress: ProgressSnapshot::default(),
					result_query: String::new(),
					result_id: None,
					results: Arc::from(Vec::<SearchRow>::new()),
					search_active: false,
					indexer: Arc::clone(&indexer),
					search: Arc::clone(&search),
					shared: Arc::clone(&state),
					changed: Arc::clone(&changed),
				}
			})),
		);

		let service_actor_for_dispatch = Arc::clone(&service_actor);
		let dispatch_task = worker_runtime.spawn(xeno_worker::TaskClass::Interactive, async move {
			let mut command_open = true;
			let mut event_open = true;
			while command_open || event_open {
				tokio::select! {
					biased;
					maybe_cmd = command_rx.recv(), if command_open => {
						match maybe_cmd {
							Some(cmd) => {
								if service_actor_for_dispatch.send(cmd).await.is_err() {
									break;
								}
							}
							None => command_open = false,
						}
					}
					maybe_evt = event_rx.recv(), if event_open => {
						match maybe_evt {
							Some(evt) => {
								if service_actor_for_dispatch.send(evt).await.is_err() {
									break;
								}
							}
							None => event_open = false,
						}
					}
				}
			}
		});

		Self {
			state,
			query_state,
			changed,
			command_tx,
			_service_actor: service_actor,
			_dispatch_task: dispatch_task,
		}
	}
}

impl Default for FsService {
	fn default() -> Self {
		Self::new_with_runtime(xeno_worker::WorkerRuntime::new())
	}
}

impl FsService {
	pub fn ensure_index(&mut self, root: PathBuf, options: FilesystemOptions) -> bool {
		let requested = IndexSpec { root, options };
		{
			let shared = self.state.read();
			if shared.index_spec.as_ref().is_some_and(|active| active == &requested) && shared.search_active {
				return false;
			}
		}
		self.command_tx
			.send(FsServiceCmd::EnsureIndex {
				root: requested.root,
				options: requested.options,
			})
			.is_ok()
	}

	#[allow(dead_code)]
	pub fn stop_index(&mut self) {
		let _ = self.command_tx.send(FsServiceCmd::StopIndex);
	}

	#[cfg(test)]
	pub fn generation(&self) -> u64 {
		self.state.read().generation
	}

	#[cfg(test)]
	#[allow(dead_code)]
	pub fn result_id(&self) -> Option<u64> {
		self.state.read().result_id
	}

	#[cfg(test)]
	pub fn inject_index_msg(&self, msg: IndexMsg) {
		let _ = self.command_tx.send(FsServiceCmd::Indexer(FsIndexerEvt::Message(msg)));
	}

	#[cfg(test)]
	pub fn inject_search_msg(&self, msg: SearchMsg) {
		let _ = self.command_tx.send(FsServiceCmd::Search(FsSearchEvt::Message(msg)));
	}

	pub fn progress(&self) -> ProgressSnapshot {
		self.state.read().progress
	}

	pub fn data(&self) -> SearchData {
		self.state.read().data.clone()
	}

	pub fn result_query(&self) -> String {
		self.state.read().result_query.clone()
	}

	pub fn results(&self) -> Arc<[SearchRow]> {
		Arc::clone(&self.state.read().results)
	}

	pub fn query(&mut self, query: impl Into<String>, limit: usize) -> Option<u64> {
		let generation = {
			let shared = self.state.read();
			if !shared.search_active {
				return None;
			}
			shared.generation
		};

		let id = {
			let mut query_state = self.query_state.write();
			if query_state.0 != generation {
				*query_state = (generation, 0);
			}
			query_state.1 = query_state.1.wrapping_add(1);
			query_state.1
		};

		let _ = self.command_tx.send(FsServiceCmd::Query { query: query.into(), limit });

		Some(id)
	}

	/// Compatibility shim for runtime pump: state is now pushed by actors.
	pub fn pump(&mut self, _budget: PumpBudget) -> bool {
		self.changed.swap(false, AtomicOrdering::AcqRel)
	}
}
