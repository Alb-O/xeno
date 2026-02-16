use core::panic;
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use crate::ast::Block;
use crate::engine::description::build_desc;
use crate::engine::{CachedFile, Command, CommandType, EngineState, OverlayFrame, StateDelta, Variable, VirtualPath, Visibility};
use crate::{
	BlockId, Category, CompileError, Config, DeclId, FileId, GetSpan, Module, ModuleId, OverlayId, ParseError, ParseWarning, ResolvedImportPattern, Signature,
	Span, SpanId, Type, Value, VarId, VirtualPathId,
};

/// A temporary extension to the global state. This handles bridging between the global state and the
/// additional declarations and scope changes that are not yet part of the global scope.
///
/// This working set is created by the parser as a way of handling declarations and scope changes that
/// may later be merged or dropped (and not merged) depending on the needs of the code calling the parser.
pub struct StateWorkingSet<'a> {
	pub permanent_state: &'a EngineState,
	pub delta: StateDelta,
	pub files: FileStack,
	/// Whether or not predeclarations are searched when looking up a command (used with aliases)
	pub search_predecls: bool,
	pub parse_errors: Vec<ParseError>,
	pub parse_warnings: Vec<ParseWarning>,
	pub compile_errors: Vec<CompileError>,
}

include!("state_methods.rs");
include!("file_stack.rs");
