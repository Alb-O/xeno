use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use log::trace;
use xeno_nu_path::{absolute_with, is_windows_device_path};
use xeno_nu_protocol::ast::{
	Argument, AttributeBlock, Block, Call, Expr, Expression, ImportPattern, ImportPatternHead, ImportPatternMember, Pipeline, PipelineElement,
};
use xeno_nu_protocol::engine::{DEFAULT_OVERLAY_NAME, StateWorkingSet};
use xeno_nu_protocol::eval_const::eval_constant;
use xeno_nu_protocol::parser_path::ParserPath;
use xeno_nu_protocol::{
	Alias, BlockId, CommandWideCompleter, CustomExample, DeclId, FromValue, Module, ModuleId, ParseError, PositionalArg, ResolvedImportPattern, ShellError,
	Signature, Span, Spanned, SyntaxShape, Type, Value, VarId, category_from_string,
};

use crate::exportable::Exportable;
use crate::parse_block;
use crate::parser::{ArgumentParsingLevel, CallKind, compile_block, compile_block_with_id, parse_attribute, parse_redirection, redirecting_builtin_error};
use crate::type_check::{check_block_input_output, type_compatible};

include!("definitions.rs");
include!("modules.rs");
include!("overlay_bindings.rs");
