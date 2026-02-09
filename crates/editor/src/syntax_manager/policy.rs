use std::time::Duration;

use xeno_runtime_language::syntax::InjectionPolicy;

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
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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
	/// Maximum time allowed for a single parse operation.
	pub parse_timeout: Duration,
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
				parse_timeout: Duration::from_millis(500),
				debounce: Duration::from_millis(80),
				cooldown_on_timeout: Duration::from_millis(400),
				cooldown_on_error: Duration::from_millis(150),
				injections: InjectionPolicy::Eager,
				retention_hidden: RetentionPolicy::Keep,
				parse_when_hidden: false,
				sync_bootstrap_timeout: Some(Duration::from_millis(5)),
			},
			m: TierCfg {
				parse_timeout: Duration::from_millis(1200),
				debounce: Duration::from_millis(140),
				cooldown_on_timeout: Duration::from_secs(2),
				cooldown_on_error: Duration::from_millis(250),
				injections: InjectionPolicy::Eager,
				retention_hidden: RetentionPolicy::DropAfter(Duration::from_secs(60)),
				parse_when_hidden: false,
				sync_bootstrap_timeout: Some(Duration::from_millis(3)),
			},
			l: TierCfg {
				parse_timeout: Duration::from_secs(3),
				debounce: Duration::from_millis(250),
				cooldown_on_timeout: Duration::from_secs(10),
				cooldown_on_error: Duration::from_secs(2),
				injections: InjectionPolicy::Disabled,
				retention_hidden: RetentionPolicy::DropWhenHidden,
				parse_when_hidden: false,
				sync_bootstrap_timeout: None,
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
