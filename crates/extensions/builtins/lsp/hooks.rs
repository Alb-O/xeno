//! LSP hooks for document synchronization.
//!
//! These hooks respond to buffer events and notify language servers.

use std::path::PathBuf;
use std::sync::Arc;

use xeno_api::editor::extensions::ExtensionMap;
use xeno_registry::{HookAction, HookContext, HookResult, async_hook};

use super::LspManager;

fn lsp_from_ctx(ctx: &HookContext) -> Option<Arc<LspManager>> {
	ctx.extensions::<ExtensionMap>()
		.and_then(|ext| ext.get::<Arc<LspManager>>())
		.cloned()
}

async_hook!(
	lsp_buffer_open,
	BufferOpen,
	50,
	"Notify language servers when a buffer is opened",
	setup |ctx| {
		let Some(lsp) = lsp_from_ctx(ctx) else {
			return HookAction::done();
		};
	}
	async |path: PathBuf, text: String, file_type: Option<String>| {
		lsp.did_open(&path, &text, file_type.as_deref(), 1).await;
		HookResult::Continue
	}
);

async_hook!(
	lsp_buffer_change,
	BufferChange,
	50,
	"Notify language servers when buffer content changes",
	setup |ctx| {
		let Some(lsp) = lsp_from_ctx(ctx) else {
			return HookAction::done();
		};
	}
	async |path: PathBuf, text: String, file_type: Option<String>, version: u64| {
		let language = file_type.or_else(|| infer_language_from_path(&path));
		lsp.did_change(&path, &text, language.as_deref(), version)
			.await;
		HookResult::Continue
	}
);

async_hook!(
	lsp_buffer_close,
	BufferClose,
	50,
	"Notify language servers when a buffer is closed",
	setup |ctx| {
		let Some(lsp) = lsp_from_ctx(ctx) else {
			return HookAction::done();
		};
	}
	async |path: PathBuf, file_type: Option<String>| {
		let language = file_type.or_else(|| infer_language_from_path(&path));
		lsp.did_close(&path, language.as_deref()).await;
		HookResult::Continue
	}
);

async_hook!(
	lsp_editor_quit,
	EditorQuit,
	10,
	"Shutdown language servers when editor quits",
	setup |ctx| {
		let Some(lsp) = lsp_from_ctx(ctx) else {
			return HookAction::done();
		};
	}
	async || {
		lsp.shutdown_all().await;
		HookResult::Continue
	}
);

/// Infer language name from file path extension.
fn infer_language_from_path(path: &std::path::Path) -> Option<String> {
	let ext = path.extension()?.to_str()?;
	let language = match ext {
		"rs" => "rust",
		"ts" | "tsx" => "typescript",
		"js" | "jsx" | "mjs" | "cjs" => "javascript",
		"py" | "pyi" => "python",
		"go" => "go",
		"c" | "h" => "c",
		"cpp" | "hpp" | "cc" | "cxx" | "hxx" => "cpp",
		"java" => "java",
		"kt" | "kts" => "kotlin",
		"rb" => "ruby",
		"php" => "php",
		"lua" => "lua",
		"zig" => "zig",
		"nim" => "nim",
		"swift" => "swift",
		"cs" => "csharp",
		"fs" | "fsx" => "fsharp",
		"ml" | "mli" => "ocaml",
		"hs" | "lhs" => "haskell",
		"ex" | "exs" => "elixir",
		"erl" | "hrl" => "erlang",
		"clj" | "cljs" | "cljc" => "clojure",
		"scala" | "sc" => "scala",
		"toml" => "toml",
		"yaml" | "yml" => "yaml",
		"json" | "jsonc" => "json",
		"md" | "markdown" => "markdown",
		"html" | "htm" => "html",
		"css" => "css",
		"scss" | "sass" => "scss",
		"vue" => "vue",
		"svelte" => "svelte",
		"nix" => "nix",
		"sh" | "bash" | "zsh" => "bash",
		_ => return None,
	};
	Some(language.to_string())
}

#[cfg(test)]
mod tests {
	use std::path::Path;

	use xeno_api::editor::extensions::ExtensionMap;
	use xeno_registry::{HookAction, HookContext, HookEvent, HookEventData, OwnedHookContext};

	use super::*;

	#[test]
	fn infer_language_from_path_common_extensions() {
		let cases = [
			("main.rs", Some("rust")),
			("app.ts", Some("typescript")),
			("component.tsx", Some("typescript")),
			("script.js", Some("javascript")),
			("module.mjs", Some("javascript")),
			("script.py", Some("python")),
			("main.go", Some("go")),
			("main.c", Some("c")),
			("main.cpp", Some("cpp")),
			("Cargo.toml", Some("toml")),
			("config.yaml", Some("yaml")),
			("package.json", Some("json")),
			("file.xyz", None),
			("Makefile", None),
			("/home/user/src/main.rs", Some("rust")),
		];
		for (path, expected) in cases {
			assert_eq!(
				infer_language_from_path(Path::new(path)),
				expected.map(String::from),
				"path: {path}"
			);
		}
	}

	#[test]
	fn hook_definitions_have_correct_events() {
		assert_eq!(HOOK_lsp_buffer_open.event, HookEvent::BufferOpen);
		assert_eq!(HOOK_lsp_buffer_change.event, HookEvent::BufferChange);
		assert_eq!(HOOK_lsp_buffer_close.event, HookEvent::BufferClose);
		assert_eq!(HOOK_lsp_editor_quit.event, HookEvent::EditorQuit);
		assert!(HOOK_lsp_editor_quit.priority < HOOK_lsp_buffer_open.priority);
	}

	#[test]
	fn handler_returns_done_without_extensions() {
		let rope = ropey::Rope::from_str("fn main() {}");
		let ctx = HookContext::new(
			HookEventData::BufferOpen {
				path: Path::new("test.rs"),
				text: rope.slice(..),
				file_type: Some("rust"),
			},
			None,
		);
		assert!(matches!(
			hook_handler_lsp_buffer_open(&ctx),
			HookAction::Done(_)
		));
	}

	#[test]
	fn handler_returns_done_without_lsp_manager() {
		let ext_map = ExtensionMap::new();
		let rope = ropey::Rope::from_str("fn main() {}");
		let ctx = HookContext::new(
			HookEventData::BufferOpen {
				path: Path::new("test.rs"),
				text: rope.slice(..),
				file_type: Some("rust"),
			},
			Some(&ext_map),
		);
		assert!(matches!(
			hook_handler_lsp_buffer_open(&ctx),
			HookAction::Done(_)
		));
	}

	#[test]
	fn owned_context_preserves_file_type() {
		let rope = ropey::Rope::from_str("fn main() {}");
		let ctx = HookContext::new(
			HookEventData::BufferChange {
				path: Path::new("test.rs"),
				text: rope.slice(..),
				file_type: Some("rust"),
				version: 1,
			},
			None,
		);
		let OwnedHookContext::BufferChange { file_type, .. } = ctx.to_owned() else {
			panic!("expected BufferChange");
		};
		assert_eq!(file_type, Some("rust".to_string()));
	}

	#[test]
	fn file_type_takes_precedence_over_path_inference() {
		let path = Path::new("main.rs");
		assert_eq!(
			None::<String>.or_else(|| infer_language_from_path(path)),
			Some("rust".to_string())
		);
		assert_eq!(
			Some("custom".to_string()).or_else(|| infer_language_from_path(path)),
			Some("custom".to_string())
		);
	}
}
