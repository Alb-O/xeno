use super::*;

#[test]
fn test_grammar_search_paths_not_empty() {
	let dirs = grammar_search_paths();
	assert!(!dirs.is_empty());
}

#[test]
fn test_query_search_paths_not_empty() {
	let dirs = query_search_paths();
	assert!(!dirs.is_empty());
}

#[test]
fn test_grammar_library_name() {
	let name = grammar_library_name("rust");
	#[cfg(target_os = "linux")]
	assert_eq!(name, "librust.so");
	#[cfg(target_os = "macos")]
	assert_eq!(name, "librust.dylib");
	#[cfg(target_os = "windows")]
	assert_eq!(name, "rust.dll");
}

#[test]
fn test_cache_dir_is_some() {
	#[cfg(unix)]
	assert!(cache_dir().is_some());
}
