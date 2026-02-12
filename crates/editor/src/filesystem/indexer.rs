use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::mpsc::{self, Receiver, SyncSender};
use std::thread;
use std::time::{Duration, Instant};

use ignore::{DirEntry, Error as IgnoreError, WalkBuilder, WalkState};

use super::types::{FileRow, IndexKind, IndexMsg, IndexUpdate, ProgressSnapshot};

const DISPATCH_INTERVAL: Duration = Duration::from_millis(120);
const MIN_BATCH_SIZE: usize = 32;
const MAX_BATCH_SIZE: usize = 1_024;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FilesystemOptions {
	pub include_hidden: bool,
	pub follow_symlinks: bool,
	pub respect_ignore_files: bool,
	pub git_ignore: bool,
	pub git_global: bool,
	pub git_exclude: bool,
	pub max_depth: Option<usize>,
	pub threads: usize,
	pub allowed_extensions: Option<Vec<String>>,
	pub file_channel_capacity: usize,
	pub update_channel_capacity: usize,
}

impl Default for FilesystemOptions {
	fn default() -> Self {
		Self {
			include_hidden: false,
			follow_symlinks: false,
			respect_ignore_files: true,
			git_ignore: true,
			git_global: true,
			git_exclude: true,
			max_depth: None,
			threads: 0,
			allowed_extensions: None,
			file_channel_capacity: 8_192,
			update_channel_capacity: 64,
		}
	}
}

impl FilesystemOptions {
	pub fn thread_count(&self) -> usize {
		self.threads.max(1)
	}

	fn extension_filter(&self) -> Option<HashSet<String>> {
		self.allowed_extensions.as_ref().map(|extensions| {
			extensions
				.iter()
				.map(|ext| ext.trim().trim_start_matches('.').to_ascii_lowercase())
				.filter(|ext| !ext.is_empty())
				.collect::<HashSet<_>>()
		})
	}
}

pub fn spawn_filesystem_index(generation: u64, root: PathBuf, options: FilesystemOptions) -> Receiver<IndexMsg> {
	let (update_tx, update_rx) = mpsc::sync_channel(options.update_channel_capacity.max(1));

	thread::spawn(move || run_indexer(generation, root, options, update_tx));

	update_rx
}

fn run_indexer(generation: u64, root: PathBuf, options: FilesystemOptions, update_tx: SyncSender<IndexMsg>) {
	let start = Instant::now();
	tracing::info!(generation, root = %root.display(), threads = options.thread_count(), "fs.index.start");

	let reset_msg = IndexMsg::Update(IndexUpdate {
		generation,
		kind: IndexKind::Live,
		reset: true,
		files: Arc::from(Vec::<FileRow>::new()),
		progress: ProgressSnapshot {
			indexed_files: 0,
			total_files: None,
			complete: false,
		},
		cached_data: None,
	});

	if update_tx.send(reset_msg).is_err() {
		return;
	}

	let (file_tx, file_rx) = mpsc::sync_channel::<FileRow>(options.file_channel_capacity.max(1));
	let aggregator_tx = update_tx.clone();
	let aggregator = thread::spawn(move || aggregate_files(generation, file_rx, aggregator_tx));

	let extension_filter = options.extension_filter().map(Arc::new);
	let walk_root = Arc::new(root.clone());
	build_walk(&root, &options).build_parallel().run(|| {
		let sender = file_tx.clone();
		let root = Arc::clone(&walk_root);
		let extension_filter = extension_filter.clone();
		Box::new(move |entry: Result<DirEntry, IgnoreError>| {
			if let Ok(entry) = entry {
				let Some(file_type) = entry.file_type() else {
					return WalkState::Continue;
				};
				if !file_type.is_file() {
					return WalkState::Continue;
				}

				let path = entry.path();
				let relative = path.strip_prefix(root.as_path()).unwrap_or(path);
				if let Some(filter) = extension_filter.as_ref() {
					let extension = relative.extension().and_then(|ext| ext.to_str()).map(|ext| ext.to_ascii_lowercase());
					match extension {
						Some(ext) if filter.contains(&ext) => {}
						_ => return WalkState::Continue,
					}
				}

				let relative_display = relative.to_string_lossy().replace('\\', "/");
				if sender.send(FileRow::new(Arc::<str>::from(relative_display))).is_err() {
					return WalkState::Quit;
				}
			}

			WalkState::Continue
		})
	});

	drop(file_tx);
	let indexed_files = aggregator.join().unwrap_or_default();
	let elapsed_ms = start.elapsed().as_millis() as u64;

	let _ = update_tx.send(IndexMsg::Complete {
		generation,
		indexed_files,
		elapsed_ms,
	});

	tracing::debug!(generation, indexed_files, elapsed_ms, "fs.index.complete");
}

