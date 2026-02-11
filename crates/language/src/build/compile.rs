//! Grammar compilation into dynamic libraries.

use std::fs;
use std::path::Path;
use std::process::{Command, Stdio};

use tracing::info;

use super::config::{GrammarConfig, get_grammar_src_dir, grammar_lib_dir, library_extension};
use super::{GrammarBuildError, Result};

/// Returns the first compiler from `candidates` that executes successfully.
fn find_compiler<'a>(candidates: &[&'a str]) -> Option<&'a str> {
	candidates
		.iter()
		.copied()
		.find(|name| Command::new(name).arg("--version").stdout(Stdio::null()).stderr(Stdio::null()).status().is_ok())
}

/// Resolves C and C++ compilers, preferring environment variables then probing common names.
///
/// On Unix, probes `cc`, `clang`, `gcc`. On Windows, probes `cl`, `clang-cl`, `clang`, `gcc`.
/// Returns `None` for a compiler if neither the environment variable nor any candidate is found.
fn resolve_compilers() -> (Option<&'static str>, Option<&'static str>) {
	static COMPILERS: std::sync::OnceLock<(Option<&'static str>, Option<&'static str>)> = std::sync::OnceLock::new();
	*COMPILERS.get_or_init(|| {
		#[cfg(unix)]
		const CC_CANDIDATES: &[&str] = &["cc", "clang", "gcc"];
		#[cfg(unix)]
		const CXX_CANDIDATES: &[&str] = &["c++", "clang++", "g++"];
		#[cfg(windows)]
		const CC_CANDIDATES: &[&str] = &["cl", "clang-cl", "clang", "gcc"];
		#[cfg(windows)]
		const CXX_CANDIDATES: &[&str] = &["cl", "clang-cl", "clang++", "g++"];
		#[cfg(not(any(unix, windows)))]
		const CC_CANDIDATES: &[&str] = &["cc", "clang", "gcc"];
		#[cfg(not(any(unix, windows)))]
		const CXX_CANDIDATES: &[&str] = &["c++", "clang++", "g++"];

		let cc = std::env::var("CC").ok().map(|s| s.leak() as &str).or_else(|| find_compiler(CC_CANDIDATES));
		let cxx = std::env::var("CXX").ok().map(|s| s.leak() as &str).or_else(|| find_compiler(CXX_CANDIDATES));
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

/// Compiles a Tree-sitter grammar into a dynamic library.
///
/// This function coordinates the compilation process:
/// 1. Verifies the presence of `parser.c`.
/// 2. Resolves suitable C/C++ compilers.
/// 3. Checks if a recompile is necessary by comparing mtimes.
/// 4. Compiles object files using the [`cc`] crate.
/// 5. Links the objects into a platform-specific shared library.
///
/// # Errors
///
/// * Returns [`GrammarBuildError::NoParserSource`] if the grammar source is incomplete.
/// * Returns [`GrammarBuildError::Compilation`] if either the compilation or linking stage fails.
pub fn build_grammar(grammar: &GrammarConfig) -> Result<BuildStatus> {
	let src_dir = get_grammar_src_dir(grammar);
	if !src_dir.join("parser.c").exists() {
		return Err(GrammarBuildError::NoParserSource(src_dir));
	}

	let lib_dir = grammar_lib_dir();
	fs::create_dir_all(&lib_dir)?;

	let lib_path = lib_dir.join(format!("lib{}.{}", grammar.grammar_id.replace('-', "_"), library_extension()));

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
	let (cc, cxx) = resolve_compilers();
	let compiler = if needs_cxx {
		cxx.ok_or_else(|| {
			GrammarBuildError::Compilation(format!(
				"C++ compiler required for {} but none found. \
				 Install clang++/g++ or set CXX env var.",
				grammar.grammar_id
			))
		})?
	} else {
		cc.ok_or_else(|| GrammarBuildError::Compilation("C compiler required but none found. Install clang/gcc or set CC env var.".into()))?
	};

	compile_objects(&src_dir, &lib_dir, &grammar.grammar_id, compiler, needs_cxx)?;
	link_shared_library(&src_dir, &lib_path, compiler, needs_cxx)?;

	if !lib_path.exists() {
		return Err(GrammarBuildError::Compilation(format!(
			"compilation succeeded but library not found at {}",
			lib_path.display()
		)));
	}

	tracing::debug!(grammar = %grammar.grammar_id, lib_path = %lib_path.display(), "Successfully compiled grammar");
	Ok(BuildStatus::Built)
}

fn compile_objects(src_dir: &Path, lib_dir: &Path, grammar_id: &str, compiler: &str, needs_cxx: bool) -> Result<()> {
	let target = std::env::var("TARGET").unwrap_or_else(|_| {
		let arch = std::env::consts::ARCH;
		if cfg!(target_os = "windows") {
			format!("{arch}-pc-windows-msvc")
		} else if cfg!(target_os = "macos") {
			format!("{arch}-apple-darwin")
		} else {
			format!("{arch}-unknown-linux-gnu")
		}
	});

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
		.compiler(compiler)
		.file(src_dir.join("parser.c"));

	if needs_cxx && scanner_cc.exists() {
		build.cpp(true).file(&scanner_cc).std("c++14");
	} else if scanner_c.exists() {
		build.file(&scanner_c);
	}

	let obj_dir = lib_dir.join("obj").join(grammar_id);
	fs::create_dir_all(&obj_dir)?;
	build.out_dir(&obj_dir);

	build.try_compile(grammar_id).map_err(|e| GrammarBuildError::Compilation(e.to_string()))
}

/// Links source files into a shared library using the system compiler.
fn link_shared_library(src_dir: &Path, lib_path: &Path, compiler: &str, needs_cxx: bool) -> Result<()> {
	let scanner_cc = src_dir.join("scanner.cc");
	let scanner_c = src_dir.join("scanner.c");

	#[cfg(unix)]
	{
		let mut cmd = Command::new(compiler);
		cmd.args(["-shared", "-fPIC", "-O3", "-fno-exceptions"])
			.arg("-I")
			.arg(src_dir)
			.arg("-o")
			.arg(lib_path)
			.arg(src_dir.join("parser.c"));

		if needs_cxx && scanner_cc.exists() {
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

		if needs_cxx && scanner_cc.exists() {
			cmd.arg("/std:c++14").arg(&scanner_cc);
		} else if scanner_c.exists() {
			cmd.arg(&scanner_c);
		}

		run_compiler(cmd)
	}
}

fn run_compiler(mut cmd: Command) -> Result<()> {
	let output = cmd.output().map_err(|e| GrammarBuildError::Compilation(e.to_string()))?;

	if output.status.success() {
		Ok(())
	} else {
		Err(GrammarBuildError::Compilation(String::from_utf8_lossy(&output.stderr).into()))
	}
}
