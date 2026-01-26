//! Background syntax scheduling + parsing.
//!
//! Goals:
//! - single-flight per document (never enqueue multiple parses per doc)
//! - global concurrency cap (avoid blocking-thread stampede)
//! - debounce (wait for quiet period)
//! - cooldown (back off after timeouts / repeated failures)
//! - tiered policy by file size (S/M/L) controlling timeout, injections, retention

use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

use futures::FutureExt;
use ropey::Rope;
use tokio::sync::{OwnedSemaphorePermit, Semaphore};
use tokio::task::JoinHandle;
use xeno_runtime_language::syntax::{InjectionPolicy, Syntax, SyntaxError, SyntaxOptions};
use xeno_runtime_language::{LanguageId, LanguageLoader};

use crate::buffer::DocumentId;

const DEFAULT_MAX_CONCURRENCY: usize = 2;

/// Parsing visibility / urgency.
///
/// The scheduler uses this to decide whether to keep trees around and whether
/// to run parses at all when the doc isn't currently rendered.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SyntaxHotness {
	/// Actively displayed (we need highlights now).
	Visible,
	/// Not visible but likely to become visible soon (e.g. split/tab MRU).
	Warm,
	/// Not visible; safe to drop heavy state.
	Cold,
}

/// File-size tier.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SyntaxTier {
	S,
	M,
	L,
}

/// Tier configuration.
#[derive(Debug, Clone, Copy)]
pub struct TierCfg {
	pub max_bytes_inclusive: Option<usize>,
	pub parse_timeout: Duration,
	pub debounce: Duration,
	pub cooldown_on_timeout: Duration,
	pub cooldown_on_error: Duration,
	pub injections: InjectionPolicy,
	pub retention_hidden: RetentionPolicy,
	pub parse_when_hidden: bool,
}

/// Syntax tree retention policy (memory control).
#[derive(Debug, Clone, Copy)]
pub enum RetentionPolicy {
	/// Never drop.
	Keep,
	/// Drop immediately once hidden (or cold).
	DropWhenHidden,
	/// Drop after a TTL since last Visible.
	DropAfter(Duration),
}

/// Tiered policy: compute tier from size -> cfg.
#[derive(Debug, Clone)]
pub struct TieredSyntaxPolicy {
	s: TierCfg,
	m: TierCfg,
	l: TierCfg,
}

impl Default for TieredSyntaxPolicy {
	fn default() -> Self {
		Self {
			s: TierCfg {
				max_bytes_inclusive: Some(256 * 1024),
				parse_timeout: Duration::from_millis(500),
				debounce: Duration::from_millis(80),
				cooldown_on_timeout: Duration::from_millis(400),
				cooldown_on_error: Duration::from_millis(150),
				injections: InjectionPolicy::Eager,
				retention_hidden: RetentionPolicy::Keep,
				parse_when_hidden: false,
			},
			m: TierCfg {
				max_bytes_inclusive: Some(1024 * 1024),
				parse_timeout: Duration::from_millis(1200),
				debounce: Duration::from_millis(140),
				cooldown_on_timeout: Duration::from_secs(2),
				cooldown_on_error: Duration::from_millis(250),
				injections: InjectionPolicy::Eager,
				retention_hidden: RetentionPolicy::DropAfter(Duration::from_secs(60)),
				parse_when_hidden: false,
			},
			l: TierCfg {
				max_bytes_inclusive: None,
				parse_timeout: Duration::from_secs(3),
				debounce: Duration::from_millis(250),
				cooldown_on_timeout: Duration::from_secs(10),
				cooldown_on_error: Duration::from_secs(2),
				injections: InjectionPolicy::Disabled, // biggest win: avoid injection layer explosion
				retention_hidden: RetentionPolicy::DropWhenHidden,
				parse_when_hidden: false,
			},
		}
	}
}

