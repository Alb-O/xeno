//! Grammar compilation into dynamic libraries.

use std::fs;
use std::path::Path;
use std::process::{Command, Stdio};

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
/// Library name uses `lib` prefix to match [`load_grammar`](super::load_grammar) expectations.
pub fn build_grammar(grammar: &GrammarConfig) -> Result<BuildStatus> {
	let src_dir = get_grammar_src_dir(grammar);
	let parser_path = src_dir.join("parser.c");

	if !parser_path.exists() {
		return Err(GrammarBuildError::NoParserSource(src_dir));
	}

	let lib_dir = grammar_lib_dir();
	fs::create_dir_all(&lib_dir)?;

	let lib_path = lib_dir.join(format!(
		"lib{}.{}",
		grammar.grammar_id.replace('-', "_"),
		library_extension()
	));

	if !needs_recompile(&src_dir, &lib_path) {
		return Ok(BuildStatus::AlreadyBuilt);
	}

	let scanner_cc = src_dir.join("scanner.cc");
	let scanner_c = src_dir.join("scanner.c");
	let needs_cxx = scanner_cc.exists();

	let (cc, cxx) = resolve_compilers();
	let compiler = if needs_cxx {
		cxx.ok_or_else(|| {
			GrammarBuildError::Compilation(format!(
				"C++ compiler required for {} (scanner.cc) but none found. \
				 Install clang++/g++ or set CXX env var.",
				grammar.grammar_id
			))
		})?
	} else {
		cc.ok_or_else(|| {
			GrammarBuildError::Compilation(
				"C compiler required but none found. Install clang/gcc or set CC env var."
					.to_string(),
			)
		})?
	};

	let target = std::env::var("TARGET")
		.unwrap_or_else(|_| format!("{}-unknown-linux-gnu", std::env::consts::ARCH));

	// SAFETY: env vars set before cc crate spawns threads
	unsafe {
		std::env::set_var("TARGET", &target);
		std::env::set_var("HOST", &target);
		std::env::set_var("CC", cc.unwrap_or(compiler));
		std::env::set_var("CXX", cxx.unwrap_or(compiler));
	}

	let mut build = cc::Build::new();
	build
		.opt_level(3)
		.cargo_metadata(false)
		.warnings(false)
		.include(&src_dir)
		.host(&target)
		.target(&target)
		.file(&parser_path);

	if scanner_cc.exists() {
		build.cpp(true).file(&scanner_cc).std("c++14");
	} else if scanner_c.exists() {
		build.file(&scanner_c);
	}

	let obj_dir = lib_dir.join("obj").join(&grammar.grammar_id);
	fs::create_dir_all(&obj_dir)?;
	build.out_dir(&obj_dir);

	build
		.try_compile(&grammar.grammar_id)
		.map_err(|e| GrammarBuildError::Compilation(e.to_string()))?;

	compile_shared_library(&src_dir, &lib_path)?;

	Ok(BuildStatus::Built)
}

/// Links object files into a shared library using the system compiler.
///
/// Caller must ensure required compiler exists (checked in [`build_grammar`]).
fn compile_shared_library(src_dir: &Path, lib_path: &Path) -> Result<()> {
	let parser_path = src_dir.join("parser.c");
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
			.arg(&parser_path);

		if scanner_cc.exists() {
			cmd.args(["-std=c++14", "-lstdc++"]).arg(&scanner_cc);
		} else if scanner_c.exists() {
			cmd.arg(&scanner_c);
		}

		#[cfg(target_os = "linux")]
		cmd.arg("-Wl,-z,relro,-z,now");

		let output = cmd
			.output()
			.map_err(|e| GrammarBuildError::Compilation(e.to_string()))?;

		if !output.status.success() {
			return Err(GrammarBuildError::Compilation(
				String::from_utf8_lossy(&output.stderr).into(),
			));
		}
	}

	#[cfg(windows)]
	{
		let mut cmd = Command::new("cl.exe");
		cmd.args(["/nologo", "/LD", "/O2", "/utf-8"])
			.arg(format!("/I{}", src_dir.display()))
			.arg(format!("/Fe:{}", lib_path.display()))
			.arg(&parser_path);

		if scanner_cc.exists() {
			cmd.arg("/std:c++14").arg(&scanner_cc);
		} else if scanner_c.exists() {
			cmd.arg(&scanner_c);
		}

		let output = cmd
			.output()
			.map_err(|e| GrammarBuildError::Compilation(e.to_string()))?;

		if !output.status.success() {
			return Err(GrammarBuildError::Compilation(
				String::from_utf8_lossy(&output.stderr).into(),
			));
		}
	}

	Ok(())
}
