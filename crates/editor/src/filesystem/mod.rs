//! Filesystem indexing and picker backend state.

mod indexer;
mod search;
mod service;
mod types;

pub(crate) use indexer::{FilesystemOptions, run_filesystem_index};
pub(crate) use search::{apply_search_delta, run_search_query};
pub(crate) use service::FsService;
