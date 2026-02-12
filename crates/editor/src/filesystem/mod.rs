//! Filesystem indexing and picker backend state.

mod indexer;
mod search;
mod service;
mod types;

pub(crate) use indexer::{FilesystemOptions, spawn_filesystem_index};
pub(crate) use search::spawn_search_worker;
pub(crate) use service::FsService;
pub(crate) use types::*;
