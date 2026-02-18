use super::join_error_panic_message;

#[tokio::test]
async fn extracts_static_str_payload() {
	let handle = tokio::spawn(async { panic!("boom-str") });
	let err = handle.await.unwrap_err();
	let msg = join_error_panic_message(err).expect("should be a panic");
	assert!(msg.contains("boom-str"), "expected 'boom-str', got: {msg}");
}

#[tokio::test]
async fn extracts_string_payload() {
	let handle = tokio::spawn(async { panic!("{}", String::from("boom-string")) });
	let err = handle.await.unwrap_err();
	let msg = join_error_panic_message(err).expect("should be a panic");
	assert!(msg.contains("boom-string"), "expected 'boom-string', got: {msg}");
}

#[tokio::test]
async fn returns_none_for_cancellation() {
	let handle = tokio::spawn(async {
		tokio::time::sleep(std::time::Duration::from_secs(60)).await;
	});
	handle.abort();
	let err = handle.await.unwrap_err();
	assert!(join_error_panic_message(err).is_none(), "cancelled task should return None");
}
