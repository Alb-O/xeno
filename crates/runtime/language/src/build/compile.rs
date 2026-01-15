//! Grammar compilation into dynamic libraries.

use std::fs;
use std::path::Path;
use std::process::Command;

use super::config::{GrammarConfig, get_grammar_src_dir, grammar_lib_dir, library_extension};
use super::{GrammarBuildError, Result};

/// Status of a build operation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BuildStatus {
	/// Grammar was already built and up to date.
	AlreadyBuilt,
	/// Grammar was newly built.
	Built,
}

/// Check if a grammar needs to be recompiled.
fn needs_recompile(src_dir: &Path, lib_path: &Path) -> bool {
	if !lib_path.exists() {
		return true;
	}

	let lib_mtime = match fs::metadata(lib_path).and_then(|m| m.modified()) {
		Ok(t) => t,
		Err(_) => return true,
	};

	// Check if any source file is newer than the library
	let source_files = ["parser.c", "scanner.c", "scanner.cc"];
	for file in source_files {
		let src_path = src_dir.join(file);
		if src_path.exists()
			&& let Ok(meta) = fs::metadata(&src_path)
			&& let Ok(src_mtime) = meta.modified()
			&& src_mtime > lib_mtime
		{
			return true;
		}
	}

	false
}

/// Build a single grammar into a dynamic library.
pub fn build_grammar(grammar: &GrammarConfig) -> Result<BuildStatus> {
	let src_dir = get_grammar_src_dir(grammar);
	let parser_path = src_dir.join("parser.c");

	if !parser_path.exists() {
		return Err(GrammarBuildError::NoParserSource(src_dir));
	}

	let lib_dir = grammar_lib_dir();
	fs::create_dir_all(&lib_dir)?;

	// Use lib prefix to match what load_grammar() expects
	let lib_name = format!(
		"lib{}.{}",
		grammar.grammar_id.replace('-', "_"),
		library_extension()
	);
	let lib_path = lib_dir.join(&lib_name);

	if !needs_recompile(&src_dir, &lib_path) {
		return Ok(BuildStatus::AlreadyBuilt);
	}

	// Set HOST and TARGET env vars if not present (needed outside cargo).
	let target = std::env::var("TARGET")
		.unwrap_or_else(|_| std::env::consts::ARCH.to_string() + "-unknown-linux-gnu");
	// SAFETY: We're setting env vars before any multi-threaded work happens in the cc crate
	unsafe {
		std::env::set_var("TARGET", &target);
		std::env::set_var("HOST", &target);
	}

	let mut build = cc::Build::new();
	build
		.opt_level(3)
		.cargo_metadata(false)
		.warnings(false)
		.include(&src_dir)
		.host(&target)
		.target(&target);

	build.file(&parser_path);

	let scanner_c = src_dir.join("scanner.c");
	let scanner_cc = src_dir.join("scanner.cc");

	if scanner_cc.exists() {
		build.cpp(true);
		build.file(&scanner_cc);
		build.std("c++14");
	} else if scanner_c.exists() {
		build.file(&scanner_c);
	}

	let obj_dir = lib_dir.join("obj").join(&grammar.grammar_id);
	fs::create_dir_all(&obj_dir)?;
	build.out_dir(&obj_dir);

	build
		.try_compile(&grammar.grammar_id)
		.map_err(|e| GrammarBuildError::Compilation(e.to_string()))?;

	compile_shared_library(&grammar.grammar_id, &src_dir, &lib_path)?;

	Ok(BuildStatus::Built)
}

/// Compile a grammar directly into a shared library using the system compiler.
fn compile_shared_library(_name: &str, src_dir: &Path, lib_path: &Path) -> Result<()> {
	let parser_path = src_dir.join("parser.c");
	let scanner_c = src_dir.join("scanner.c");
	let scanner_cc = src_dir.join("scanner.cc");

	#[cfg(unix)]
	{
		let compiler = if scanner_cc.exists() { "c++" } else { "cc" };

		let mut cmd = Command::new(compiler);
		cmd.arg("-shared")
			.arg("-fPIC")
			.arg("-O3")
			.arg("-fno-exceptions")
			.arg("-I")
			.arg(src_dir)
			.arg("-o")
			.arg(lib_path);

		cmd.arg(&parser_path);

		if scanner_cc.exists() {
			cmd.arg("-std=c++14");
			cmd.arg(&scanner_cc);
			cmd.arg("-lstdc++");
		} else if scanner_c.exists() {
			cmd.arg(&scanner_c);
		}

		// Security hardening on Linux
		#[cfg(target_os = "linux")]
		{
			cmd.arg("-Wl,-z,relro,-z,now");
		}

		let output = cmd
			.output()
			.map_err(|e| GrammarBuildError::Compilation(e.to_string()))?;

		if !output.status.success() {
			return Err(GrammarBuildError::Compilation(
				String::from_utf8_lossy(&output.stderr).to_string(),
			));
		}
	}

	#[cfg(windows)]
	{
		// Windows compilation using MSVC
		let mut cmd = Command::new("cl.exe");
		cmd.arg("/nologo")
			.arg("/LD")
			.arg("/O2")
			.arg("/utf-8")
			.arg(format!("/I{}", src_dir.display()))
			.arg(format!("/Fe:{}", lib_path.display()))
			.arg(&parser_path);

		if scanner_cc.exists() {
			cmd.arg("/std:c++14");
			cmd.arg(&scanner_cc);
		} else if scanner_c.exists() {
			cmd.arg(&scanner_c);
		}

		let output = cmd
			.output()
			.map_err(|e| GrammarBuildError::Compilation(e.to_string()))?;

		if !output.status.success() {
			return Err(GrammarBuildError::Compilation(
				String::from_utf8_lossy(&output.stderr).to_string(),
			));
		}
	}

	Ok(())
}
