//! Config file loading utilities.

use std::path::{Path, PathBuf};

use super::{Config, ConfigWarning};

/// Aggregate result of loading user configuration layers.
#[derive(Debug, Default)]
pub struct ConfigLoadReport {
	/// Merged config if any layer was loaded successfully.
	pub config: Option<Config>,
	/// Non-fatal parse warnings keyed by source file path.
	pub warnings: Vec<(PathBuf, ConfigWarning)>,
	/// File read or parse/eval errors keyed by source file path.
	pub errors: Vec<(PathBuf, String)>,
}

/// Loads and merges user configuration from `config.kdl`, `config.nuon`, and `config.nu`.
///
/// Merge precedence is fixed and deterministic:
/// `config.kdl` < `config.nuon` < `config.nu`.
pub fn load_user_config_from_dir(config_dir: &Path) -> ConfigLoadReport {
	let mut report = ConfigLoadReport::default();
	let mut merged = Config::default();
	let mut found_any = false;

	#[cfg(feature = "config-kdl")]
	load_layer(&mut report, &mut merged, &mut found_any, config_dir, "config.kdl", |content, _| {
		crate::config::kdl::parse_config_str(content)
	});

	#[cfg(feature = "config-nuon")]
	load_layer(&mut report, &mut merged, &mut found_any, config_dir, "config.nuon", |content, _| {
		crate::config::nuon::parse_config_str(content)
	});

	#[cfg(feature = "config-nu")]
	load_layer(&mut report, &mut merged, &mut found_any, config_dir, "config.nu", |content, path| {
		crate::config::nu::eval_config_str(content, &path.to_string_lossy())
	});

	if found_any {
		report.config = Some(merged);
	}

	report
}

fn load_layer<F>(report: &mut ConfigLoadReport, merged: &mut Config, found_any: &mut bool, config_dir: &Path, filename: &str, parser: F)
where
	F: FnOnce(&str, &Path) -> super::Result<Config>,
{
	let path = config_dir.join(filename);
	if !path.exists() {
		return;
	}

	let content = match std::fs::read_to_string(&path) {
		Ok(content) => content,
		Err(error) => {
			report.errors.push((path, error.to_string()));
			return;
		}
	};

	merge_layer(report, merged, found_any, &path, parser(&content, &path));
}

fn merge_layer(report: &mut ConfigLoadReport, merged: &mut Config, found_any: &mut bool, path: &Path, layer: super::Result<Config>) {
	match layer {
		Ok(mut config) => {
			let path_buf = path.to_path_buf();
			for warning in config.warnings.drain(..) {
				report.warnings.push((path_buf.clone(), warning));
			}
			merged.merge(config);
			*found_any = true;
		}
		Err(error) => {
			report.errors.push((path.to_path_buf(), error.to_string()));
		}
	}
}

#[cfg(test)]
mod tests {
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

	#[test]
	fn load_kdl_layer() {
		let dir = unique_temp_dir("kdl");
		write_file(&dir.join("config.kdl"), "options { tab-width 2 }");

		let report = load_user_config_from_dir(&dir);
		let config = report.config.expect("kdl config should load");
		let tab_width = crate::options::find("tab-width").expect("tab-width option should exist");
		assert_eq!(config.options.get(tab_width.dense_id()), Some(&crate::options::OptionValue::Int(2)));
		assert!(report.errors.is_empty());

		let _ = std::fs::remove_dir_all(dir);
	}

	#[cfg(all(feature = "config-nuon", feature = "config-nu"))]
	#[test]
	fn load_order_precedence_kdl_nuon_nu() {
		let dir = unique_temp_dir("precedence");
		write_file(&dir.join("config.kdl"), "options { tab-width 2 }");
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
		write_file(&dir.join("config.kdl"), "options { tab-width 2 }");
		write_file(&dir.join("config.nuon"), "{ options: ");

		let report = load_user_config_from_dir(&dir);
		let config = report.config.expect("valid layers should still merge");
		let tab_width = crate::options::find("tab-width").expect("tab-width option should exist");
		assert_eq!(config.options.get(tab_width.dense_id()), Some(&crate::options::OptionValue::Int(2)));
		assert!(report.errors.iter().any(|(path, _)| path.ends_with("config.nuon")));

		let _ = std::fs::remove_dir_all(dir);
	}
}
