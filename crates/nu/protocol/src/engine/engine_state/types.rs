type PoisonDebuggerError<'a> = PoisonError<MutexGuard<'a, Box<dyn Debugger>>>;

use super::{CurrentJob, Jobs, Mail, Mailbox, ThreadJob};

#[derive(Clone, Debug)]
pub enum VirtualPath {
	File(FileId),
	Dir(Vec<VirtualPathId>),
}

pub struct ReplState {
	pub buffer: String,
	// A byte position, as `EditCommand::MoveToPosition` is also a byte position
	pub cursor_pos: usize,
	/// Immediately accept the buffer on the next loop.
	pub accept: bool,
}

pub struct IsDebugging(AtomicBool);

impl IsDebugging {
	pub fn new(val: bool) -> Self {
		IsDebugging(AtomicBool::new(val))
	}
}

impl Clone for IsDebugging {
	fn clone(&self) -> Self {
		IsDebugging(AtomicBool::new(self.0.load(Ordering::Relaxed)))
	}
}

/// The core global engine state. This includes all global definitions as well as any global state that
/// will persist for the whole session.
///
/// Declarations, variables, blocks, and other forms of data are held in the global state and referenced
/// elsewhere using their IDs. These IDs are simply their index into the global state. This allows us to
/// more easily handle creating blocks, binding variables and callsites, and more, because each of these
/// will refer to the corresponding IDs rather than their definitions directly. At runtime, this means
/// less copying and smaller structures.
///
/// Many of the larger objects in this structure are stored within `Arc` to decrease the cost of
/// cloning `EngineState`. While `Arc`s are generally immutable, they can be modified using
/// `Arc::make_mut`, which automatically clones to a new allocation if there are other copies of
/// the `Arc` already in use, but will let us modify the `Arc` directly if we have the only
/// reference to it.
///
/// Note that the runtime stack is not part of this global state. Runtime stacks are handled differently,
/// but they also rely on using IDs rather than full definitions.
#[derive(Clone)]
pub struct EngineState {
	files: Vec<CachedFile>,
	pub(super) virtual_paths: Vec<(String, VirtualPath)>,
	vars: Vec<Variable>,
	decls: Arc<Vec<Box<dyn Command + 'static>>>,
	// The Vec is wrapped in Arc so that if we don't need to modify the list, we can just clone
	// the reference and not have to clone each individual Arc inside. These lists can be
	// especially long, so it helps
	pub(super) blocks: Arc<Vec<Arc<Block>>>,
	pub(super) modules: Arc<Vec<Arc<Module>>>,
	pub spans: Vec<Span>,
	doccomments: Doccomments,
	pub scope: ScopeFrame,
	signals: Signals,
	pub signal_handlers: Option<Handlers>,
	pub env_vars: Arc<EnvVars>,
	pub previous_env_vars: Arc<HashMap<String, Value>>,
	pub config: Arc<Config>,
	pub pipeline_externals_state: Arc<(AtomicU32, AtomicU32)>,
	pub repl_state: Arc<Mutex<ReplState>>,
	pub table_decl_id: Option<DeclId>,
	config_path: HashMap<String, PathBuf>,
	pub history_enabled: bool,
	pub history_session_id: i64,
	// Path to the file Nushell is currently evaluating, or None if we're in an interactive session.
	pub file: Option<PathBuf>,
	pub regex_cache: Arc<Mutex<LruCache<String, Regex>>>,
	pub is_interactive: bool,
	pub is_login: bool,
	pub is_lsp: bool,
	pub is_mcp: bool,
	startup_time: i64,
	is_debugging: IsDebugging,
	pub debugger: Arc<Mutex<Box<dyn Debugger>>>,
	pub report_log: Arc<Mutex<ReportLog>>,

	pub jobs: Arc<Mutex<Jobs>>,

	// The job being executed with this engine state, or None if main thread
	pub current_job: CurrentJob,

	pub root_job_sender: Sender<Mail>,

	// When there are background jobs running, the interactive behavior of `exit` changes depending on
	// the value of this flag:
	// - if this is false, then a warning about running jobs is shown and `exit` enables this flag
	// - if this is true, then `exit` will `std::process::exit`
	//
	// This ensures that running exit twice will terminate the program correctly
	pub exit_warning_given: Arc<AtomicBool>,
}

// The max number of compiled regexes to keep around in a LRU cache, arbitrarily chosen
const REGEX_CACHE_SIZE: usize = 100; // must be nonzero, otherwise will panic

pub const NU_VARIABLE_ID: VarId = VarId::new(0);
pub const IN_VARIABLE_ID: VarId = VarId::new(1);
pub const ENV_VARIABLE_ID: VarId = VarId::new(2);
// NOTE: If you add more to this list, make sure to update the > checks based on the last in the list

// The first span is unknown span
pub const UNKNOWN_SPAN_ID: SpanId = SpanId::new(0);
