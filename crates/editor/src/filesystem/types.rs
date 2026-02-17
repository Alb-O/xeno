use std::path::PathBuf;
use std::sync::Arc;

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct FileRow {
	pub path: Arc<str>,
}

impl FileRow {
	pub fn new(path: impl Into<Arc<str>>) -> Self {
		Self { path: path.into() }
	}
}

#[derive(Clone, Debug, Default)]
pub struct SearchData {
	pub root: Option<PathBuf>,
	pub files: Vec<FileRow>,
}

#[derive(Debug, Clone, Copy, Default)]
pub struct ProgressSnapshot {
	pub indexed_files: usize,
	pub total_files: Option<usize>,
	pub complete: bool,
}

#[derive(Debug, Clone)]
pub struct IndexUpdate {
	pub generation: u64,
	pub reset: bool,
	pub files: Arc<[FileRow]>,
	pub progress: ProgressSnapshot,
	pub cached_data: Option<SearchData>,
}

#[derive(Debug)]
pub enum IndexMsg {
	Update(IndexUpdate),
	Error { generation: u64, message: Arc<str> },
	Complete { generation: u64, indexed_files: usize, elapsed_ms: u64 },
}

#[derive(Debug)]
pub enum IndexDelta {
	Reset,
	Replace(SearchData),
	Append(Arc<[FileRow]>),
}

#[derive(Debug, Clone)]
pub struct SearchRow {
	pub path: Arc<str>,
	pub score: i32,
	pub match_indices: Option<Vec<usize>>,
}

#[derive(Debug)]
pub enum SearchMsg {
	Result {
		generation: u64,
		id: u64,
		query: String,
		rows: Arc<[SearchRow]>,
		scanned: usize,
		elapsed_ms: u64,
	},
}
