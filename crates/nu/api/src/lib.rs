//! Public Nu API boundary for Xeno.
//!
//! Re-exports the runtime facade and Xeno-owned value model, plus NUON parsing
//! into `xeno-nu-data::Value`.

use std::error::Error;
use std::fmt;

pub use xeno_nu_data::{NuRecord, NuSpan, NuType, NuValue, Record, Span, Value};
pub use xeno_nu_runtime::{CompileError, ExecError, ExportId, NuProgram, ProgramPolicy};

/// Error emitted while parsing NUON source.
#[derive(Debug, Clone)]
pub enum NuonError {
	Parse(String),
	UnsupportedValue(String),
}

impl NuonError {
	fn parse(error: impl fmt::Display) -> Self {
		Self::Parse(error.to_string())
	}

	fn unsupported(error: impl fmt::Display) -> Self {
		Self::UnsupportedValue(error.to_string())
	}
}

impl fmt::Display for NuonError {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		match self {
			Self::Parse(message) | Self::UnsupportedValue(message) => f.write_str(message),
		}
	}
}

impl Error for NuonError {}

/// Parse NUON source into [`Value`].
pub fn parse_nuon(input: &str) -> Result<Value, NuonError> {
	let value = xeno_nu_nuon::from_nuon(input, None).map_err(NuonError::parse)?;
	Value::try_from(value).map_err(NuonError::unsupported)
}
