use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering as AtomicOrdering};
use std::time::Duration;

use parking_lot::RwLock;
use tokio::sync::broadcast;

use crate::filesystem::types::{IndexDelta, IndexMsg, ProgressSnapshot, SearchData, SearchMsg, SearchRow};
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
#[derive(Debug)]
pub(crate) enum FsServiceCmd {
	EnsureIndex {
		root: PathBuf,
		options: FilesystemOptions,
	},
	Query {
		query: String,
		limit: usize,
	},
	Indexer(FsIndexerEvt),
	Search(FsSearchEvt),
	#[cfg(test)]
	CrashForTest,
}

/// Event protocol emitted by the filesystem service actor.
#[derive(Debug, Clone)]
pub(crate) enum FsServiceEvt {
	SnapshotChanged,
}

#[derive(Debug, Clone)]
pub struct FsServiceShutdownReport {
	pub service: xeno_worker::ActorShutdownReport,
	pub indexer: xeno_worker::ActorShutdownReport,
	pub search: xeno_worker::ActorShutdownReport,
}

/// Command protocol for the indexer worker actor.
#[derive(Debug)]
pub(crate) enum FsIndexerCmd {
	Start {
		generation: u64,
		root: PathBuf,
		options: FilesystemOptions,
	},
	Stop,
}

/// Event protocol emitted by the indexer worker actor.
#[derive(Debug)]
pub(crate) enum FsIndexerEvt {
	Message(IndexMsg),
}

/// Command protocol for the search worker actor.
#[derive(Debug)]
pub(crate) enum FsSearchCmd {
	Start { generation: u64, data: SearchData },
	UpdateDelta { generation: u64, delta: IndexDelta },
	RunQuery { generation: u64, id: u64, query: String, limit: usize },
	Stop,
}

/// Event protocol emitted by the search worker actor.
#[derive(Debug)]
pub(crate) enum FsSearchEvt {
	Message(SearchMsg),
}

struct FsIndexerActor {
	command_port: Arc<std::sync::OnceLock<xeno_worker::ActorCommandPort<FsServiceCmd>>>,
}

#[async_trait::async_trait]
impl xeno_worker::Actor for FsIndexerActor {
	type Cmd = FsIndexerCmd;
	type Evt = ();

	async fn handle(&mut self, cmd: Self::Cmd, _ctx: &mut xeno_worker::ActorContext<Self::Evt>) -> Result<xeno_worker::ActorFlow, String> {
		match cmd {
			FsIndexerCmd::Start { generation, root, options } => {
				let command_port = Arc::clone(&self.command_port);
				xeno_worker::spawn_thread(xeno_worker::TaskClass::IoBlocking, move || {
					run_filesystem_index(
						generation,
						root,
						options,
						Arc::new(move |msg| {
							command_port
								.get()
								.is_some_and(|port| port.send(FsServiceCmd::Indexer(FsIndexerEvt::Message(msg))).is_ok())
						}),
					);
				});
			}
			FsIndexerCmd::Stop => {}
		}
		Ok(xeno_worker::ActorFlow::Continue)
	}
}

