use std::num::NonZeroI32;

use job::JobError;
use miette::{Diagnostic, LabeledSpan, NamedSource};
use serde::{Deserialize, Serialize};
use thiserror::Error;

use super::chained_error::ChainedError;
use crate::ast::Operator;
use crate::engine::{Stack, StateWorkingSet};
use crate::{ConfigError, FromValue, LabeledError, ParseError, Span, Spanned, Type, Value, format_cli_error, record};

pub mod bridge;
pub mod io;
pub mod job;
pub mod location;
pub mod network;

include!("shell_error_kind.rs");
include!("shell_error_methods.rs");
include!("shell_error_convert.rs");

#[cfg(test)]
include!("shell_error_tests.rs");
