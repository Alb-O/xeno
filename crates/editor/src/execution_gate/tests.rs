use std::time::Duration;

use super::*;

#[tokio::test]
async fn test_gate_blocks_background() {
	let gate = ExecutionGate::new();

	let guard = gate.enter_interactive();

	let gate_clone = gate.clone();
	let background = tokio::spawn(async move {
		gate_clone.wait_for_background().await;
	});

	// Background should be blocked
	tokio::time::sleep(Duration::from_millis(50)).await;
	assert!(!background.is_finished());

	// Drop guard, background should proceed
	drop(guard);
	tokio::time::timeout(Duration::from_millis(50), background).await.unwrap().unwrap();
}

#[tokio::test]
async fn test_gate_open_scope_overrides_interactive() {
	let gate = ExecutionGate::new();
	let _guard = gate.enter_interactive();

	let gate_clone = gate.clone();
	let background = tokio::spawn(async move {
		gate_clone.wait_for_background().await;
	});

	tokio::time::sleep(Duration::from_millis(50)).await;
	assert!(!background.is_finished());

	let _scope = gate.open_background_scope();
	tokio::time::timeout(Duration::from_millis(50), background).await.unwrap().unwrap();
}

#[tokio::test]
async fn test_gate_nested_scopes() {
	let gate = ExecutionGate::new();
	let _guard = gate.enter_interactive();

	let _scope1 = gate.open_background_scope();
	let _scope2 = gate.open_background_scope();

	assert!(gate.background_open_depth.load(Ordering::SeqCst) == 2);

	drop(_scope2);
	assert!(gate.background_open_depth.load(Ordering::SeqCst) == 1);

	// Still open
	let gate_clone = gate.clone();
	let background = tokio::spawn(async move {
		gate_clone.wait_for_background().await;
	});
	tokio::time::timeout(Duration::from_millis(50), background).await.unwrap().unwrap();

	drop(_scope1);
	assert!(gate.background_open_depth.load(Ordering::SeqCst) == 0);
}
