//! Adaptive syntax-task metrics.
//!
//! Tracks per-language/tier/task-class EMAs for duration and failure signals
//! and derives parse-timeout budgets from observed behavior.

use std::collections::HashMap;
use std::time::Duration;

use xeno_language::{InjectionPolicy, LanguageId};

use super::policy::SyntaxTier;
use super::tasks::TaskClass;

/// Smoothing factor for Exponential Moving Average.
/// alpha = 2 / (N + 1). For N=10, alpha ~= 0.18.
const EMA_ALPHA: f64 = 0.2;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
struct MetricsKey {
	lang_id: LanguageId,
	tier: SyntaxTier,
	class: TaskClass,
	injections: InjectionPolicy,
}

#[derive(Debug, Clone, Default)]
struct Ema {
	value: f64,
	initialized: bool,
}

impl Ema {
	fn update(&mut self, next: f64) {
		if self.initialized {
			self.value = EMA_ALPHA * next + (1.0 - EMA_ALPHA) * self.value;
		} else {
			self.value = next;
			self.initialized = true;
		}
	}
}

#[derive(Debug, Clone, Default)]
struct Entry {
	duration_ms: Ema,
	timeout_rate: Ema,
	error_rate: Ema,
	install_rate: Ema,
}

#[derive(Debug, Clone, Default)]
pub struct SyntaxMetrics {
	entries: HashMap<MetricsKey, Entry>,
}

impl SyntaxMetrics {
	pub fn new() -> Self {
		Self::default()
	}

	#[allow(clippy::too_many_arguments, reason = "metrics recording has a flat parameter list")]
	pub fn record_task_result(
		&mut self,
		lang_id: LanguageId,
		tier: SyntaxTier,
		class: TaskClass,
		injections: InjectionPolicy,
		elapsed: Duration,
		is_timeout: bool,
		is_error: bool,
		is_installed: bool,
	) {
		let key = MetricsKey {
			lang_id,
			tier,
			class,
			injections,
		};
		let entry = self.entries.entry(key).or_default();

		entry.duration_ms.update(elapsed.as_secs_f64() * 1000.0);
		entry.timeout_rate.update(if is_timeout { 1.0 } else { 0.0 });
		entry.error_rate.update(if is_error { 1.0 } else { 0.0 });
		entry.install_rate.update(if is_installed { 1.0 } else { 0.0 });
	}

	pub fn avg_duration(&self, lang_id: LanguageId, tier: SyntaxTier, class: TaskClass, injections: InjectionPolicy) -> Option<Duration> {
		let key = MetricsKey {
			lang_id,
			tier,
			class,
			injections,
		};
		self.entries.get(&key).map(|e| Duration::from_secs_f64(e.duration_ms.value / 1000.0))
	}

	pub fn timeout_rate(&self, lang_id: LanguageId, tier: SyntaxTier, class: TaskClass, injections: InjectionPolicy) -> f64 {
		let key = MetricsKey {
			lang_id,
			tier,
			class,
			injections,
		};
		self.entries.get(&key).map(|e| e.timeout_rate.value).unwrap_or(0.0)
	}

	/// Predicts the duration for a task without clamping to policy bounds.
	pub fn predict_duration(&self, lang_id: LanguageId, tier: SyntaxTier, class: TaskClass, injections: InjectionPolicy) -> Option<Duration> {
		let key = MetricsKey {
			lang_id,
			tier,
			class,
			injections,
		};

		self.entries.get(&key).map(|entry| {
			let ema_ms = entry.duration_ms.value;
			let timeout_scale = 1.0 + (entry.timeout_rate.value * 2.0);
			let budget_ms = ema_ms * 2.5 * timeout_scale;
			Duration::from_secs_f64(budget_ms / 1000.0)
		})
	}

	pub fn derive_timeout(
		&self,
		lang_id: LanguageId,
		tier: SyntaxTier,
		class: TaskClass,
		injections: InjectionPolicy,
		min: Duration,
		max: Duration,
	) -> Duration {
		let key = MetricsKey {
			lang_id,
			tier,
			class,
			injections,
		};

		if let Some(entry) = self.entries.get(&key) {
			let ema_ms = entry.duration_ms.value;
			// Budget = 2.5x EMA, but scaled up if timeout rate is high
			let timeout_scale = 1.0 + (entry.timeout_rate.value * 2.0);
			let budget_ms = ema_ms * 2.5 * timeout_scale;

			Duration::from_secs_f64(budget_ms / 1000.0).clamp(min, max)
		} else {
			max
		}
	}
}

#[cfg(test)]
mod tests;
