use std::sync::Arc;
use std::time::{Duration, Instant};

use rustc_hash::FxHashMap;
use tokio::sync::Semaphore;
use tokio::task::JoinHandle;
use xeno_language::syntax::{SealedSource, Syntax, SyntaxError, SyntaxOptions};
use xeno_language::{LanguageId, LanguageLoader};
use xeno_primitives::{ChangeSet, DocumentId, Rope};

use super::engine::SyntaxEngine;
use super::types::{DocEpoch, OptKey, TaskId, ViewportKey};

/// Categorization of syntax tasks for metrics and scheduling.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TaskClass {
	Full,
	Incremental,
	Viewport,
}

/// The type of parse work to perform in a background task.
pub(super) enum TaskKind {
	FullParse {
		content: Rope,
	},
	ViewportParse {
		content: Rope,
		window: std::ops::Range<u32>,
	},
	Incremental {
		base: Syntax,
		old_rope: Rope,
		new_rope: Rope,
		composed: ChangeSet,
	},
}

impl TaskKind {
	pub fn class(&self) -> TaskClass {
		match self {
			Self::FullParse { .. } => TaskClass::Full,
			Self::ViewportParse { .. } => TaskClass::Viewport,
			Self::Incremental { .. } => TaskClass::Incremental,
		}
	}
}

/// Input specification for a background syntax task (what to parse and how).
pub(super) struct TaskSpec {
	pub(super) doc_id: DocumentId,
	pub(super) epoch: DocEpoch,
	pub(super) doc_version: u64,
	pub(super) lang_id: LanguageId,
	pub(super) opts_key: OptKey,
	pub(super) opts: SyntaxOptions,
	pub(super) kind: TaskKind,
	pub(super) loader: Arc<LanguageLoader>,
	pub(super) viewport_key: Option<ViewportKey>,
	pub(super) viewport_lane: Option<super::scheduling::ViewportLane>,
}

/// Output of a completed background syntax task, returned via the join handle.
pub(super) struct TaskDone {
	pub(super) id: TaskId,
	pub(super) doc_id: DocumentId,
	pub(super) epoch: DocEpoch,
	pub(super) doc_version: u64,
	pub(super) lang_id: LanguageId,
	pub(super) opts_key: OptKey,
	pub(super) result: Result<Syntax, SyntaxError>,
	pub(super) class: TaskClass,
	pub(super) elapsed: Duration,
	pub(super) viewport_key: Option<ViewportKey>,
	pub(super) viewport_lane: Option<super::scheduling::ViewportLane>,
}

/// Invariant enforcement: Collector for background syntax tasks.
pub(crate) struct TaskCollector {
	next_id: u64,
	tasks: FxHashMap<u64, JoinHandle<TaskDone>>,
	runtime: xeno_worker::WorkerRuntime,
}

impl TaskCollector {
	pub(super) fn new_with_runtime(runtime: xeno_worker::WorkerRuntime) -> Self {
		Self {
			next_id: 0,
			tasks: FxHashMap::default(),
			runtime,
		}
	}

	/// Spawns a background syntax task.
	///
	/// `privileged` tasks (urgent viewport Stage-A) can consume any available
	/// permit. Non-privileged tasks (full, incremental, Stage-B enrichment)
	/// must leave `reserve` permits available for urgent work.
	pub(super) fn spawn(&mut self, permits: Arc<Semaphore>, engine: Arc<dyn SyntaxEngine>, spec: TaskSpec, reserve: usize, privileged: bool) -> Option<TaskId> {
		let permit = if privileged {
			permits.try_acquire_owned().ok()?
		} else {
			let available = permits.available_permits();
			if available <= reserve {
				return None;
			}
			permits.try_acquire_owned().ok()?
		};

		let id_val = self.next_id;
		self.next_id = self.next_id.wrapping_add(1);
		let task_id = TaskId(id_val);

		let class = spec.kind.class();
		let viewport_key = spec.viewport_key;
		let viewport_lane = spec.viewport_lane;

		let handle = xeno_worker::spawn_blocking(xeno_worker::TaskClass::CpuBlocking, move || {
			let _permit = permit; // Tie permit lifetime to closure

			let t0 = Instant::now();
			let result = match spec.kind {
				TaskKind::FullParse { content } => engine.parse(content.slice(..), spec.lang_id, &spec.loader, spec.opts),
				TaskKind::ViewportParse { content, window, .. } => {
					if let Some(data) = spec.loader.get(spec.lang_id) {
						let repair: xeno_language::syntax::ViewportRepair = data.viewport_repair();
						let forward_haystack = if window.end < content.len_bytes() as u32 {
							Some(content.byte_slice(window.end as usize..))
						} else {
							None
						};
						let plan = repair.scan(content.byte_slice(window.start as usize..window.end as usize), forward_haystack);
						let end = (window.end as usize + plan.extension_bytes as usize).min(content.len_bytes());
						let sealed = Arc::new(SealedSource::from_window(content.byte_slice(window.start as usize..end), &plan.suffix));
						Syntax::new_viewport(sealed, spec.lang_id, &spec.loader, spec.opts, window.start)
					} else {
						Err(SyntaxError::NoLanguage)
					}
				}
				TaskKind::Incremental {
					base,
					old_rope,
					new_rope,
					composed,
				} => engine.update_incremental(base, old_rope.slice(..), new_rope.slice(..), &composed, spec.lang_id, &spec.loader, spec.opts),
			};
			let elapsed = t0.elapsed();

			TaskDone {
				id: task_id,
				doc_id: spec.doc_id,
				epoch: spec.epoch,
				doc_version: spec.doc_version,
				lang_id: spec.lang_id,
				opts_key: spec.opts_key,
				result,
				class,
				elapsed,
				viewport_key,
				viewport_lane,
			}
		});

		self.tasks.insert(id_val, handle);
		Some(task_id)
	}

	pub(super) fn drain_finished(&mut self) -> Vec<TaskDone> {
		let mut done = Vec::new();

		self.tasks.retain(|_, handle| {
			match xeno_primitives::future::poll_once(handle) {
				None => true, // Still running, keep it
				Some(Ok(task_done)) => {
					done.push(task_done);
					false // Done, remove it
				}
				Some(Err(e)) => {
					tracing::error!("Syntax task join error: {}", e);
					false // Done (crashed), remove it
				}
			}
		});

		done
	}

	pub(super) fn any_finished(&self) -> bool {
		self.tasks.values().any(|h| h.is_finished())
	}
}
