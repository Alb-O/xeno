//! Grammar fetching from git repositories.

use std::fs;
use std::process::Command;

use tracing::info;

use super::config::{GrammarConfig, GrammarSource, grammar_sources_dir};
use super::{GrammarBuildError, Result};

/// Status of a fetch operation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FetchStatus {
	/// Grammar was already up to date.
	UpToDate,
	/// Grammar was updated to a new revision.
	Updated,
	/// Grammar uses a local path (no fetch needed).
	Local,
}

/// Check if git is available on PATH.
fn ensure_git_available() -> Result<()> {
	Command::new("git")
		.arg("--version")
		.output()
		.map_err(|_| GrammarBuildError::GitNotAvailable)?;
	Ok(())
}

/// Fetches a grammar from its git repository.
///
/// Checks the current revision before fetching to avoid unnecessary network calls.
/// Returns [`FetchStatus::Local`] for non-git sources.
pub fn fetch_grammar(grammar: &GrammarConfig) -> Result<FetchStatus> {
	let GrammarSource::Git {
		remote, revision, ..
	} = &grammar.source
	else {
		return Ok(FetchStatus::Local);
	};

	ensure_git_available()?;

	let grammar_dir = grammar_sources_dir().join(&grammar.grammar_id);
	fs::create_dir_all(&grammar_dir)?;

	if is_valid_git_repo(&grammar_dir) {
		update_existing_repo(&grammar_dir, &grammar.grammar_id, revision)
	} else {
		clone_fresh(&grammar_dir, &grammar.grammar_id, remote, revision)
	}
}

fn is_valid_git_repo(dir: &std::path::Path) -> bool {
	dir.join(".git").join("HEAD").exists()
}

fn update_existing_repo(
	grammar_dir: &std::path::Path,
	grammar_id: &str,
	revision: &str,
) -> Result<FetchStatus> {
	let current_rev = git_rev_parse(grammar_dir)?;

	if current_rev.starts_with(revision) || revision.starts_with(&current_rev) {
		return Ok(FetchStatus::UpToDate);
	}

	info!(grammar = %grammar_id, "Updating grammar");
	git_fetch(grammar_dir, revision)?;
	git_checkout(grammar_dir, "FETCH_HEAD")?;

	Ok(FetchStatus::Updated)
}

fn clone_fresh(
	grammar_dir: &std::path::Path,
	grammar_id: &str,
	remote: &str,
	revision: &str,
) -> Result<FetchStatus> {
	if grammar_dir.exists() {
		fs::remove_dir_all(grammar_dir)?;
		fs::create_dir_all(grammar_dir)?;
	}

	info!(grammar = %grammar_id, "Cloning grammar");
	git_clone(remote, grammar_dir)?;
	git_fetch(grammar_dir, revision).or_else(|_| git_fetch_full(grammar_dir, revision))?;
	git_checkout(grammar_dir, revision).or_else(|_| git_checkout(grammar_dir, "FETCH_HEAD"))?;

	Ok(FetchStatus::Updated)
}

fn git_rev_parse(dir: &std::path::Path) -> Result<String> {
	let output = Command::new("git")
		.args(["rev-parse", "HEAD"])
		.current_dir(dir)
		.output()
		.map_err(|e| GrammarBuildError::GitCommand(e.to_string()))?;

	Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

fn git_fetch(dir: &std::path::Path, revision: &str) -> Result<()> {
	run_git(dir, &["fetch", "--depth", "1", "origin", revision])
}

fn git_fetch_full(dir: &std::path::Path, revision: &str) -> Result<()> {
	run_git(dir, &["fetch", "origin", revision])
}

fn git_checkout(dir: &std::path::Path, target: &str) -> Result<()> {
	run_git(dir, &["checkout", target])
}

fn git_clone(remote: &str, dest: &std::path::Path) -> Result<()> {
	let output = Command::new("git")
		.args(["clone", "--depth", "1", "--single-branch", remote])
		.arg(dest)
		.output()
		.map_err(|e| GrammarBuildError::GitCommand(e.to_string()))?;

	if output.status.success() {
		Ok(())
	} else {
		Err(GrammarBuildError::GitCommand(
			String::from_utf8_lossy(&output.stderr).into(),
		))
	}
}

fn run_git(dir: &std::path::Path, args: &[&str]) -> Result<()> {
	let output = Command::new("git")
		.args(args)
		.current_dir(dir)
		.output()
		.map_err(|e| GrammarBuildError::GitCommand(e.to_string()))?;

	if output.status.success() {
		Ok(())
	} else {
		Err(GrammarBuildError::GitCommand(
			String::from_utf8_lossy(&output.stderr).into(),
		))
	}
}
