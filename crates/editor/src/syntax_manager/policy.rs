use std::time::Duration;

use xeno_language::syntax::InjectionPolicy;

/// Visibility and urgency of a document for the syntax scheduler.
///
/// Hotness determines the priority of background parsing tasks and the aggressiveness
/// of syntax tree retention.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SyntaxHotness {
	/// Actively displayed in a window.
	///
	/// Parsing is high priority and results are always installed.
	Visible,
	/// Not currently visible but likely to become so soon (e.g., recently closed split).
	///
	/// Parsing is allowed but lower priority.
	Warm,
	/// Not visible and not in recent use.
	///
	/// Safe to drop heavy syntax state to save memory.
	Cold,
}

/// Size-based tier for a file.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SyntaxTier {
	/// Small file.
	S,
	/// Medium file.
	M,
	/// Large file.
	L,
}

/// Configuration for a specific [`SyntaxTier`].
#[derive(Debug, Clone, Copy)]
pub struct TierCfg {
	/// Minimum time allowed for a single parse operation.
	pub parse_timeout_min: Duration,
	/// Maximum time allowed for a single parse operation.
	pub parse_timeout_max: Duration,
	/// Time to wait after an edit before triggering a background parse.
	pub debounce: Duration,
	/// Backoff duration after a parse timeout.
	pub cooldown_on_timeout: Duration,
	/// Backoff duration after a parse error.
	pub cooldown_on_error: Duration,
	/// Injection handling policy.
	pub injections: InjectionPolicy,
	/// Retention policy for hidden documents.
	pub retention_hidden: RetentionPolicy,
	/// Whether to allow background parsing when the document is not visible.
	pub parse_when_hidden: bool,
	/// Timeout for the synchronous bootstrap parse attempt on the render
	/// thread. `None` disables the fast path for this tier.
	pub sync_bootstrap_timeout: Option<Duration>,

	/// Byte lookbehind for viewport-bounded parsing.
	pub viewport_lookbehind: u32,
	/// Byte lookahead for viewport-bounded parsing.
	pub viewport_lookahead: u32,
	/// Maximum window size for viewport-bounded parsing.
	pub viewport_window_max: u32,
	/// Minimum timeout for viewport-bounded parsing.
	pub viewport_parse_timeout_min: Duration,
	/// Maximum timeout for viewport-bounded parsing.
	pub viewport_parse_timeout_max: Duration,
	/// Injection policy for viewport-bounded parsing.
	pub viewport_injections: InjectionPolicy,
	/// Budget for Stage B viewport-bounded parsing (with injections).
	/// `None` disables Stage B.
	pub viewport_stage_b_budget: Option<Duration>,
	/// Maximum visible viewport byte span consumed by scheduler decisions.
	///
	/// Guards against pathological long-line viewports that would otherwise
	/// appear as near-file-wide byte ranges.
	pub viewport_visible_span_cap: u32,
	/// Backoff duration after a viewport parse timeout.
	pub viewport_cooldown_on_timeout: Duration,
	/// Backoff duration after a viewport parse error.
	pub viewport_cooldown_on_error: Duration,
}

/// Syntax tree retention policy for memory management.
#[derive(Debug, Clone, Copy)]
pub enum RetentionPolicy {
	/// Never drop the syntax tree.
	Keep,
	/// Drop the syntax tree immediately once the document is hidden.
	DropWhenHidden,
	/// Drop the syntax tree after a TTL since the document was last visible.
	DropAfter(Duration),
}

/// Tiered syntax policy that maps file size to specific configurations.
#[derive(Debug, Clone)]
pub struct TieredSyntaxPolicy {
	/// Threshold for the small (S) tier.
	pub s_max_bytes_inclusive: usize,
	/// Threshold for the medium (M) tier.
	pub m_max_bytes_inclusive: usize,
	/// Configuration for small files.
	pub s: TierCfg,
	/// Configuration for medium files.
	pub m: TierCfg,
	/// Configuration for large files.
	pub l: TierCfg,
}

