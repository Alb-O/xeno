//! LSP hooks for document synchronization.
//!
//! These hooks respond to buffer events and notify language servers.

use std::sync::Arc;

use linkme::distributed_slice;
use evildoer_api::editor::extensions::ExtensionMap;
use evildoer_manifest::RegistrySource;
use evildoer_manifest::hooks::{
	HOOKS, HookAction, HookContext, HookDef, HookEvent, HookResult, OwnedHookContext,
};

use super::LspManager;

#[distributed_slice(HOOKS)]
static LSP_BUFFER_OPEN: HookDef = HookDef {
	id: "evildoer-extensions::lsp::buffer_open",
	name: "lsp_buffer_open",
	event: HookEvent::BufferOpen,
	description: "Notify language servers when a buffer is opened",
	priority: 50,
	handler: lsp_buffer_open_handler,
	source: RegistrySource::Crate("evildoer-extensions"),
};

fn lsp_buffer_open_handler(ctx: &HookContext) -> HookAction {
	let Some(lsp) = ctx
		.extensions::<ExtensionMap>()
		.and_then(|ext| ext.get::<Arc<LspManager>>())
		.cloned()
	else {
		return HookAction::done();
	};
	let owned = ctx.to_owned();

	HookAction::Async(Box::pin(async move {
		if let OwnedHookContext::BufferOpen {
			path,
			text,
			file_type,
		} = owned
		{
			lsp.did_open(&path, &text, file_type.as_deref(), 1).await;
		}
		HookResult::Continue
	}))
}

#[distributed_slice(HOOKS)]
static LSP_BUFFER_CHANGE: HookDef = HookDef {
	id: "evildoer-extensions::lsp::buffer_change",
	name: "lsp_buffer_change",
	event: HookEvent::BufferChange,
	description: "Notify language servers when buffer content changes",
	priority: 50,
	handler: lsp_buffer_change_handler,
	source: RegistrySource::Crate("evildoer-extensions"),
};

fn lsp_buffer_change_handler(ctx: &HookContext) -> HookAction {
	let Some(lsp) = ctx
		.extensions::<ExtensionMap>()
		.and_then(|ext| ext.get::<Arc<LspManager>>())
		.cloned()
	else {
		return HookAction::done();
	};
	let owned = ctx.to_owned();

	HookAction::Async(Box::pin(async move {
		if let OwnedHookContext::BufferChange {
			path,
			text,
			file_type,
			version,
		} = owned
		{
			let language = file_type.or_else(|| infer_language_from_path(&path));
			lsp.did_change(&path, &text, language.as_deref(), version)
				.await;
		}
		HookResult::Continue
	}))
}

#[distributed_slice(HOOKS)]
static LSP_BUFFER_CLOSE: HookDef = HookDef {
	id: "evildoer-extensions::lsp::buffer_close",
	name: "lsp_buffer_close",
	event: HookEvent::BufferClose,
	description: "Notify language servers when a buffer is closed",
	priority: 50,
	handler: lsp_buffer_close_handler,
	source: RegistrySource::Crate("evildoer-extensions"),
};

fn lsp_buffer_close_handler(ctx: &HookContext) -> HookAction {
	let Some(lsp) = ctx
		.extensions::<ExtensionMap>()
		.and_then(|ext| ext.get::<Arc<LspManager>>())
		.cloned()
	else {
		return HookAction::done();
	};
	let owned = ctx.to_owned();

	HookAction::Async(Box::pin(async move {
		if let OwnedHookContext::BufferClose { path, file_type } = owned {
			let language = file_type.or_else(|| infer_language_from_path(&path));
			lsp.did_close(&path, language.as_deref()).await;
		}
		HookResult::Continue
	}))
}

#[distributed_slice(HOOKS)]
static LSP_EDITOR_QUIT: HookDef = HookDef {
	id: "evildoer-extensions::lsp::editor_quit",
	name: "lsp_editor_quit",
	event: HookEvent::EditorQuit,
	description: "Shutdown language servers when editor quits",
	priority: 10,
	handler: lsp_editor_quit_handler,
	source: RegistrySource::Crate("evildoer-extensions"),
};

fn lsp_editor_quit_handler(ctx: &HookContext) -> HookAction {
	let Some(lsp) = ctx
		.extensions::<ExtensionMap>()
		.and_then(|ext| ext.get::<Arc<LspManager>>())
		.cloned()
	else {
		return HookAction::done();
	};

	HookAction::Async(Box::pin(async move {
		lsp.shutdown_all().await;
		HookResult::Continue
	}))
}

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

	use evildoer_api::editor::extensions::ExtensionMap;
	use evildoer_manifest::hooks::{
		HookAction, HookContext, HookEvent, HookEventData, OwnedHookContext,
	};

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
		assert_eq!(LSP_BUFFER_OPEN.event, HookEvent::BufferOpen);
		assert_eq!(LSP_BUFFER_CHANGE.event, HookEvent::BufferChange);
		assert_eq!(LSP_BUFFER_CLOSE.event, HookEvent::BufferClose);
		assert_eq!(LSP_EDITOR_QUIT.event, HookEvent::EditorQuit);
		assert!(LSP_EDITOR_QUIT.priority < LSP_BUFFER_OPEN.priority);
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
		assert!(matches!(lsp_buffer_open_handler(&ctx), HookAction::Done(_)));
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
		assert!(matches!(lsp_buffer_open_handler(&ctx), HookAction::Done(_)));
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
