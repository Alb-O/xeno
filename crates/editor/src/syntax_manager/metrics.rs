use std::collections::HashMap;
use std::time::Duration;

use xeno_runtime_language::LanguageId;
use xeno_runtime_language::syntax::InjectionPolicy;

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
struct EMA {
	value: f64,
	initialized: bool,
}

impl EMA {
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
	duration_ms: EMA,
	timeout_rate: EMA,
	error_rate: EMA,
	install_rate: EMA,
}

#[derive(Debug, Clone, Default)]
pub struct SyntaxMetrics {
	entries: HashMap<MetricsKey, Entry>,
}

impl SyntaxMetrics {
	pub fn new() -> Self {
		Self::default()
	}

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
		entry
			.timeout_rate
			.update(if is_timeout { 1.0 } else { 0.0 });
		entry.error_rate.update(if is_error { 1.0 } else { 0.0 });
		entry
			.install_rate
			.update(if is_installed { 1.0 } else { 0.0 });
	}

	pub fn avg_duration(
		&self,
		lang_id: LanguageId,
		tier: SyntaxTier,
		class: TaskClass,
		injections: InjectionPolicy,
	) -> Option<Duration> {
		let key = MetricsKey {
			lang_id,
			tier,
			class,
			injections,
		};
		self.entries
			.get(&key)
			.map(|e| Duration::from_secs_f64(e.duration_ms.value / 1000.0))
	}

	pub fn timeout_rate(
		&self,
		lang_id: LanguageId,
		tier: SyntaxTier,
		class: TaskClass,
		injections: InjectionPolicy,
	) -> f64 {
		let key = MetricsKey {
			lang_id,
			tier,
			class,
			injections,
		};
		self.entries
			.get(&key)
			.map(|e| e.timeout_rate.value)
			.unwrap_or(0.0)
	}

	/// Predicts the duration for a task without clamping to policy bounds.
	pub fn predict_duration(
		&self,
		lang_id: LanguageId,
		tier: SyntaxTier,
		class: TaskClass,
		injections: InjectionPolicy,
	) -> Option<Duration> {
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
mod tests {
	use super::*;

	#[test]
	fn test_ema_update() {
		let mut ema = EMA::default();
		ema.update(100.0);
		assert_eq!(ema.value, 100.0);
		ema.update(200.0);
		// 0.2 * 200 + 0.8 * 100 = 40 + 80 = 120
		assert_eq!(ema.value, 120.0);
	}

	#[test]
	fn test_derive_timeout_scales_with_timeouts() {
		let mut metrics = SyntaxMetrics::new();
		let lang = LanguageId::new(1);
		let tier = SyntaxTier::S;
		let class = TaskClass::Full;
		let injections = InjectionPolicy::Eager;
		let min = Duration::from_millis(10);
		let max = Duration::from_millis(1000);

		// Record a fast parse
		metrics.record_task_result(
			lang,
			tier,
			class,
			injections,
			Duration::from_millis(100),
			false,
			false,
			true,
		);
		let t1 = metrics.derive_timeout(lang, tier, class, injections, min, max);
		// 100 * 2.5 * 1.0 = 250ms
		assert_eq!(t1.as_millis(), 250);

		// Record a timeout
		metrics.record_task_result(
			lang,
			tier,
			class,
			injections,
			Duration::from_millis(250),
			true,
			false,
			false,
		);
		let t2 = metrics.derive_timeout(lang, tier, class, injections, min, max);
		// EMA ms: 0.2 * 250 + 0.8 * 100 = 130
		// Timeout rate: 0.2 * 1.0 + 0.8 * 0.0 = 0.2
		// Scale: 1.0 + 0.2 * 2 = 1.4
		// Budget: 130 * 2.5 * 1.4 = 455ms
		assert_eq!(t2.as_millis(), 455);
	}
}
