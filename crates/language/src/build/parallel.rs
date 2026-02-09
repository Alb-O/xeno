//! Parallel grammar fetching and building.

use std::sync::mpsc;
use std::thread;

use super::Result;
use super::compile::{BuildStatus, build_grammar};
use super::config::GrammarConfig;
use super::fetch::{FetchStatus, fetch_grammar};

/// Callback type for progress reporting.
pub type ProgressCallback = Box<dyn Fn(&str, &str) + Send + Sync>;

/// Fetch all grammars in parallel.
pub fn fetch_all_grammars(
	grammars: Vec<GrammarConfig>,
	on_progress: Option<ProgressCallback>,
) -> Vec<(GrammarConfig, Result<FetchStatus>)> {
	let (tx, rx) = mpsc::channel();
	let num_jobs = std::thread::available_parallelism()
		.map(|n| n.get())
		.unwrap_or(4)
		.min(8);

	let chunk_size = (grammars.len() / num_jobs).max(1);
	let chunks: Vec<Vec<GrammarConfig>> = grammars.chunks(chunk_size).map(|c| c.to_vec()).collect();

	for chunk in chunks {
		let tx = tx.clone();

		thread::spawn(move || {
			for grammar in chunk {
				let result = fetch_grammar(&grammar);
				let _ = tx.send((grammar, result));
			}
		});
	}

	drop(tx);

	let mut results = Vec::new();
	for (grammar, result) in rx {
		if let Some(ref cb) = on_progress {
			let status = match &result {
				Ok(FetchStatus::UpToDate) => "up to date",
				Ok(FetchStatus::Updated) => "updated",
				Ok(FetchStatus::Local) => "local",
				Err(_) => "error",
			};
			cb(&grammar.grammar_id, status);
		}
		results.push((grammar, result));
	}

	results
}

/// Build all grammars in parallel.
pub fn build_all_grammars(
	grammars: Vec<GrammarConfig>,
	on_progress: Option<ProgressCallback>,
) -> Vec<(GrammarConfig, Result<BuildStatus>)> {
	let (tx, rx) = mpsc::channel();
	let num_jobs = std::thread::available_parallelism()
		.map(|n| n.get())
		.unwrap_or(4)
		.min(8);

	let chunk_size = (grammars.len() / num_jobs).max(1);
	let chunks: Vec<Vec<GrammarConfig>> = grammars.chunks(chunk_size).map(|c| c.to_vec()).collect();

	for chunk in chunks {
		let tx = tx.clone();

		thread::spawn(move || {
			for grammar in chunk {
				let result = build_grammar(&grammar);
				let _ = tx.send((grammar, result));
			}
		});
	}

	drop(tx);

	let mut results = Vec::new();
	for (grammar, result) in rx {
		if let Some(ref cb) = on_progress {
			let status = match &result {
				Ok(BuildStatus::AlreadyBuilt) => "up to date",
				Ok(BuildStatus::Built) => "built",
				Err(_) => "error",
			};
			cb(&grammar.grammar_id, status);
		}
		results.push((grammar, result));
	}

	results
}
