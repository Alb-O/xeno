#[cfg(unix)]
use std::os::unix::fs::symlink as create_file_symlink;
#[cfg(windows)]
use std::os::windows::fs::symlink_file as create_file_symlink;
use std::path::Path;

use super::*;

fn sandbox_check(source: &str, config_root: Option<&Path>) -> Result<(), String> {
	let mut engine_state = super::super::create_engine_state(config_root).expect("engine state");
	let mut working_set = xeno_nu_protocol::engine::StateWorkingSet::new(&engine_state);
	let block = xeno_nu_parser::parse(&mut working_set, Some("<test>"), source.as_bytes(), false);
	if let Some(err) = working_set.parse_errors.first() {
		return Err(format!("parse error: {err}"));
	}
	ensure_sandboxed(&working_set, block.as_ref(), config_root)?;
	let delta = working_set.render();
	engine_state.merge_delta(delta).map_err(|e| e.to_string())?;
	Ok(())
}

#[test]
fn blocks_external_command() {
	let err = sandbox_check("^ls", None).unwrap_err();
	assert!(err.contains("external") || err.contains("parse error"), "{err}");
}

#[test]
fn blocks_run_external() {
	let err = sandbox_check("run-external 'ls'", None).unwrap_err();
	assert!(err.contains("external") || err.contains("parse error"), "{err}");
}

#[test]
fn blocks_filesystem_commands() {
	for cmd in ["open foo.txt", "save bar.txt", "rm baz", "cp a b", "ls", "cd /tmp", "mkdir d"] {
		let result = sandbox_check(cmd, None);
		assert!(result.is_err(), "{cmd} should be blocked: {result:?}");
	}
}

#[test]
fn blocks_networking() {
	let err = sandbox_check("http get https://example.com", None).unwrap_err();
	assert!(
		err.contains("network") || err.contains("external") || err.contains("not allowed") || err.contains("parse error"),
		"{err}"
	);
}

#[test]
fn blocks_looping() {
	for cmd in ["while true { }", "for x in [1 2] { }", "loop { break }"] {
		let err = sandbox_check(cmd, None).unwrap_err();
		assert!(
			err.contains("looping") || err.contains("parse error") || err.contains("external"),
			"{cmd}: {err}"
		);
	}
}

#[test]
fn blocks_external_in_match_guard() {
	let err = sandbox_check("match 1 { 1 if (^ls) => 1, _ => 0 }", None).unwrap_err();
	assert!(err.contains("external") || err.contains("parse error"), "{err}");
}

#[test]
fn blocks_run_external_in_match_guard() {
	let err = sandbox_check("match 1 { 1 if (run-external 'ls') => 1, _ => 0 }", None).unwrap_err();
	assert!(err.contains("run-external") || err.contains("external") || err.contains("not allowed"), "{err}");
}

#[test]
fn blocks_redirection() {
	let err = sandbox_check("1 | save out.txt", None).unwrap_err();
	assert!(
		err.contains("not allowed") || err.contains("filesystem") || err.contains("external") || err.contains("parse error"),
		"{err}"
	);
}

#[test]
fn allows_pure_record() {
	sandbox_check("{ name: 'test', value: 42 }", None).expect("pure record should pass");
}

#[test]
fn allows_function_defs() {
	sandbox_check("def greet [name: string] { $'hello ($name)' }\ngreet 'world'", None).expect("function defs should pass");
}

#[test]
fn blocks_parent_dir_in_use() {
	let temp = tempfile::tempdir().unwrap();
	let err = sandbox_check("use ../evil.nu", Some(temp.path())).unwrap_err();
	assert!(err.contains("parent") || err.contains("parse error"), "{err}");
}

#[test]
fn allows_use_star_import_within_root() {
	let temp = tempfile::tempdir().unwrap();
	std::fs::write(temp.path().join("helper.nu"), "export def x [] { 1 }").unwrap();
	sandbox_check("use helper.nu *", Some(temp.path())).expect("use with star import within root should pass");
}

#[test]
fn allows_export_use_star_import_within_root() {
	let temp = tempfile::tempdir().unwrap();
	std::fs::write(temp.path().join("helper.nu"), "export def x [] { 1 }").unwrap();
	sandbox_check("export use helper.nu *", Some(temp.path())).expect("export use with star import within root should pass");
}

#[test]
fn allows_nested_use_relative_to_module() {
	let temp = tempfile::tempdir().unwrap();
	let sub = temp.path().join("sub");
	std::fs::create_dir_all(&sub).unwrap();
	std::fs::write(sub.join("b.nu"), "export def from_b [] { 7 }").unwrap();
	std::fs::write(sub.join("a.nu"), "use b.nu *\nexport def from_a [] { from_b }").unwrap();

	sandbox_check("use sub/a.nu *\nfrom_a", Some(temp.path())).expect("nested module-relative use should pass");
}

#[test]
fn allows_directory_module_with_mod_nu() {
	let temp = tempfile::tempdir().unwrap();
	let pkg = temp.path().join("pkg");
	std::fs::create_dir_all(&pkg).unwrap();
	std::fs::write(pkg.join("mod.nu"), "export def from_pkg [] { 9 }").unwrap();

	sandbox_check("use pkg *\nfrom_pkg", Some(temp.path())).expect("directory module with mod.nu should pass");
}

#[test]
#[cfg_attr(windows, ignore = "requires symlink privilege")]
fn blocks_use_symlink_escape() {
	let temp = tempfile::tempdir().unwrap();
	let config_root = temp.path().join("config");
	std::fs::create_dir(&config_root).unwrap();

	let outside = temp.path().join("outside.nu");
	std::fs::write(&outside, "export def x [] { 1 }").unwrap();
	create_file_symlink(&outside, config_root.join("helper.nu")).unwrap();

	let err = sandbox_check("use helper.nu *", Some(&config_root)).unwrap_err();
	assert!(err.contains("outside the config directory root") || err.contains("outside"), "{err}");
}

#[test]
fn blocks_extern_decl() {
	let err = sandbox_check("extern ls []", None).unwrap_err();
	assert!(err.contains("external signatures") || err.contains("parse error"), "{err}");
}

#[test]
fn blocks_export_extern_decl() {
	let err = sandbox_check("export extern git []", None).unwrap_err();
	assert!(err.contains("external signatures") || err.contains("parse error"), "{err}");
}

#[test]
fn blocks_bare_path_at_statement_level() {
	// A bare path at statement level parses as an external call, which is blocked.
	let err = sandbox_check(r#"/tmp/foo"#, None).unwrap_err();
	assert!(err.contains("disabled"), "{err}");
}

#[test]
fn use_with_module_path_still_allowed() {
	let temp = tempfile::tempdir().expect("temp dir");
	std::fs::write(temp.path().join("helper.nu"), "export def greet [] { 'hi' }").expect("write helper");
	std::fs::write(temp.path().join("xeno.nu"), "use helper.nu *\ngreet").expect("write xeno.nu");
	sandbox_check("use helper.nu *\ngreet", Some(temp.path())).expect("use with path should be allowed");
}

#[test]
fn decl_inventory_audit() {
	let engine_state = super::super::create_engine_state(None).expect("engine state");
	let mut decls: Vec<String> = engine_state
		.get_decls_sorted(false)
		.into_iter()
		.map(|(name, _)| String::from_utf8_lossy(&name).to_string())
		.collect();
	decls.sort();

	println!("Nu decl inventory ({} commands)", decls.len());
	for decl in decls {
		println!("{decl}");
	}
}
