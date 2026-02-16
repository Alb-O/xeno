use std::collections::HashMap;
use std::num::NonZeroUsize;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::mpsc::{Sender, channel};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

use fancy_regex::Regex;
use lru::LruCache;
use xeno_nu_path::AbsolutePathBuf;
use xeno_nu_utils::IgnoreCaseExt;

use crate::ast::Block;
use crate::debugger::{Debugger, NoopDebugger};
use crate::engine::description::{Doccomments, build_desc};
use crate::engine::{CachedFile, Command, DEFAULT_OVERLAY_NAME, EnvVars, OverlayFrame, ScopeFrame, Stack, StateDelta, Variable, Visibility};
use crate::eval_const::create_nu_constant;
use crate::report_error::ReportLog;
use crate::shell_error::io::IoError;
use crate::{
	BlockId, Config, DeclId, FileId, GetSpan, Handlers, HistoryConfig, JobId, Module, ModuleId, OverlayId, ShellError, SignalAction, Signals, Signature, Span,
	SpanId, Type, Value, VarId, VirtualPathId,
};

include!("types.rs");
include!("methods.rs");
include!("default_impl.rs");
