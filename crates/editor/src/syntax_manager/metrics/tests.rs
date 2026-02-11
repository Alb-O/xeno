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
	metrics.record_task_result(lang, tier, class, injections, Duration::from_millis(100), false, false, true);
	let t1 = metrics.derive_timeout(lang, tier, class, injections, min, max);
	// 100 * 2.5 * 1.0 = 250ms
	assert_eq!(t1.as_millis(), 250);

	// Record a timeout
	metrics.record_task_result(lang, tier, class, injections, Duration::from_millis(250), true, false, false);
	let t2 = metrics.derive_timeout(lang, tier, class, injections, min, max);
	// EMA ms: 0.2 * 250 + 0.8 * 100 = 130
	// Timeout rate: 0.2 * 1.0 + 0.8 * 0.0 = 0.2
	// Scale: 1.0 + 0.2 * 2 = 1.4
	// Budget: 130 * 2.5 * 1.4 = 455ms
	assert_eq!(t2.as_millis(), 455);
}
