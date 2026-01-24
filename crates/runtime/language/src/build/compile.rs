//! Grammar compilation into dynamic libraries.

use std::fs;
use std::path::Path;
use std::process::{Command, Stdio};

use tracing::info;

use super::config::{GrammarConfig, get_grammar_src_dir, grammar_lib_dir, library_extension};
use super::{GrammarBuildError, Result};

/// Returns the first compiler from `candidates` that executes successfully.
fn find_compiler<'a>(candidates: &[&'a str]) -> Option<&'a str> {
	candidates.iter().copied().find(|name| {
		Command::new(name)
			.arg("--version")
			.stdout(Stdio::null())
			.stderr(Stdio::null())
			.status()
			.is_ok()
	})
}

/// Resolves C and C++ compilers, preferring env vars then probing common names.
///
/// Returns `None` for a compiler if neither env var nor any candidate is found.
fn resolve_compilers() -> (Option<&'static str>, Option<&'static str>) {
	static COMPILERS: std::sync::OnceLock<(Option<&'static str>, Option<&'static str>)> =
		std::sync::OnceLock::new();
	*COMPILERS.get_or_init(|| {
		let cc = std::env::var("CC")
			.ok()
			.map(|s| s.leak() as &str)
			.or_else(|| find_compiler(&["cc", "clang", "gcc"]));
		let cxx = std::env::var("CXX")
			.ok()
			.map(|s| s.leak() as &str)
			.or_else(|| find_compiler(&["c++", "clang++", "g++"]));
		(cc, cxx)
	})
}

/// Status of a build operation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BuildStatus {
	/// Grammar was already built and up to date.
	AlreadyBuilt,
	/// Grammar was newly built.
	Built,
}

/// Returns true if any source file is newer than the compiled library.
fn needs_recompile(src_dir: &Path, lib_path: &Path) -> bool {
	let Ok(lib_mtime) = fs::metadata(lib_path).and_then(|m| m.modified()) else {
		return true;
	};

	["parser.c", "scanner.c", "scanner.cc"].iter().any(|file| {
		fs::metadata(src_dir.join(file))
			.and_then(|m| m.modified())
			.is_ok_and(|src_mtime| src_mtime > lib_mtime)
	})
}

/// Compiles a tree-sitter grammar into a dynamic library.
///
/// Uses [`cc`] to compile object files, then links into a shared library.
/// Skips compilation if the library is newer than all source files.
///
/// # Errors
///
/// Returns [`GrammarBuildError::NoParserSource`] if `parser.c` is missing,
/// or [`GrammarBuildError::Compilation`] if compilation fails.
pub fn build_grammar(grammar: &GrammarConfig) -> Result<BuildStatus> {
	let src_dir = get_grammar_src_dir(grammar);
	if !src_dir.join("parser.c").exists() {
		return Err(GrammarBuildError::NoParserSource(src_dir));
	}

	let lib_dir = grammar_lib_dir();
	fs::create_dir_all(&lib_dir)?;

	let lib_path = lib_dir.join(format!(
		"lib{}.{}",
		grammar.grammar_id.replace('-', "_"),
		library_extension()
	));

	tracing::debug!(
		grammar = %grammar.grammar_id,
		lib_path = %lib_path.display(),
		lib_exists = lib_path.exists(),
		"Grammar library path"
	);

	if !needs_recompile(&src_dir, &lib_path) {
		return Ok(BuildStatus::AlreadyBuilt);
	}

	info!(grammar = %grammar.grammar_id, lib_path = %lib_path.display(), "Compiling grammar");

	let needs_cxx = src_dir.join("scanner.cc").exists();
	let compiler = get_compiler(needs_cxx, &grammar.grammar_id)?;

	setup_cc_env(compiler)?;
	compile_objects(&src_dir, &lib_dir, &grammar.grammar_id)?;
	link_shared_library(&src_dir, &lib_path)?;

	if !lib_path.exists() {
		return Err(GrammarBuildError::Compilation(format!(
			"compilation succeeded but library not found at {}",
			lib_path.display()
		)));
	}

	tracing::debug!(grammar = %grammar.grammar_id, lib_path = %lib_path.display(), "Successfully compiled grammar");
	Ok(BuildStatus::Built)
}

