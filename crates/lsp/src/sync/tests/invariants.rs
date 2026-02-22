/// Source-level invariant: no direct `ClientHandle.text_document_did_*` calls outside `crates/lsp/src/sync/`.
///
/// All didOpen/didClose/didChange must flow through `DocumentSync` to maintain registration
/// state consistency. Direct calls bypass the unregister-on-failure and force_full_sync guards.
#[test]
fn no_direct_did_notifications_outside_sync_module() {
	use std::path::Path;

	let workspace_root = Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap().parent().unwrap();
	let crates_dir = workspace_root.join("crates");

	let forbidden_patterns = [
		".text_document_did_open(",
		".text_document_did_close(",
		".text_document_did_change(",
		".text_document_did_change_full(",
		".text_document_did_change_with_barrier(",
	];

	let sync_dir = crates_dir.join("lsp").join("src").join("sync");
	let client_api_dir = crates_dir.join("lsp").join("src").join("client").join("api");

	fn walk_rs_files(dir: &Path, out: &mut Vec<std::path::PathBuf>) {
		let Ok(entries) = std::fs::read_dir(dir) else { return };
		for entry in entries.flatten() {
			let path = entry.path();
			if path.is_dir() {
				walk_rs_files(&path, out);
			} else if path.extension().is_some_and(|ext| ext == "rs") {
				out.push(path);
			}
		}
	}

	let mut rs_files = Vec::new();
	walk_rs_files(&crates_dir, &mut rs_files);

	let mut violations = Vec::new();

	for path in &rs_files {
		// Allow the sync module itself (implementation).
		if path.starts_with(&sync_dir) {
			continue;
		}
		// Allow client API definitions.
		if path.starts_with(&client_api_dir) {
			continue;
		}

		let content = match std::fs::read_to_string(path) {
			Ok(c) => c,
			Err(_) => continue,
		};

		for pattern in &forbidden_patterns {
			for (line_no, line) in content.lines().enumerate() {
				if line.contains(pattern) {
					violations.push(format!(
						"{}:{}: {}",
						path.strip_prefix(workspace_root).unwrap_or(path).display(),
						line_no + 1,
						line.trim()
					));
				}
			}
		}
	}

	assert!(
		violations.is_empty(),
		"Direct ClientHandle.text_document_did_* calls found outside sync module.\n\
		 All didOpen/didClose/didChange must go through DocumentSync.\n\
		 Violations:\n{}",
		violations.join("\n")
	);
}
