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
mod tests;
