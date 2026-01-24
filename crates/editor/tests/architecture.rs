//! Architecture boundary enforcement tests.
//!
//! These tests verify architectural invariants established by the refactor.
//! They use external tools (rg, cargo) to check the codebase at test time.

use std::process::Command;

/// Verifies no `cfg(feature = "lsp")` guards exist in the render path.
///
/// The render code should use unconditional types with empty defaults when
/// LSP is disabled, eliminating code path divergence.
#[test]
fn no_cfg_lsp_in_render() {
	let output = Command::new("rg")
		.args([
			"#\\[cfg\\(feature = \"lsp\"\\)\\]",
			"crates/editor/src/render",
		])
		.output()
		.expect("rg command failed");

	let stdout = String::from_utf8_lossy(&output.stdout);
	assert!(
		stdout.is_empty(),
		"Found cfg(feature = \"lsp\") in render path:\n{}",
		stdout
	);
}

/// Verifies actions crate doesn't directly import motions crate.
///
/// Actions should emit MotionId requests, not call motion handlers directly.
/// The transitive dependency through textobj is acceptable.
#[test]
fn actions_no_direct_motions_import() {
	let output = Command::new("rg")
		.args([
			"use xeno_registry_motions",
			"crates/registry/actions/src",
			"--type",
			"rust",
		])
		.output()
		.expect("rg command failed");

	let stdout = String::from_utf8_lossy(&output.stdout);
	assert!(
		stdout.is_empty(),
		"Actions crate directly imports motions:\n{}",
		stdout
	);
}

/// Verifies actions Cargo.toml doesn't list motions as direct dependency.
#[test]
fn actions_cargo_no_motions_dep() {
	let output = Command::new("rg")
		.args([
			"xeno-registry-motions",
			"crates/registry/actions/Cargo.toml",
		])
		.output()
		.expect("rg command failed");

	let stdout = String::from_utf8_lossy(&output.stdout);
	assert!(
		stdout.is_empty(),
		"Actions Cargo.toml lists motions dependency:\n{}",
		stdout
	);
}

/// Verifies the project builds with default features.
#[test]
fn build_default_features() {
	let status = Command::new("cargo")
		.args(["check", "--all-targets"])
		.status()
		.expect("cargo check failed");

	assert!(status.success(), "cargo check --all-targets failed");
}

/// Verifies the project builds without default features (LSP disabled).
#[test]
#[ignore = "slow: full build without lsp"]
fn build_no_default_features() {
	let status = Command::new("cargo")
		.args(["check", "--no-default-features"])
		.status()
		.expect("cargo check failed");

	assert!(status.success(), "cargo check --no-default-features failed");
}

/// Verifies motion IDs in primitives match actual motion definitions.
#[test]
fn motion_ids_match_definitions() {
	let ids_output = Command::new("rg")
		.args([
			r#"MotionId\("xeno-registry-motions::(\w+)"\)"#,
			"crates/primitives/src/ids.rs",
			"-or",
			"$1",
		])
		.output()
		.expect("rg command failed");

	let ids_str = String::from_utf8_lossy(&ids_output.stdout);
	let ids: Vec<_> = ids_str.lines().filter(|s| !s.is_empty()).collect();

	let defs_output = Command::new("rg")
		.args([
			r#"motion!\((\w+),"#,
			"crates/registry/motions/src",
			"-or",
			"$1",
		])
		.output()
		.expect("rg command failed");

	let defs_str = String::from_utf8_lossy(&defs_output.stdout);
	let defs: Vec<_> = defs_str.lines().filter(|s| !s.is_empty()).collect();

	for id in &ids {
		assert!(
			defs.contains(id),
			"MotionId '{}' referenced in primitives but not defined in motions crate",
			id
		);
	}
}