fn get_compiler(needs_cxx: bool, grammar_id: &str) -> Result<&'static str> {
	let (cc, cxx) = resolve_compilers();
	if needs_cxx {
		cxx.ok_or_else(|| {
			GrammarBuildError::Compilation(format!(
				"C++ compiler required for {grammar_id} but none found. \
				 Install clang++/g++ or set CXX env var."
			))
		})
	} else {
		cc.ok_or_else(|| {
			GrammarBuildError::Compilation(
				"C compiler required but none found. Install clang/gcc or set CC env var.".into(),
			)
		})
	}
}

fn setup_cc_env(compiler: &str) -> Result<()> {
	let (cc, cxx) = resolve_compilers();
	let target = std::env::var("TARGET")
		.unwrap_or_else(|_| format!("{}-unknown-linux-gnu", std::env::consts::ARCH));

	// SAFETY: env vars set before cc crate spawns threads
	unsafe {
		std::env::set_var("TARGET", &target);
		std::env::set_var("HOST", &target);
		std::env::set_var("CC", cc.unwrap_or(compiler));
		std::env::set_var("CXX", cxx.unwrap_or(compiler));
	}
	Ok(())
}

fn compile_objects(src_dir: &Path, lib_dir: &Path, grammar_id: &str) -> Result<()> {
	let target = std::env::var("TARGET")
		.unwrap_or_else(|_| format!("{}-unknown-linux-gnu", std::env::consts::ARCH));

	let scanner_cc = src_dir.join("scanner.cc");
	let scanner_c = src_dir.join("scanner.c");

	let mut build = cc::Build::new();
	build
		.opt_level(3)
		.cargo_metadata(false)
		.warnings(false)
		.include(src_dir)
		.host(&target)
		.target(&target)
		.file(src_dir.join("parser.c"));

	if scanner_cc.exists() {
		build.cpp(true).file(&scanner_cc).std("c++14");
	} else if scanner_c.exists() {
		build.file(&scanner_c);
	}

	let obj_dir = lib_dir.join("obj").join(grammar_id);
	fs::create_dir_all(&obj_dir)?;
	build.out_dir(&obj_dir);

	build
		.try_compile(grammar_id)
		.map_err(|e| GrammarBuildError::Compilation(e.to_string()))
}

/// Links source files into a shared library using the system compiler.
fn link_shared_library(src_dir: &Path, lib_path: &Path) -> Result<()> {
	let scanner_cc = src_dir.join("scanner.cc");
	let scanner_c = src_dir.join("scanner.c");

	#[cfg(unix)]
	{
		let (cc, cxx) = resolve_compilers();
		let compiler = if scanner_cc.exists() {
			cxx.expect("C++ compiler checked in build_grammar")
		} else {
			cc.expect("C compiler checked in build_grammar")
		};

		let mut cmd = Command::new(compiler);
		cmd.args(["-shared", "-fPIC", "-O3", "-fno-exceptions"])
			.arg("-I")
			.arg(src_dir)
			.arg("-o")
			.arg(lib_path)
			.arg(src_dir.join("parser.c"));

		if scanner_cc.exists() {
			cmd.args(["-std=c++14", "-lstdc++"]).arg(&scanner_cc);
		} else if scanner_c.exists() {
			cmd.arg(&scanner_c);
		}

		#[cfg(target_os = "linux")]
		cmd.arg("-Wl,-z,relro,-z,now");

		run_compiler(cmd)
	}

	#[cfg(windows)]
	{
		let mut cmd = Command::new("cl.exe");
		cmd.args(["/nologo", "/LD", "/O2", "/utf-8"])
			.arg(format!("/I{}", src_dir.display()))
			.arg(format!("/Fe:{}", lib_path.display()))
			.arg(src_dir.join("parser.c"));

		if scanner_cc.exists() {
			cmd.arg("/std:c++14").arg(&scanner_cc);
		} else if scanner_c.exists() {
			cmd.arg(&scanner_c);
		}

		run_compiler(cmd)
	}
}

fn run_compiler(mut cmd: Command) -> Result<()> {
	let output = cmd
		.output()
		.map_err(|e| GrammarBuildError::Compilation(e.to_string()))?;

	if output.status.success() {
		Ok(())
	} else {
		Err(GrammarBuildError::Compilation(
			String::from_utf8_lossy(&output.stderr).into(),
		))
	}
}
