#![allow(clippy::byte_char_slices)]

use std::collections::{HashMap, HashSet};
use std::str;
use std::sync::Arc;

use itertools::Itertools;
use log::trace;
use xeno_nu_engine::DIR_VAR_PARSER_INFO;
use xeno_nu_protocol::ast::*;
use xeno_nu_protocol::casing::Casing;
use xeno_nu_protocol::engine::StateWorkingSet;
use xeno_nu_protocol::eval_const::eval_constant;
use xeno_nu_protocol::{
	BlockId, DeclId, DidYouMean, ENV_VARIABLE_ID, FilesizeUnit, Flag, IN_VARIABLE_ID, ParseError, PositionalArg, ShellError, Signature, Span, Spanned,
	SyntaxShape, Type, Value, VarId, did_you_mean,
};

use crate::lex::{LexState, is_assignment_operator, lex, lex_n_tokens, lex_signature};
use crate::lite_parser::{LiteCommand, LitePipeline, LiteRedirection, LiteRedirectionTarget, lite_parse};
use crate::parse_keywords::*;
use crate::parse_patterns::parse_pattern;
use crate::parse_shape_specs::{parse_completer, parse_shape_name, parse_type};
use crate::type_check::{self, check_range_types, math_result_type, type_compatible};
use crate::{Token, TokenContents};

include!("calls.rs");
include!("attributes.rs");
include!("primitives.rs");
include!("signatures.rs");
include!("expressions.rs");
include!("blocks.rs");
