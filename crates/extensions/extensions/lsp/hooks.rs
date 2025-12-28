//! LSP hooks for document synchronization.
//!
//! These hooks respond to buffer events and notify language servers.

use std::sync::Arc;

use linkme::distributed_slice;
use tome_api::editor::extensions::ExtensionMap;
use tome_manifest::hooks::{
	HOOKS, HookAction, HookContext, HookDef, HookEvent, HookResult, OwnedHookContext,
};
use tome_manifest::RegistrySource;

use super::LspManager;

#[distributed_slice(HOOKS)]
static LSP_BUFFER_OPEN: HookDef = HookDef {
	id: "tome-extensions::lsp::buffer_open",
	name: "lsp_buffer_open",
	event: HookEvent::BufferOpen,
	description: "Notify language servers when a buffer is opened",
	priority: 50,
	handler: lsp_buffer_open_handler,
	source: RegistrySource::Crate("tome-extensions"),
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
	id: "tome-extensions::lsp::buffer_change",
	name: "lsp_buffer_change",
	event: HookEvent::BufferChange,
	description: "Notify language servers when buffer content changes",
	priority: 50,
	handler: lsp_buffer_change_handler,
	source: RegistrySource::Crate("tome-extensions"),
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
			version,
		} = owned
		{
			let language = infer_language_from_path(&path);
			lsp.did_change(&path, &text, language.as_deref(), version)
				.await;
		}
		HookResult::Continue
	}))
}

#[distributed_slice(HOOKS)]
static LSP_BUFFER_CLOSE: HookDef = HookDef {
	id: "tome-extensions::lsp::buffer_close",
	name: "lsp_buffer_close",
	event: HookEvent::BufferClose,
	description: "Notify language servers when a buffer is closed",
	priority: 50,
	handler: lsp_buffer_close_handler,
	source: RegistrySource::Crate("tome-extensions"),
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
		if let OwnedHookContext::BufferClose { path } = owned {
			let language = infer_language_from_path(&path);
			lsp.did_close(&path, language.as_deref()).await;
		}
		HookResult::Continue
	}))
}

#[distributed_slice(HOOKS)]
static LSP_EDITOR_QUIT: HookDef = HookDef {
	id: "tome-extensions::lsp::editor_quit",
	name: "lsp_editor_quit",
	event: HookEvent::EditorQuit,
	description: "Shutdown language servers when editor quits",
	priority: 10,
	handler: lsp_editor_quit_handler,
	source: RegistrySource::Crate("tome-extensions"),
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
