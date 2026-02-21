//! NUON → [`LanguagesSpec`] compiler.
//!
//! Language definitions live in `languages.nuon`. Tree-sitter query files
//! (`.scm`) are pulled from a pinned Helix runtime checkout and merged into
//! each language's spec at build time. Optional local overrides can be added
//! under `assets/queries/`.

use std::collections::{BTreeMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

use serde::Deserialize;

use crate::build_support::compile::*;
use crate::schema::languages::*;

#[derive(Debug, Deserialize)]
struct HelixRuntimeLock {
	upstream: String,
	commit: String,
}

pub fn build(ctx: &BuildCtx) {
	let root = ctx.asset("src/domains/languages/assets");
	ctx.rerun_tree(&root);

	let path = root.join("languages.nuon");
	let mut spec: LanguagesSpec = read_nuon_spec(&path);

	let lock_path = root.join("helix_runtime.nuon");
	let lock: HelixRuntimeLock = read_nuon_spec(&lock_path);

	let helix_queries_root = ensure_helix_queries_checkout(ctx, &lock);
	let local_overrides_root = root.join("queries");
	let mut query_roots = vec![helix_queries_root];
	if local_overrides_root.is_dir() {
		query_roots.push(local_overrides_root);
	}

	for lang in &mut spec.langs {
		merge_queries(lang, &query_roots);
	}

	let mut seen = HashSet::new();
	for lang in &spec.langs {
		if !seen.insert(&lang.common.name) {
			panic!("duplicate language name: '{}'", lang.common.name);
		}
	}

	let bin = postcard::to_stdvec(&spec).expect("failed to serialize languages spec");
	ctx.write_blob("languages.bin", &bin);
}

fn merge_queries(lang: &mut LanguageSpec, query_roots: &[PathBuf]) {
	let mut merged: BTreeMap<String, String> = lang.queries.iter().map(|q| (q.kind.clone(), q.text.clone())).collect();

	for root in query_roots {
		let lang_dir = root.join(&lang.common.name);
		if !lang_dir.is_dir() {
			continue;
		}

		for path in collect_files_sorted(&lang_dir, "scm") {
			let kind = path
				.file_stem()
				.and_then(|stem| stem.to_str())
				.unwrap_or_else(|| panic!("invalid query filename (utf-8): {}", path.display()))
				.to_string();
			let text = fs::read_to_string(&path).unwrap_or_else(|e| panic!("failed to read query {}: {e}", path.display()));
			merged.insert(kind, text);
		}
	}

	lang.queries = merged.into_iter().map(|(kind, text)| LanguageQuerySpec { kind, text }).collect();
}

fn ensure_helix_queries_checkout(ctx: &BuildCtx, lock: &HelixRuntimeLock) -> PathBuf {
	let checkout_dir = helix_checkout_dir(ctx, &lock.commit);
	let queries_dir = checkout_dir.join("runtime").join("queries");

	if is_valid_checkout(&checkout_dir, &lock.commit) {
		return queries_dir;
	}

	if checkout_dir.exists() {
		fs::remove_dir_all(&checkout_dir).unwrap_or_else(|e| panic!("failed to remove stale checkout {}: {e}", checkout_dir.display()));
	}

	let cache_root = checkout_dir.parent().expect("helix checkout path always has parent");
	fs::create_dir_all(cache_root).unwrap_or_else(|e| panic!("failed to create cache root {}: {e}", cache_root.display()));

	let nonce = SystemTime::now()
		.duration_since(UNIX_EPOCH)
		.expect("system time should be after unix epoch")
		.as_nanos();
	let staging = cache_root.join(format!(".tmp-helix-runtime-{}-{nonce}", std::process::id()));
	if staging.exists() {
		fs::remove_dir_all(&staging).unwrap_or_else(|e| panic!("failed to cleanup staging {}: {e}", staging.display()));
	}

	clone_sparse_helix_queries(&staging, lock);
	match fs::rename(&staging, &checkout_dir) {
		Ok(()) => {}
		Err(_) if is_valid_checkout(&checkout_dir, &lock.commit) => {
			fs::remove_dir_all(&staging).unwrap_or_else(|e| panic!("failed to cleanup staging {}: {e}", staging.display()));
		}
		Err(e) => {
			panic!(
				"failed to promote helix runtime checkout from {} to {}: {e}",
				staging.display(),
				checkout_dir.display()
			);
		}
	}

	let queries_dir = checkout_dir.join("runtime").join("queries");
	if !queries_dir.is_dir() {
		panic!("helix runtime checkout missing queries dir: {}", queries_dir.display());
	}
	queries_dir
}

fn helix_checkout_dir(ctx: &BuildCtx, commit: &str) -> PathBuf {
	target_dir(ctx).join("external").join("helix-runtime").join(commit)
}

fn target_dir(ctx: &BuildCtx) -> PathBuf {
	let workspace_root = workspace_root(&ctx.manifest_dir);
	match std::env::var_os("CARGO_TARGET_DIR") {
		Some(dir) => {
			let path = PathBuf::from(dir);
			if path.is_absolute() {
				path
			} else {
				workspace_root.join(path)
			}
		}
		None => workspace_root.join("target"),
	}
}

fn workspace_root(manifest_dir: &Path) -> PathBuf {
	manifest_dir
		.parent()
		.and_then(Path::parent)
		.unwrap_or_else(|| panic!("expected crate manifest dir under workspace: {}", manifest_dir.display()))
		.to_path_buf()
}

fn is_valid_checkout(checkout_dir: &Path, commit: &str) -> bool {
	if !checkout_dir.join("runtime").join("queries").is_dir() {
		return false;
	}
	match run_git_capture(checkout_dir, &["rev-parse", "HEAD"]) {
		Ok(head) => head.trim() == commit,
		Err(_) => false,
	}
}

fn clone_sparse_helix_queries(checkout_dir: &Path, lock: &HelixRuntimeLock) {
	fs::create_dir_all(checkout_dir).unwrap_or_else(|e| panic!("failed to create checkout dir {}: {e}", checkout_dir.display()));

	run_git(checkout_dir, &["init", "-q"]);
	run_git(checkout_dir, &["remote", "add", "origin", &lock.upstream]);
	run_git(checkout_dir, &["config", "core.sparseCheckout", "true"]);

	let sparse_checkout = checkout_dir.join(".git").join("info").join("sparse-checkout");
	fs::write(&sparse_checkout, "/runtime/queries/\n").unwrap_or_else(|e| panic!("failed to write sparse-checkout {}: {e}", sparse_checkout.display()));

	run_git(checkout_dir, &["fetch", "--depth=1", "origin", &lock.commit, "-q"]);
	run_git(checkout_dir, &["checkout", "FETCH_HEAD", "-q"]);

	let head = run_git_capture(checkout_dir, &["rev-parse", "HEAD"])
		.unwrap_or_else(|e| panic!("failed to resolve fetched helix commit in {}: {e}", checkout_dir.display()));
	if head.trim() != lock.commit {
		panic!("helix checkout resolved to {} but lock requires {}", head.trim(), lock.commit);
	}
}

fn run_git(dir: &Path, args: &[&str]) {
	run_git_capture(dir, args).unwrap_or_else(|e| panic!("git {:?} failed in {}: {e}", args, dir.display()));
}

fn run_git_capture(dir: &Path, args: &[&str]) -> Result<String, String> {
	let output = Command::new("git").args(args).current_dir(dir).output().map_err(|e| e.to_string())?;

	if output.status.success() {
		Ok(String::from_utf8_lossy(&output.stdout).to_string())
	} else {
		Err(String::from_utf8_lossy(&output.stderr).to_string())
	}
}