impl TieredSyntaxPolicy {
	pub fn tier_for_bytes(&self, bytes: usize) -> SyntaxTier {
		if bytes <= self.s.max_bytes_inclusive.unwrap() {
			SyntaxTier::S
		} else if bytes <= self.m.max_bytes_inclusive.unwrap() {
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
}

struct DocState {
	last_edit_at: Instant,
	last_visible_at: Instant,
	cooldown_until: Option<Instant>,
	inflight: Option<PendingSyntaxTask>,
}

impl DocState {
	fn new(now: Instant) -> Self {
		Self {
			last_edit_at: now,
			last_visible_at: now,
			cooldown_until: None,
			inflight: None,
		}
	}
}

struct PendingSyntaxTask {
	doc_version: u64,
	_started_at: Instant,
	task: JoinHandle<Result<Syntax, SyntaxError>>,
}

/// Result of polling syntax state.
#[derive(Debug, PartialEq, Eq)]
pub enum SyntaxPollResult {
	/// Syntax is ready.
	Ready,
	/// Parse is pending in background.
	Pending,
	/// Parse was kicked off.
	Kicked,
	/// No language configured for this document.
	NoLanguage,
	/// Cooldown active after timeout/error.
	CoolingDown,
	/// Background parsing disabled for this state (e.g. hidden large file).
	Disabled,
	/// Throttled by global concurrency cap.
	Throttled,
}

/// Background syntax scheduling + parsing.
pub struct SyntaxManager {
	policy: TieredSyntaxPolicy,
	permits: Arc<Semaphore>,
	docs: HashMap<DocumentId, DocState>,
}

impl Default for SyntaxManager {
	fn default() -> Self {
		Self::new(DEFAULT_MAX_CONCURRENCY)
	}
}

impl SyntaxManager {
	pub fn new(max_concurrency: usize) -> Self {
		Self {
			policy: TieredSyntaxPolicy::default(),
			permits: Arc::new(Semaphore::new(max_concurrency.max(1))),
			docs: HashMap::new(),
		}
	}

	pub fn set_policy(&mut self, policy: TieredSyntaxPolicy) {
		self.policy = policy;
	}

	/// Records an edit (for debounce). Do NOT abort inflight tasks (single-flight).
	pub fn note_edit(&mut self, doc_id: DocumentId) {
		let now = Instant::now();
		self.docs
			.entry(doc_id)
			.or_insert_with(|| DocState::new(now))
			.last_edit_at = now;
	}

	/// Cleans up tracking state for a closed document.
	pub fn on_document_close(&mut self, doc_id: DocumentId) {
		if let Some(mut st) = self.docs.remove(&doc_id)
			&& let Some(p) = st.inflight.take()
		{
			p.task.abort();
		}
	}

	pub fn has_pending(&self, doc_id: DocumentId) -> bool {
		self.docs
			.get(&doc_id)
			.and_then(|d| d.inflight.as_ref())
			.is_some()
	}

	pub fn pending_count(&self) -> usize {
		self.docs.values().filter(|d| d.inflight.is_some()).count()
	}

	/// Polls or kicks background syntax parsing.
	pub fn ensure_syntax(
		&mut self,
		doc_id: DocumentId,
		doc_version: u64,
		language_id: Option<LanguageId>,
		content: &Rope,
		current_syntax: &mut Option<Syntax>,
		syntax_dirty: &mut bool,
		hotness: SyntaxHotness,
		loader: &Arc<LanguageLoader>,
	) -> SyntaxPollResult {
		let Some(lang_id) = language_id else {
			return SyntaxPollResult::NoLanguage;
		};

		let now = Instant::now();
		let st = self
			.docs
			.entry(doc_id)
			.or_insert_with(|| DocState::new(now));

		if matches!(hotness, SyntaxHotness::Visible | SyntaxHotness::Warm) {
			st.last_visible_at = now;
		}

		let bytes = content.len_bytes();
		let tier = self.policy.tier_for_bytes(bytes);
		let cfg = self.policy.cfg(tier);

		// Memory retention: drop syntax when hidden/cold per tier config.
		apply_retention(
			now,
			st,
			cfg.retention_hidden,
			hotness,
			current_syntax,
			syntax_dirty,
		);

		if current_syntax.is_some() && !*syntax_dirty {
			return SyntaxPollResult::Ready;
		}

		// If hidden and this tier forbids background parsing, bail early.
		if !matches!(hotness, SyntaxHotness::Visible) && !cfg.parse_when_hidden {
			return SyntaxPollResult::Disabled;
		}

		// Poll inflight.
		if let Some(p) = st.inflight.as_mut() {
			let join = (&mut p.task).now_or_never();
			if join.is_none() {
				return SyntaxPollResult::Pending;
			}
			let done = st.inflight.take().expect("inflight present");
			let join = join.expect("checked ready");
			let done_version = done.doc_version;

			match join {
				Ok(Ok(syntax)) if done_version == doc_version => {
					*current_syntax = Some(syntax);
					*syntax_dirty = false;
					st.cooldown_until = None;
					return SyntaxPollResult::Ready;
				}
				Ok(Ok(_stale)) => {
					// Stale result (doc changed while parsing). Discard and continue to scheduling.
				}
				Ok(Err(SyntaxError::Timeout)) => {
					st.cooldown_until = Some(now + cfg.cooldown_on_timeout);
					// Keep dirty so we retry after cooldown.
					return SyntaxPollResult::CoolingDown;
				}
				Ok(Err(e)) => {
					tracing::warn!(doc_id=?doc_id, tier=?tier, error=%e, "Background syntax parse failed");
					st.cooldown_until = Some(now + cfg.cooldown_on_error);
					return SyntaxPollResult::CoolingDown;
				}
				Err(e) => {
					tracing::warn!(doc_id=?doc_id, tier=?tier, error=%e, "Background syntax task panicked");
					st.cooldown_until = Some(now + cfg.cooldown_on_error);
					return SyntaxPollResult::CoolingDown;
				}
			}
		}

		// Debounce: wait for quiet period.
		if now.duration_since(st.last_edit_at) < cfg.debounce {
			return SyntaxPollResult::Pending;
		}

		// Cooldown: backoff.
		if let Some(until) = st.cooldown_until {
			if now < until {
				return SyntaxPollResult::CoolingDown;
			}
			st.cooldown_until = None;
		}

		// Global concurrency cap.
		let permit = match Arc::clone(&self.permits).try_acquire_owned() {
			Ok(p) => p,
			Err(_) => return SyntaxPollResult::Throttled,
		};

		// Spawn parse.
		let loader = Arc::clone(loader);
		let content = content.clone();
		let opts = SyntaxOptions {
			parse_timeout: cfg.parse_timeout,
			injections: cfg.injections,
		};
		let task = tokio::task::spawn_blocking(move || {
			let _permit: OwnedSemaphorePermit = permit;
			Syntax::new(content.slice(..), lang_id, &loader, opts)
		});
		st.inflight = Some(PendingSyntaxTask {
			doc_version,
			_started_at: now,
			task,
		});
		SyntaxPollResult::Kicked
	}
}

fn apply_retention(
	now: Instant,
	st: &DocState,
	policy: RetentionPolicy,
	hotness: SyntaxHotness,
	current_syntax: &mut Option<Syntax>,
	syntax_dirty: &mut bool,
) {
	if matches!(hotness, SyntaxHotness::Visible | SyntaxHotness::Warm) {
		return;
	}

	match policy {
		RetentionPolicy::Keep => {}
		RetentionPolicy::DropWhenHidden => {
			if current_syntax.is_some() {
				*current_syntax = None;
				*syntax_dirty = true;
			}
		}
		RetentionPolicy::DropAfter(ttl) => {
			if current_syntax.is_some() && now.duration_since(st.last_visible_at) > ttl {
				*current_syntax = None;
				*syntax_dirty = true;
			}
		}
	}
}
