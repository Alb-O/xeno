use super::*;

#[test]
fn test_load_grammar_configs() {
	let result = load_grammar_configs();
	assert!(result.is_ok(), "Failed to parse: {:?}", result.err());

	let configs = result.unwrap();
	assert!(!configs.is_empty(), "No grammar configs found");

	// Check that rust grammar exists
	let rust = configs.iter().find(|c| c.grammar_id == "rust");
	assert!(rust.is_some(), "Rust grammar not found");

	let rust = rust.unwrap();
	match &rust.source {
		GrammarSource::Git { remote, .. } => {
			assert!(remote.contains("tree-sitter-rust"));
		}
		GrammarSource::Local { .. } => panic!("Expected git source for rust"),
	}
}

#[test]
fn test_library_extension() {
	let ext = library_extension();
	#[cfg(target_os = "linux")]
	assert_eq!(ext, "so");
	#[cfg(target_os = "macos")]
	assert_eq!(ext, "dylib");
	#[cfg(target_os = "windows")]
	assert_eq!(ext, "dll");
}
