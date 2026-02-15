use std::path::Path;

use super::*;

fn unique_temp_dir(prefix: &str) -> std::path::PathBuf {
	let nanos = std::time::SystemTime::now()
		.duration_since(std::time::UNIX_EPOCH)
		.expect("system time should be after unix epoch")
		.as_nanos();
	let dir = std::env::temp_dir().join(format!("xeno-{prefix}-{}-{nanos}", std::process::id()));
	std::fs::create_dir_all(&dir).expect("temp dir should be creatable");
	dir
}

fn write_file(path: &Path, content: &str) {
	std::fs::write(path, content).expect("file should be writable");
}

#[test]
fn eval_config_returns_config() {
	let config = eval_config_str("{ options: { tab-width: 4 } }", "config.nu").expect("config.nu should evaluate");
	let tab_width = crate::options::find("tab-width").expect("tab-width option should exist");
	assert_eq!(config.options.get(tab_width.dense_id()), Some(&crate::options::OptionValue::Int(4)));
}

#[test]
fn eval_config_rejects_external() {
	let err = eval_config_str("^echo hi; { options: { tab-width: 4 } }", "config.nu").expect_err("external commands must be rejected");
	assert!(matches!(err, ConfigError::NuParse(_)));
}

#[test]
fn eval_config_rejects_redirection() {
	let err = eval_config_str("1 > out.txt; { options: { tab-width: 4 } }", "config.nu").expect_err("redirection must be rejected");
	assert!(matches!(err, ConfigError::NuParse(_)));
}

#[test]
fn eval_config_rejects_while() {
	let err = eval_config_str("while true { }; { options: { tab-width: 4 } }", "config.nu").expect_err("while loops must be rejected");
	assert!(matches!(err, ConfigError::NuParse(_)));
}

#[test]
fn eval_config_merge_precedence() {
	let mut merged = crate::config::nuon::parse_config_str("{ options: { tab-width: 2 } }").expect("nuon config should parse");
	merged.merge(eval_config_str("{ options: { tab-width: 4 } }", "config.nu").expect("nu config should evaluate"));

	let tab_width = crate::options::find("tab-width").expect("tab-width option should exist");
	assert_eq!(merged.options.get(tab_width.dense_id()), Some(&crate::options::OptionValue::Int(4)));
}

#[test]
fn eval_config_allows_use_under_config_root() {
	let dir = unique_temp_dir("config-nu-use");
	write_file(&dir.join("mod.nu"), "export def tw [] { 4 }");
	let config_path = dir.join("config.nu");
	let config =
		eval_config_str("use mod.nu *\n{ options: { tab-width: (tw) } }", &config_path.to_string_lossy()).expect("use under config root should succeed");

	let tab_width = crate::options::find("tab-width").expect("tab-width option should exist");
	assert_eq!(config.options.get(tab_width.dense_id()), Some(&crate::options::OptionValue::Int(4)));

	let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn eval_config_rejects_use_parent_dir() {
	let base = unique_temp_dir("config-nu-parent");
	let config_dir = base.join("config");
	std::fs::create_dir_all(&config_dir).expect("config dir should be creatable");
	let outer_dir = base.join("outer");
	std::fs::create_dir_all(&outer_dir).expect("outer dir should be creatable");
	write_file(&outer_dir.join("evil.nu"), "export def nope [] { 1 }");
	let config_path = config_dir.join("config.nu");
	let err = eval_config_str("use ../outer/evil.nu *\n{ options: { tab-width: 4 } }", &config_path.to_string_lossy())
		.expect_err("parent traversal should be rejected");
	assert!(matches!(err, ConfigError::NuParse(_)));
	let _ = std::fs::remove_dir_all(base);
}

#[test]
fn config_nu_parses_structured_keys_with_prelude() {
	let input = r#"{
		keys: {
			normal: {
				"ctrl+s": (command write),
				"g r": (editor reload_config),
			}
		}
	}"#;
	let config = eval_config_str(input, "config.nu").expect("config.nu with prelude keys should evaluate");
	let keys = config.keys.expect("keys should be parsed");
	let normal = keys.modes.get("normal").expect("normal mode should exist");

	let ctrl_s = normal.get("ctrl+s").expect("ctrl+s binding should exist");
	assert!(matches!(ctrl_s, crate::Invocation::Command { name, args } if name == "write" && args.is_empty()));

	let gr = normal.get("g r").expect("g r binding should exist");
	assert!(matches!(gr, crate::Invocation::EditorCommand { name, args } if name == "reload_config" && args.is_empty()));
}

#[test]
fn config_nu_rejects_string_key_target() {
	let input = r#"{
		keys: {
			normal: {
				"ctrl+s": "command:write"
			}
		}
	}"#;
	let err = eval_config_str(input, "config.nu").expect_err("string key target should be rejected");
	match err {
		ConfigError::InvalidKeyBinding(msg) => assert!(msg.contains("expected invocation record"), "got: {msg}"),
		other => panic!("expected Nuon error, got: {other:?}"),
	}
}

#[test]
fn config_nu_rejects_missing_kind() {
	let input = r#"{
		keys: {
			normal: {
				"ctrl+s": { name: "write" }
			}
		}
	}"#;
	let err = eval_config_str(input, "config.nu").expect_err("missing kind should be rejected");
	match err {
		ConfigError::InvalidKeyBinding(msg) => assert!(msg.contains("kind"), "got: {msg}"),
		other => panic!("expected Nuon error, got: {other:?}"),
	}
}

#[test]
fn eval_config_rejects_use_wildcard() {
	let dir = unique_temp_dir("config-nu-wildcard");
	let config_path = dir.join("config.nu");
	let err = eval_config_str("use *.nu *\n{ options: { tab-width: 4 } }", &config_path.to_string_lossy()).expect_err("wildcard paths should be rejected");
	assert!(matches!(err, ConfigError::NuParse(_)));
	let _ = std::fs::remove_dir_all(dir);
}
