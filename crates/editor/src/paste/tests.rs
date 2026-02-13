use super::normalize_to_lf;

#[test]
fn crlf_to_lf() {
	assert_eq!(normalize_to_lf("a\r\nb\r\n".to_string()), "a\nb\n");
}

#[test]
fn cr_to_lf() {
	assert_eq!(normalize_to_lf("a\rb\rc".to_string()), "a\nb\nc");
}

#[test]
fn mixed_sequences() {
	assert_eq!(normalize_to_lf("a\r\nb\rc\n".to_string()), "a\nb\nc\n");
}

#[test]
fn idempotent() {
	let input = "a\r\nb\rc\n".to_string();
	let once = normalize_to_lf(input);
	let twice = normalize_to_lf(once.clone());
	assert_eq!(twice, once);
}
