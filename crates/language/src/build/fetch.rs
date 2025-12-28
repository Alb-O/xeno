//! Grammar fetching from git repositories.

use std::fs;
use std::process::Command;

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

/// Fetch a single grammar from its git repository.
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

	// Check for .git/HEAD to ensure this is a valid git repo, not a partial clone
	if grammar_dir.join(".git").join("HEAD").exists() {
		let fetch_output = Command::new("git")
			.args(["fetch", "--depth", "1", "origin", revision])
			.current_dir(&grammar_dir)
			.output()
			.map_err(|e| GrammarBuildError::GitCommand(e.to_string()))?;

		if !fetch_output.status.success() {
			return Err(GrammarBuildError::GitCommand(
				String::from_utf8_lossy(&fetch_output.stderr).to_string(),
			));
		}

		// Check if we're already at the right revision
		let rev_parse = Command::new("git")
			.args(["rev-parse", "HEAD"])
			.current_dir(&grammar_dir)
			.output()
			.map_err(|e| GrammarBuildError::GitCommand(e.to_string()))?;

		let current_rev = String::from_utf8_lossy(&rev_parse.stdout)
			.trim()
			.to_string();

		if current_rev.starts_with(revision) || revision.starts_with(&current_rev) {
			return Ok(FetchStatus::UpToDate);
		}

		// Checkout the new revision
		let checkout_output = Command::new("git")
			.args(["checkout", "FETCH_HEAD"])
			.current_dir(&grammar_dir)
			.output()
			.map_err(|e| GrammarBuildError::GitCommand(e.to_string()))?;

		if !checkout_output.status.success() {
			return Err(GrammarBuildError::GitCommand(
				String::from_utf8_lossy(&checkout_output.stderr).to_string(),
			));
		}

		Ok(FetchStatus::Updated)
	} else {
		// Clean up any partial/corrupted clone before trying again
		if grammar_dir.exists() {
			fs::remove_dir_all(&grammar_dir)?;
			fs::create_dir_all(&grammar_dir)?;
		}

		let clone_output = Command::new("git")
			.args([
				"clone",
				"--depth",
				"1",
				"--single-branch",
				remote,
				grammar_dir.to_str().unwrap(),
			])
			.output()
			.map_err(|e| GrammarBuildError::GitCommand(e.to_string()))?;

		if !clone_output.status.success() {
			return Err(GrammarBuildError::GitCommand(
				String::from_utf8_lossy(&clone_output.stderr).to_string(),
			));
		}

		// Fetch the specific revision
		let fetch_output = Command::new("git")
			.args(["fetch", "--depth", "1", "origin", revision])
			.current_dir(&grammar_dir)
			.output()
			.map_err(|e| GrammarBuildError::GitCommand(e.to_string()))?;

		if !fetch_output.status.success() {
			// Try without depth for older git versions or if revision is a branch
			let fetch_output = Command::new("git")
				.args(["fetch", "origin", revision])
				.current_dir(&grammar_dir)
				.output()
				.map_err(|e| GrammarBuildError::GitCommand(e.to_string()))?;

			if !fetch_output.status.success() {
				return Err(GrammarBuildError::GitCommand(
					String::from_utf8_lossy(&fetch_output.stderr).to_string(),
				));
			}
		}

		// Checkout the revision
		let checkout_output = Command::new("git")
			.args(["checkout", revision])
			.current_dir(&grammar_dir)
			.output()
			.map_err(|e| GrammarBuildError::GitCommand(e.to_string()))?;

		if !checkout_output.status.success() {
			// Try FETCH_HEAD if direct checkout fails
			let checkout_output = Command::new("git")
				.args(["checkout", "FETCH_HEAD"])
				.current_dir(&grammar_dir)
				.output()
				.map_err(|e| GrammarBuildError::GitCommand(e.to_string()))?;

			if !checkout_output.status.success() {
				return Err(GrammarBuildError::GitCommand(
					String::from_utf8_lossy(&checkout_output.stderr).to_string(),
				));
			}
		}

		Ok(FetchStatus::Updated)
	}
}
