//! Build-time seam enforcement: ensures frontend crates only access core editor
//! types through `render_api` and never reach into internal modules directly.

/// Forbidden substring patterns in frontend source files.
///
/// Each entry is `(pattern, reason)`. If any frontend `.rs` file contains the
/// pattern, the test fails with the reason and file location.
const FORBIDDEN_PATTERNS: &[(&str, &str)] = &[
	// Direct module imports that should go through render_api.
	("xeno_editor::completion::", "use render_api re-exports instead of xeno_editor::completion"),
	("xeno_editor::snippet::", "use render_api re-exports instead of xeno_editor::snippet"),
	("xeno_editor::overlay::", "use render_api re-exports instead of xeno_editor::overlay"),
	("xeno_editor::ui::", "use render_api re-exports instead of xeno_editor::ui"),
	("xeno_editor::info_popup::", "use render_api re-exports instead of xeno_editor::info_popup"),
	("xeno_editor::window::", "use render_api re-exports instead of xeno_editor::window"),
	("xeno_editor::geometry::", "use render_api re-exports instead of xeno_editor::geometry"),
	// Internal render module (frontends should use render_api, not render::).
	("xeno_editor::render::", "use render_api re-exports instead of xeno_editor::render"),
	// Policy/internal calls that should not appear in frontends.
	("buffer_view_render_plan", "use core-owned view plan APIs instead"),
	("editor.layout(", "use core-owned separator/document plan APIs instead"),
	(".layout(", "use core-owned separator/document plan APIs instead"),
	(".layout_mut(", "use core-owned separator/document plan APIs instead"),
	("base_window().layout", "use core-owned plan APIs instead"),
	(".ui_mut(", "use core-owned frame planning APIs instead"),
	(".viewport_mut(", "use core-owned frame planning APIs instead"),
	(".frame_mut(", "use core-owned frame planning APIs instead"),
	// Legacy single-document render plan (replaced by document_view_plans).
	("DocumentRenderPlan", "use document_view_plans() instead"),
	("focused_document_render_plan", "use document_view_plans() instead"),
	// Legacy drift vectors.
	("BufferRenderContext", "internal render type leaked to frontend"),
	("RenderBufferParams", "internal render type leaked to frontend"),
];

/// Frontend source directories to scan, relative to this crate's manifest dir.
const FRONTEND_DIRS: &[&str] = &["../editor-tui/src", "../editor-iced/src"];

#[cfg(test)]
mod tests {
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
}