impl Default for TieredSyntaxPolicy {
	fn default() -> Self {
		Self {
			s_max_bytes_inclusive: 256 * 1024,
			m_max_bytes_inclusive: 1024 * 1024,
			s: TierCfg {
				parse_timeout_min: Duration::from_millis(50),
				parse_timeout_max: Duration::from_millis(500),
				debounce: Duration::from_millis(80),
				cooldown_on_timeout: Duration::from_millis(400),
				cooldown_on_error: Duration::from_millis(150),
				injections: InjectionPolicy::Eager,
				retention_hidden: RetentionPolicy::Keep,
				parse_when_hidden: false,
				sync_bootstrap_timeout: Some(Duration::from_millis(5)),
				viewport_lookbehind: 8192,
				viewport_lookahead: 8192,
				viewport_window_max: 128 * 1024,
				viewport_parse_timeout_min: Duration::from_millis(5),
				viewport_parse_timeout_max: Duration::from_millis(15),
				viewport_injections: InjectionPolicy::Disabled,
				viewport_stage_b_budget: Some(Duration::from_millis(25)),
				viewport_visible_span_cap: 64 * 1024,
				viewport_cooldown_on_timeout: Duration::from_millis(250),
				viewport_cooldown_on_error: Duration::from_millis(100),
			},
			m: TierCfg {
				parse_timeout_min: Duration::from_millis(100),
				parse_timeout_max: Duration::from_millis(1200),
				debounce: Duration::from_millis(140),
				cooldown_on_timeout: Duration::from_secs(2),
				cooldown_on_error: Duration::from_millis(250),
				injections: InjectionPolicy::Eager,
				retention_hidden: RetentionPolicy::DropAfter(Duration::from_secs(60)),
				parse_when_hidden: false,
				sync_bootstrap_timeout: Some(Duration::from_millis(3)),
				viewport_lookbehind: 8192,
				viewport_lookahead: 8192,
				viewport_window_max: 128 * 1024,
				viewport_parse_timeout_min: Duration::from_millis(5),
				viewport_parse_timeout_max: Duration::from_millis(15),
				viewport_injections: InjectionPolicy::Disabled,
				viewport_stage_b_budget: Some(Duration::from_millis(25)),
				viewport_visible_span_cap: 64 * 1024,
				viewport_cooldown_on_timeout: Duration::from_millis(300),
				viewport_cooldown_on_error: Duration::from_millis(120),
			},
			l: TierCfg {
				parse_timeout_min: Duration::from_millis(250),
				parse_timeout_max: Duration::from_secs(3),
				debounce: Duration::from_millis(250),
				cooldown_on_timeout: Duration::from_secs(10),
				cooldown_on_error: Duration::from_secs(2),
				injections: InjectionPolicy::Disabled,
				retention_hidden: RetentionPolicy::DropWhenHidden,
				parse_when_hidden: false,
				sync_bootstrap_timeout: None,
				viewport_lookbehind: 8192,
				viewport_lookahead: 8192,
				viewport_window_max: 128 * 1024,
				viewport_parse_timeout_min: Duration::from_millis(5),
				viewport_parse_timeout_max: Duration::from_millis(15),
				viewport_injections: InjectionPolicy::Disabled,
				viewport_stage_b_budget: Some(Duration::from_millis(45)),
				viewport_visible_span_cap: 96 * 1024,
				viewport_cooldown_on_timeout: Duration::from_millis(500),
				viewport_cooldown_on_error: Duration::from_millis(200),
			},
		}
	}
}

impl TieredSyntaxPolicy {
	pub fn tier_for_bytes(&self, bytes: usize) -> SyntaxTier {
		if bytes <= self.s_max_bytes_inclusive {
			SyntaxTier::S
		} else if bytes <= self.m_max_bytes_inclusive {
			SyntaxTier::M
		} else {
			SyntaxTier::L
		}
	}

	pub fn cfg(&self, tier: SyntaxTier) -> TierCfg {
		match tier {
			SyntaxTier::S => self.s,
			SyntaxTier::M => self.m,
			SyntaxTier::L => self.l,
		}
	}

	#[cfg(any(test, doc))]
	pub fn test_default() -> Self {
		let mut p = Self::default();
		p.s.sync_bootstrap_timeout = None;
		p.m.sync_bootstrap_timeout = None;
		p.l.sync_bootstrap_timeout = None;
		p
	}
}

/// Global configuration for the [`SyntaxManager`].
#[derive(Debug, Clone)]
pub struct SyntaxManagerCfg {
	/// Maximum concurrent background parse tasks.
	pub max_concurrency: usize,
	/// Number of permits to reserve exclusively for viewport-bounded tasks.
	///
	/// If `max_concurrency` is 4 and `viewport_reserve` is 1, then at most 3
	/// full/incremental parses can run concurrently, leaving 1 permit for
	/// immediate viewport parsing.
	pub viewport_reserve: usize,
}

impl Default for SyntaxManagerCfg {
	fn default() -> Self {
		Self {
			max_concurrency: 2,
			viewport_reserve: 1,
		}
	}
}
