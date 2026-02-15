//! Nu script configuration parsing for Xeno.

use std::path::Path;

use super::{Config, ConfigError, Result};

/// Evaluate a Nu script and parse its resulting value as [`Config`].
pub fn eval_config_str(input: &str, fname: &str) -> Result<Config> {
	let config_root = Path::new(fname).parent();
	let mut engine_state = xeno_nu::create_engine_state(config_root).map_err(ConfigError::NuParse)?;
	let parsed = xeno_nu::parse_and_validate(&mut engine_state, fname, input, config_root).map_err(ConfigError::NuParse)?;
	let value = xeno_nu::evaluate_block(&engine_state, parsed.block.as_ref()).map_err(ConfigError::NuRuntime)?;

	if value.as_record().is_err() {
		return Err(ConfigError::NuRuntime("config.nu must evaluate to a record value".to_string()));
	}

	crate::config::nuon::parse_config_value(&value)
}

#[cfg(test)]
mod tests;
