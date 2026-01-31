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

/// Key for checking if parse options have changed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct OptKey {
	injections: InjectionPolicy,
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
	lang_id: LanguageId,
	opts: OptKey,
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

/// Abstract engine for parsing syntax (for test mockability).
pub trait SyntaxEngine: Send + Sync {
	fn parse(
		&self,
		content: ropey::RopeSlice<'_>,
		lang: LanguageId,
		loader: &LanguageLoader,
		opts: SyntaxOptions,
	) -> Result<Syntax, SyntaxError>;
}

struct RealSyntaxEngine;
impl SyntaxEngine for RealSyntaxEngine {
	fn parse(
		&self,
		content: ropey::RopeSlice<'_>,
		lang: LanguageId,
		loader: &LanguageLoader,
		opts: SyntaxOptions,
	) -> Result<Syntax, SyntaxError> {
		Syntax::new(content, lang, loader, opts)
	}
}

/// Background syntax scheduling + parsing.
pub struct SyntaxManager {
	policy: TieredSyntaxPolicy,
	permits: Arc<Semaphore>,
	docs: HashMap<DocumentId, DocState>,
	engine: Arc<dyn SyntaxEngine>,
}

impl Default for SyntaxManager {
	fn default() -> Self {
		Self::new(DEFAULT_MAX_CONCURRENCY)
	}
}

pub struct EnsureSyntaxContext<'a> {
	pub doc_id: DocumentId,
	pub doc_version: u64,
	pub language_id: Option<LanguageId>,
	pub content: &'a Rope,
	pub hotness: SyntaxHotness,
	pub loader: &'a Arc<LanguageLoader>,
}

pub struct SyntaxSlot<'a> {
	pub current: &'a mut Option<Syntax>,
	pub dirty: &'a mut bool,
	pub updated: &'a mut bool,
}

impl SyntaxManager {
	pub fn new(max_concurrency: usize) -> Self {
		Self {
			policy: TieredSyntaxPolicy::default(),
			permits: Arc::new(Semaphore::new(max_concurrency.max(1))),
			docs: HashMap::new(),
			engine: Arc::new(RealSyntaxEngine),
		}
	}

