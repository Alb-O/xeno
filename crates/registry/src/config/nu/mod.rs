//! Nu script configuration parsing for Xeno.

use std::path::Path;

use super::{Config, ConfigError, Result};

/// Evaluate a Nu script and parse its resulting value as [`Config`].
pub fn eval_config_str(input: &str, fname: &str) -> Result<Config> {
	let config_root = Path::new(fname).parent();
	let program = xeno_nu_api::NuProgram::compile_config_script(fname, input, config_root).map_err(|error| ConfigError::NuParse(error.to_string()))?;
	let value = program.execute_root().map_err(|error| ConfigError::NuRuntime(error.to_string()))?;

	if value.as_record().is_err() {
		return Err(ConfigError::NuRuntime("config.nu must evaluate to a record value".to_string()));
	}

	crate::config::nuon::parse_config_value(&value)
}

#[cfg(test)]
mod tests;
