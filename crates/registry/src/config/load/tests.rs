use std::path::Path;

use super::*;

fn unique_temp_dir(prefix: &str) -> PathBuf {
	let nanos = std::time::SystemTime::now()
		.duration_since(std::time::UNIX_EPOCH)
		.expect("system time should be after unix epoch")
		.as_nanos();
	let dir = std::env::temp_dir().join(format!("xeno-config-load-{prefix}-{}-{nanos}", std::process::id()));
	std::fs::create_dir_all(&dir).expect("temp dir should be creatable");
	dir
}

fn write_file(path: &Path, content: &str) {
	std::fs::write(path, content).expect("file should be writable");
}

#[test]
fn load_ignores_missing_files() {
	let dir = unique_temp_dir("missing");
	let report = load_user_config_from_dir(&dir);
	assert!(report.config.is_none());
	assert!(report.warnings.is_empty());
	assert!(report.errors.is_empty());
	let _ = std::fs::remove_dir_all(dir);
}

#[cfg(feature = "config-nuon")]
#[test]
fn load_nuon_layer() {
	let dir = unique_temp_dir("nuon");
	write_file(&dir.join("config.nuon"), "{ options: { tab-width: 2 } }");

	let report = load_user_config_from_dir(&dir);
	let config = report.config.expect("nuon config should load");
	let tab_width = crate::options::find("tab-width").expect("tab-width option should exist");
	assert_eq!(config.options.get(tab_width.dense_id()), Some(&crate::options::OptionValue::Int(2)));
	assert!(report.errors.is_empty());

	let _ = std::fs::remove_dir_all(dir);
}

#[cfg(all(feature = "config-nuon", feature = "config-nu"))]
#[test]
fn load_order_precedence_nuon_nu() {
	let dir = unique_temp_dir("precedence");
	write_file(&dir.join("config.nuon"), "{ options: { tab-width: 3 } }");
	write_file(&dir.join("config.nu"), "{ options: { tab-width: 4 } }");

	let report = load_user_config_from_dir(&dir);
	let config = report.config.expect("all layers should load");
	let tab_width = crate::options::find("tab-width").expect("tab-width option should exist");
	assert_eq!(config.options.get(tab_width.dense_id()), Some(&crate::options::OptionValue::Int(4)));
	assert!(report.errors.is_empty());

	let _ = std::fs::remove_dir_all(dir);
}

#[cfg(feature = "config-nu")]
#[test]
fn load_nu_use_module_under_root() {
	let dir = unique_temp_dir("nu-use");
	write_file(&dir.join("mod.nu"), "export def tw [] { 4 }");
	write_file(&dir.join("config.nu"), "use mod.nu *\n{ options: { tab-width: (tw) } }");

	let report = load_user_config_from_dir(&dir);
	let config = report.config.expect("nu config should load");
	let tab_width = crate::options::find("tab-width").expect("tab-width option should exist");
	assert_eq!(config.options.get(tab_width.dense_id()), Some(&crate::options::OptionValue::Int(4)));
	assert!(report.errors.is_empty());

	let _ = std::fs::remove_dir_all(dir);
}

#[cfg(feature = "config-nuon")]
#[test]
fn load_collects_diagnostics_per_file() {
	let dir = unique_temp_dir("diagnostics");
	write_file(&dir.join("config.nuon"), "{ options: ");

	let report = load_user_config_from_dir(&dir);
	assert!(report.config.is_none(), "broken nuon should not produce config");
	assert!(report.errors.iter().any(|(path, _)| path.ends_with("config.nuon")));

	let _ = std::fs::remove_dir_all(dir);
}