fn aggregate_files(generation: u64, file_rx: Receiver<FileRow>, update_tx: SyncSender<IndexMsg>) -> usize {
	let mut pending_files = Vec::new();
	let mut indexed_files = 0usize;
	let mut last_dispatch = Instant::now();

	while let Ok(file) = file_rx.recv() {
		indexed_files += 1;
		pending_files.push(file);

		let flush_size = batch_size_for(indexed_files);
		if pending_files.len() >= flush_size || last_dispatch.elapsed() >= DISPATCH_INTERVAL {
			if !flush_update(generation, indexed_files, false, &mut pending_files, &update_tx) {
				return indexed_files;
			}
			last_dispatch = Instant::now();
		}
	}

	let _ = flush_update(generation, indexed_files, true, &mut pending_files, &update_tx);
	indexed_files
}

fn flush_update(generation: u64, indexed_files: usize, complete: bool, pending_files: &mut Vec<FileRow>, update_tx: &SyncSender<IndexMsg>) -> bool {
	if pending_files.is_empty() && !complete {
		return true;
	}

	let files: Arc<[FileRow]> = std::mem::take(pending_files).into();
	let msg = IndexMsg::Update(IndexUpdate {
		generation,
		kind: IndexKind::Live,
		reset: false,
		files,
		progress: ProgressSnapshot {
			indexed_files,
			total_files: complete.then_some(indexed_files),
			complete,
		},
		cached_data: None,
	});

	if update_tx.send(msg).is_err() {
		return false;
	}

	tracing::trace!(generation, indexed_files, complete, "fs.index.flush");
	true
}

fn batch_size_for(indexed_files: usize) -> usize {
	if indexed_files < 1_024 {
		MIN_BATCH_SIZE
	} else if indexed_files < 16_384 {
		256
	} else {
		MAX_BATCH_SIZE
	}
}

fn build_walk(root: &Path, options: &FilesystemOptions) -> WalkBuilder {
	let mut walker = WalkBuilder::new(root);

	walker
		.hidden(!options.include_hidden)
		.follow_links(options.follow_symlinks)
		.git_ignore(options.git_ignore)
		.git_global(options.git_global)
		.git_exclude(options.git_exclude)
		.ignore(options.respect_ignore_files)
		.parents(true)
		.max_depth(options.max_depth)
		.threads(options.thread_count());

	walker
}

#[cfg(test)]
mod tests {
	use std::fs;
	use std::time::Duration;

	use super::{FilesystemOptions, spawn_filesystem_index};
	use crate::filesystem::IndexMsg;

	#[test]
	fn indexer_streams_relative_normalized_paths() {
		let temp_dir = tempfile::tempdir().expect("create tempdir");
		let root = temp_dir.path();

		let src = root.join("src");
		fs::create_dir_all(&src).expect("create src dir");
		fs::write(src.join("main.rs"), "fn main() {}\n").expect("write main");
		fs::write(src.join("lib.rs"), "pub fn lib() {}\n").expect("write lib");

		let rx = spawn_filesystem_index(1, root.to_path_buf(), FilesystemOptions::default());
		let mut seen_paths: Vec<String> = Vec::new();

		loop {
			let msg = rx.recv_timeout(Duration::from_secs(2)).expect("expected index message");
			match msg {
				IndexMsg::Update(update) => {
					for file in update.files.iter() {
						seen_paths.push(file.path.to_string());
					}
					if update.progress.complete {
						break;
					}
				}
				IndexMsg::Complete { .. } => {
					break;
				}
				IndexMsg::Error { .. } => {}
			}
		}

		assert!(seen_paths.iter().any(|p| p == "src/main.rs"));
		assert!(seen_paths.iter().any(|p| p == "src/lib.rs"));
		assert!(seen_paths.iter().all(|p| !p.contains('\\')));
	}
}
