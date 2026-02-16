use std::borrow::Cow;
use std::fs::File;
use std::sync::Arc;

use xeno_nu_path::{expand_path, expand_path_with};
use xeno_nu_protocol::ast::{Bits, Block, Boolean, CellPath, Comparison, Math, Operator};
use xeno_nu_protocol::debugger::DebugContext;
use xeno_nu_protocol::engine::{Argument, Closure, EngineState, ErrorHandler, Matcher, Redirection, Stack, StateWorkingSet};
use xeno_nu_protocol::ir::{Call, DataSlice, Instruction, IrAstRef, IrBlock, Literal, RedirectMode};
use xeno_nu_protocol::shell_error::io::IoError;
use xeno_nu_protocol::{
	DeclId, ENV_VARIABLE_ID, Flag, IntoPipelineData, IntoSpanned, ListStream, OutDest, PipelineData, PipelineExecutionData, PositionalArg, Range, Record,
	RegId, ShellError, Signals, Signature, Span, Spanned, Type, Value, VarId, combined_type_string,
};
use xeno_nu_utils::IgnoreCaseExt;

use crate::eval::is_automatic_env_var;
use crate::{ENV_CONVERSIONS, convert_env_vars, eval_block_with_early_return};

include!("context.rs");
include!("dispatch.rs");
include!("literals.rs");
include!("calls.rs");
include!("io_ops.rs");
