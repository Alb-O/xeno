use std::path::{Path, PathBuf};

use super::*;

fn collect_rs_files(dir: &Path) -> Vec<PathBuf> {
	let mut files = Vec::new();
	if !dir.is_dir() {
		return files;
	}
	let mut stack = vec![dir.to_path_buf()];
	while let Some(d) = stack.pop() {
		let Ok(entries) = std::fs::read_dir(&d) else {
			continue;
		};
		for entry in entries.flatten() {
			let path = entry.path();
			if path.is_dir() {
				stack.push(path);
			} else if path.extension().is_some_and(|e| e == "rs") {
				files.push(path);
			}
		}
	}
	files.sort();
	files
}

#[test]
fn frontend_sources_use_only_render_api_seam() {
	let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
	let mut violations = Vec::new();

	for rel_dir in FRONTEND_DIRS {
		let dir = manifest_dir.join(rel_dir);
		for file in collect_rs_files(&dir) {
			let Ok(content) = std::fs::read_to_string(&file) else {
				continue;
			};
			for (line_num, line) in content.lines().enumerate() {
				for &(pattern, reason) in FORBIDDEN_PATTERNS {
					if line.contains(pattern) {
						violations.push(format!(
							"  {}:{}: {}\n    > {}\n    reason: {}",
							file.display(),
							line_num + 1,
							pattern,
							line.trim(),
							reason,
						));
					}
				}
			}
		}
	}

	if !violations.is_empty() {
		panic!(
			"Seam contract violations found ({} total):\n\n{}\n\n\
			 Fix: use xeno_editor::render_api::* or core-owned plan APIs.\n\
			 See FORBIDDEN_PATTERNS in seam_contract.rs for the full list.",
			violations.len(),
			violations.join("\n\n"),
		);
	}
}
