use std::fs;
use std::sync::{Arc, mpsc};
use std::time::Duration;

use super::{FilesystemOptions, run_filesystem_index};
use crate::filesystem::types::IndexMsg;

#[test]
fn indexer_streams_relative_normalized_paths() {
	let temp_dir = tempfile::tempdir().expect("create tempdir");
	let root = temp_dir.path();

	let src = root.join("src");
	fs::create_dir_all(&src).expect("create src dir");
	fs::write(src.join("main.rs"), "fn main() {}\n").expect("write main");
	fs::write(src.join("lib.rs"), "pub fn lib() {}\n").expect("write lib");

	let (tx, rx) = mpsc::channel();
	run_filesystem_index(1, root.to_path_buf(), FilesystemOptions::default(), Arc::new(move |msg| tx.send(msg).is_ok()));
	let mut seen_paths: Vec<String> = Vec::new();

	loop {
		let Ok(msg) = rx.recv_timeout(Duration::from_secs(2)) else {
			break;
		};
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