struct FsSearchActor {
	command_port: Arc<std::sync::OnceLock<xeno_worker::ActorCommandPort<FsServiceCmd>>>,
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
impl xeno_worker::Actor for FsSearchActor {
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
					let command_port = Arc::clone(&self.command_port);
					xeno_worker::spawn(xeno_worker::TaskClass::Background, async move {
						let result = xeno_worker::spawn_blocking(xeno_worker::TaskClass::CpuBlocking, move || {
								run_search_query(generation, id, &query, limit, &data, latest_query_id.as_ref())
							})
							.await
							.ok()
							.flatten();
						if let Some(msg) = result
							&& let Some(port) = command_port.get()
						{
							let _ = port.send(FsServiceCmd::Search(FsSearchEvt::Message(msg)));
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
impl xeno_worker::Actor for FsServiceActor {
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
			FsServiceCmd::Indexer(FsIndexerEvt::Message(msg)) => {
				changed = self.apply_index_msg(msg).await;
			}
			FsServiceCmd::Search(FsSearchEvt::Message(msg)) => {
				changed = self.apply_search_msg(msg);
			}
			#[cfg(test)]
			FsServiceCmd::CrashForTest => return Err("fs.service crash test hook".to_string()),
		}

		if changed {
			self.sync_shared();
			ctx.emit(FsServiceEvt::SnapshotChanged);
		}

		Ok(xeno_worker::ActorFlow::Continue)
	}
}

pub struct FsService {
	state: Arc<RwLock<FsSharedState>>,
	command_port: xeno_worker::ActorCommandPort<FsServiceCmd>,
	service_ingress: xeno_worker::ActorCommandIngress<FsServiceCmd, FsServiceEvt>,
	indexer_actor: Arc<xeno_worker::ActorHandle<FsIndexerCmd, ()>>,
	search_actor: Arc<xeno_worker::ActorHandle<FsSearchCmd, ()>>,
	event_rx: broadcast::Receiver<FsServiceEvt>,
}

impl FsService {
	#[cfg(test)]
	pub fn new() -> Self {
		Self::default()
	}

	pub fn new_with_runtime(worker_runtime: xeno_worker::WorkerRuntime) -> Self {
		let state = Arc::new(RwLock::new(FsSharedState::default()));
		let service_command_port = Arc::new(std::sync::OnceLock::<xeno_worker::ActorCommandPort<FsServiceCmd>>::new());

		let indexer = Arc::new(
			worker_runtime.spawn_actor(
				xeno_worker::ActorSpec::new("fs.indexer", xeno_worker::TaskClass::IoBlocking, {
					let service_command_port = Arc::clone(&service_command_port);
					move || FsIndexerActor {
						command_port: Arc::clone(&service_command_port),
					}
				})
				.supervisor(xeno_worker::ActorSupervisorSpec::default()
					.restart(xeno_worker::ActorRestartPolicy::OnFailure {
						max_restarts: 3,
						backoff: Duration::from_millis(50),
					})
					.event_buffer(16)),
			),
		);

		let search = Arc::new(
			worker_runtime.spawn_actor(
				xeno_worker::ActorSpec::new("fs.search", xeno_worker::TaskClass::CpuBlocking, {
					let service_command_port = Arc::clone(&service_command_port);
					move || FsSearchActor {
						command_port: Arc::clone(&service_command_port),
						data: SearchData::default(),
						generation: None,
						latest_query_id: None,
					}
				})
				.mailbox(xeno_worker::ActorMailboxSpec::with_capacity(128))
				.coalesce_by_key(|cmd: &FsSearchCmd| match cmd {
					FsSearchCmd::RunQuery { generation, id, .. } => format!("q:{generation}:{id}"),
					FsSearchCmd::UpdateDelta { generation, .. } => format!("u:{generation}"),
					FsSearchCmd::Start { generation, .. } => format!("s:{generation}"),
					FsSearchCmd::Stop => "stop".to_string(),
				}),
			),
		);

		let service_actor = Arc::new(
			worker_runtime.spawn_actor(xeno_worker::ActorSpec::new("fs.service", xeno_worker::TaskClass::Interactive, {
				let state = Arc::clone(&state);
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
				}
			})),
		);
		let service_ingress = xeno_worker::ActorCommandIngress::new(xeno_worker::TaskClass::Interactive, Arc::clone(&service_actor));
		let command_port = service_ingress.port();
		let _ = service_command_port.set(command_port.clone());
		let service_event_rx = service_ingress.subscribe();

		Self {
			state,
			command_port,
			service_ingress,
			indexer_actor: indexer,
			search_actor: search,
			event_rx: service_event_rx,
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
		self.command_port
			.send(FsServiceCmd::EnsureIndex {
				root: requested.root,
				options: requested.options,
			})
			.is_ok()
	}

	#[cfg(test)]
	pub fn generation(&self) -> u64 {
		self.state.read().generation
	}

	#[cfg(test)]
	pub fn service_restart_count(&self) -> usize {
		self.service_ingress.actor().restart_count()
	}

	#[cfg(test)]
	pub fn inject_index_msg(&self, msg: IndexMsg) {
		let _ = self.command_port.send(FsServiceCmd::Indexer(FsIndexerEvt::Message(msg)));
	}

	#[cfg(test)]
	pub fn inject_search_msg(&self, msg: SearchMsg) {
		let _ = self.command_port.send(FsServiceCmd::Search(FsSearchEvt::Message(msg)));
	}

	#[cfg(test)]
	pub fn crash_for_test(&self) {
		let _ = self.command_port.send(FsServiceCmd::CrashForTest);
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

	pub fn query(&mut self, query: impl Into<String>, limit: usize) -> bool {
		{
			let shared = self.state.read();
			if !shared.search_active {
				return false;
			}
		}

		self.command_port.send(FsServiceCmd::Query { query: query.into(), limit }).is_ok()
	}

	/// Drains pushed snapshot-change events and returns the number consumed.
	pub fn drain_events(&mut self) -> usize {
		let mut drained = 0usize;
		loop {
			match self.event_rx.try_recv() {
				Ok(FsServiceEvt::SnapshotChanged) => drained = drained.saturating_add(1),
				Err(broadcast::error::TryRecvError::Lagged(_)) => drained = drained.saturating_add(1),
				Err(broadcast::error::TryRecvError::Empty) | Err(broadcast::error::TryRecvError::Closed) => break,
			}
		}
		drained
	}

	pub async fn shutdown(&self, mode: xeno_worker::ActorShutdownMode) -> FsServiceShutdownReport {
		let service = self.service_ingress.shutdown(mode).await;
		let indexer = self.indexer_actor.shutdown(mode).await;
		let search = self.search_actor.shutdown(mode).await;

		FsServiceShutdownReport { service, indexer, search }
	}
}