	#[cfg(test)]
	pub fn new_with_engine(max_concurrency: usize, engine: Arc<dyn SyntaxEngine>) -> Self {
		Self {
			policy: TieredSyntaxPolicy::default(),
			permits: Arc::new(Semaphore::new(max_concurrency.max(1))),
			docs: HashMap::new(),
			engine,
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
	///
	/// This is the main entry point for the syntax scheduler. It:
	/// 1. Polling/draining inflight tasks (immortal task prevention).
	/// 2. Validating results against LanguageId and OptKey.
	/// 3. Respecting retention policy at completion time.
	/// 4. Handling debounce and backoff (cooldown) timers.
	/// 5. Spawning new background parse tasks if permits are available.
	pub fn ensure_syntax(
		&mut self,
		ctx: EnsureSyntaxContext<'_>,
		slot: SyntaxSlot<'_>,
	) -> SyntaxPollResult {
		*slot.updated = false;

		let now = Instant::now();
		let st = self
			.docs
			.entry(ctx.doc_id)
			.or_insert_with(|| DocState::new(now));

		if matches!(ctx.hotness, SyntaxHotness::Visible | SyntaxHotness::Warm) {
			st.last_visible_at = now;
		}

		let bytes = ctx.content.len_bytes();
		let tier = self.policy.tier_for_bytes(bytes);
		let cfg = self.policy.cfg(tier);
		let current_opts_key = OptKey {
			injections: cfg.injections,
		};

		// 1) Poll/drain inflight FIRST (fixes “immortal inflight”)
		if let Some(p) = st.inflight.as_mut() {
			let join = xeno_primitives::future::poll_once(&mut p.task);
			if join.is_none() {
				// still running; do NOT short-circuit to Ready/Disabled
				return SyntaxPollResult::Pending;
			}

			let done = st.inflight.take().expect("inflight present");
			match join.expect("checked ready") {
				Ok(Ok(syntax)) => {
					// language gating: only install if language still matches
					let Some(current_lang) = ctx.language_id else {
						// language removed mid-flight: discard result
						return SyntaxPollResult::NoLanguage;
					};

					let lang_ok = done.lang_id == current_lang;
					let opts_ok = done.opts == current_opts_key;
					let version_match = done.doc_version == ctx.doc_version;
					let retain_ok =
						retention_allows_install(now, st, cfg.retention_hidden, ctx.hotness);

					let allow_install = should_install_completed_parse(
						version_match,
						*slot.dirty,
						slot.current.is_some(),
					);

					if lang_ok && retain_ok && allow_install {
						*slot.current = Some(syntax);
						*slot.updated = true;
					}

					// dirty clears only if this parse matches current version + "shape"
					if lang_ok && opts_ok && version_match {
						*slot.dirty = false;
						st.cooldown_until = None;
						return SyntaxPollResult::Ready;
					}
					// else: keep dirty true (caller keeps chasing newest)
				}
				Ok(Err(SyntaxError::Timeout)) => {
					st.cooldown_until = Some(now + cfg.cooldown_on_timeout);
					return SyntaxPollResult::CoolingDown;
				}
				Ok(Err(e)) => {
					tracing::warn!(doc_id=?ctx.doc_id, tier=?tier, error=%e, "Background syntax parse failed");
					st.cooldown_until = Some(now + cfg.cooldown_on_error);
					return SyntaxPollResult::CoolingDown;
				}
				Err(e) => {
					tracing::warn!(doc_id=?ctx.doc_id, tier=?tier, error=%e, "Background syntax task panicked");
					st.cooldown_until = Some(now + cfg.cooldown_on_error);
					return SyntaxPollResult::CoolingDown;
				}
			}
		}

		// 2) Handle “no language” AFTER draining inflight to avoid leaks
		let Some(lang_id) = ctx.language_id else {
			if slot.current.is_some() {
				*slot.current = None;
				*slot.updated = true;
			}
			*slot.dirty = false;
			st.cooldown_until = None;
			return SyntaxPollResult::NoLanguage;
		};

		// 3) Retention AFTER inflight completion (don’t re-install dropped trees)
		apply_retention(
			now,
			st,
			cfg.retention_hidden,
			ctx.hotness,
			slot.current,
			slot.dirty,
			slot.updated,
		);

		// 4) Clean short-circuit (safe now: no inflight exists)
		if slot.current.is_some() && !*slot.dirty {
			return SyntaxPollResult::Ready;
		}

		// 5) Hidden policy check (safe now: inflight already drained)
		if !matches!(ctx.hotness, SyntaxHotness::Visible) && !cfg.parse_when_hidden {
			return SyntaxPollResult::Disabled;
		}

		// 6) Debounce (skip for bootstrap: no existing tree → parse immediately)
		if slot.current.is_some() && now.duration_since(st.last_edit_at) < cfg.debounce {
			return SyntaxPollResult::Pending;
		}

		// 7) Cooldown
		if let Some(until) = st.cooldown_until {
			if now < until {
				return SyntaxPollResult::CoolingDown;
			}
			st.cooldown_until = None;
		}

		// 8) Concurrency cap
		let permit = match Arc::clone(&self.permits).try_acquire_owned() {
			Ok(p) => p,
			Err(_) => return SyntaxPollResult::Throttled,
		};

		// 9) Spawn
		let loader = Arc::clone(ctx.loader);
		let content = ctx.content.clone();
		let opts = SyntaxOptions {
			parse_timeout: cfg.parse_timeout,
			injections: cfg.injections,
		};
		let engine = Arc::clone(&self.engine);
		let task = tokio::task::spawn_blocking(move || {
			let _permit: OwnedSemaphorePermit = permit;
			engine.parse(content.slice(..), lang_id, &loader, opts)
		});

		st.inflight = Some(PendingSyntaxTask {
			doc_version: ctx.doc_version,
			lang_id,
			opts: current_opts_key,
			_started_at: now,
			task,
		});

		SyntaxPollResult::Kicked
	}
}

/// Whether a completed background parse should be installed into the syntax slot.
///
/// Guards against overwriting a newer incremental tree with a stale full-parse
/// result. A stale parse (version mismatch) is only installed when the caller is
/// already dirty (catch-up mode) or has no syntax at all (bootstrap).
fn should_install_completed_parse(
	version_match: bool,
	slot_dirty: bool,
	has_current: bool,
) -> bool {
	version_match || slot_dirty || !has_current
}

fn retention_allows_install(
	now: Instant,
	st: &DocState,
	policy: RetentionPolicy,
	hotness: SyntaxHotness,
) -> bool {
	if matches!(hotness, SyntaxHotness::Visible | SyntaxHotness::Warm) {
		return true;
	}
	match policy {
		RetentionPolicy::Keep => true,
		RetentionPolicy::DropWhenHidden => false,
		RetentionPolicy::DropAfter(ttl) => now.duration_since(st.last_visible_at) <= ttl,
	}
}

fn apply_retention(
	now: Instant,
	st: &DocState,
	policy: RetentionPolicy,
	hotness: SyntaxHotness,
	current_syntax: &mut Option<Syntax>,
	syntax_dirty: &mut bool,
	updated: &mut bool,
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
				*updated = true;
			}
		}
		RetentionPolicy::DropAfter(ttl) => {
			if current_syntax.is_some() && now.duration_since(st.last_visible_at) > ttl {
				*current_syntax = None;
				*syntax_dirty = true;
				*updated = true;
			}
		}
	}
}

#[cfg(test)]
mod tests;
