use xeno_primitives::ViewId;

use crate::buffer::Buffer;
use crate::geometry::Rect;
use crate::render::BufferRenderContext;
use crate::window::GutterSelector;

fn theme_from_entry(
	theme_ref: xeno_registry::core::RegistryRef<xeno_registry::themes::theme::ThemeEntry, xeno_registry::core::ThemeId>,
) -> xeno_registry::themes::Theme {
	xeno_registry::themes::Theme {
		meta: xeno_registry::RegistryMetaStatic::minimal("test", "test", ""),
		variant: theme_ref.variant,
		colors: theme_ref.colors,
	}
}

fn line_text(line: &crate::render::RenderLine<'_>) -> String {
	line.spans.iter().map(|span| span.content.as_ref()).collect()
}

#[test]
fn test_render_baseline() {
	let buffer = Buffer::new(ViewId::text(1), "Hello world".to_string(), None);
	let theme = theme_from_entry(xeno_registry::themes::get_theme("monokai").unwrap());
	let loader = xeno_language::LanguageLoader::from_embedded();
	let syntax_manager = xeno_syntax::SyntaxManager::default();

	let ctx = BufferRenderContext {
		theme: &theme,
		language_loader: &loader,
		syntax_manager: &syntax_manager,
		diagnostics: None,
		diagnostic_ranges: None,
		inlay_hints: None,
		#[cfg(feature = "lsp")]
		semantic_tokens: None,
		#[cfg(feature = "lsp")]
		document_highlights: None,
	};

	let area = Rect::new(0, 0, 20, 3);
	let mut cache = crate::render::cache::RenderCache::new();
	let result = ctx.render_buffer_with_gutter(crate::render::buffer::context::types::RenderBufferParams {
		buffer: &buffer,
		area,
		use_block_cursor: true,
		is_focused: true,
		gutter: GutterSelector::Registry,
		tab_width: 4,
		cursorline: false,
		cache: &mut cache,
	});

	assert!(result.gutter_width > 0);
	assert!(line_text(&result.gutter[0]).contains('1'));
	assert!(line_text(&result.text[0]).contains("Hello world"));
	assert!(line_text(&result.gutter[1]).contains('~'));
}

#[test]
fn test_render_wrapping() {
	// Use a very narrow text width to force wrapping
	let buffer = Buffer::new(ViewId::text(1), "One two three four five".to_string(), None);
	let theme = theme_from_entry(xeno_registry::themes::get_theme("monokai").unwrap());
	let loader = xeno_language::LanguageLoader::from_embedded();
	let syntax_manager = xeno_syntax::SyntaxManager::default();

	let ctx = BufferRenderContext {
		theme: &theme,
		language_loader: &loader,
		syntax_manager: &syntax_manager,
		diagnostics: None,
		diagnostic_ranges: None,
		inlay_hints: None,
		#[cfg(feature = "lsp")]
		semantic_tokens: None,
		#[cfg(feature = "lsp")]
		document_highlights: None,
	};

	// 30 width, gutter will take ~6, leaving ~24 for text
	let area = Rect::new(0, 0, 30, 5);
	let mut cache = crate::render::cache::RenderCache::new();
	let result = ctx.render_buffer_with_gutter(crate::render::buffer::context::types::RenderBufferParams {
		buffer: &buffer,
		area,
		use_block_cursor: true,
		is_focused: true,
		gutter: GutterSelector::Registry,
		tab_width: 4,
		cursorline: false,
		cache: &mut cache,
	});

	assert!(line_text(&result.gutter[0]).contains('1'));
	assert!(line_text(&result.text[0]).contains("One two three four five"));
}
